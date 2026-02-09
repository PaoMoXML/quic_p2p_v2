use std::{
    cell::LazyCell,
    collections::HashSet,
    net::{IpAddr, SocketAddr},
    sync::{Arc, RwLock},
    time::Duration,
};

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use hyparview::message::{DisconnectMessage, ProtocolMessage};
use quinn::{ApplicationClose, Connection, Endpoint, RecvStream};
use rootcause::{Report, prelude::ResultExt};
use tokio::sync::{
    Mutex,
    mpsc::{self, UnboundedSender},
    oneshot,
};
use tokio_util::{
    bytes::Bytes,
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
};
use tracing::{Instrument, debug, error, info, info_span, warn};

use crate::p2p::{
    net::create_endpoint,
    node::{
        message::{MessagePayload, P2pNodeProtocolMessage},
        node_id::{LocalNodeId, NodeId, SecretKey},
    },
    tls::{self, TlsConfig},
};
#[derive(Debug, Default)]
pub struct P2PNodeServerBuilder {
    secret_key: Option<SecretKey>,
    tls_config: Option<TlsConfig>,
    alpn_protocols: Vec<Vec<u8>>,
}

impl P2PNodeServerBuilder {
    pub fn secret_key(mut self, secret_key: SecretKey) -> Self {
        self.secret_key.replace(secret_key);
        self
    }

    pub fn tls_config(mut self, tls_config: TlsConfig) -> Self {
        self.tls_config.replace(tls_config);
        self
    }

    pub fn alpn_protocols(mut self, alpn_protocols: Vec<Vec<u8>>) -> Self {
        self.alpn_protocols = alpn_protocols;
        self
    }

    pub async fn bind<M: MessagePayload>(
        self,
        addr: Option<SocketAddr>,
    ) -> Result<P2PNodeServer<M>, Report> {
        let tls_config = match self.tls_config {
            Some(tls_config) => tls_config,
            None => TlsConfig::new(self.secret_key.expect("Secret Key is None")),
        };
        let client_config = tls_config.make_client_config(self.alpn_protocols.clone())?;
        let server_config = tls_config.make_server_config(self.alpn_protocols)?;
        // let mut endpoint = Endpoint::client(addr)?;
        let (mut endpoint, stun_addr) = create_endpoint(addr).await?;
        endpoint.set_server_config(Some(server_config));
        endpoint.set_default_client_config(client_config);

        let handle = ServerHandle {
            endpoint,
            local_node: Arc::new(RwLock::new(None)),
            remote_nodes: Arc::new(DashMap::new()),
            connection_locks: Arc::new(DashMap::new()),
        };
        let local_ip = get_local_ip()?;
        let mut hash_set = HashSet::new();
        hash_set.insert(stun_addr);
        hash_set.insert(SocketAddr::new(
            local_ip,
            handle.endpoint.local_addr()?.port(),
        ));
        Ok(P2PNodeServer {
            addresses: hash_set,
            tls_config,
            server_is_running: false,
            handle,
        })
    }
}

#[derive(Debug)]
pub struct P2PNodeServer<M: MessagePayload> {
    addresses: HashSet<SocketAddr>,
    tls_config: TlsConfig,
    server_is_running: bool,
    handle: ServerHandle<M>,
}

fn get_local_ip() -> Result<IpAddr, Report> {
    // 创建一个UDP socket连接到一个远程地址，然后获取本地地址
    // 这个方法可以获取到实际使用的网络接口的IP地址
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?; // 连接到Google DNS服务器
    let local_addr = socket.local_addr()?;
    Ok(local_addr.ip())
}

impl<M: MessagePayload> P2PNodeServer<M> {
    pub fn builder() -> P2PNodeServerBuilder {
        P2PNodeServerBuilder::default()
    }

    pub fn handle(&self) -> ServerHandle<M> {
        self.handle.clone()
    }

    pub fn get_addresses(&self) -> &HashSet<SocketAddr> {
        &self.addresses
    }
}

impl<M: MessagePayload> Future for P2PNodeServer<M> {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let server = self.get_mut();
        let handle = server.handle.clone();
        if !server.server_is_running {
            server.server_is_running = true;
            tokio::spawn(async move {
                if let Err(e) = handle.start_server().await {
                    warn!("Server err: {e:?}");
                };
            });
        }
        // 返回 Pending，等待新命令时由接收器唤醒
        std::task::Poll::Pending
    }
}

#[derive(Debug, Clone)]
pub struct ServerHandle<M: MessagePayload> {
    endpoint: Endpoint,
    local_node: Arc<RwLock<Option<(LocalNodeId, NodeHandle<M>)>>>,
    remote_nodes: Arc<DashMap<LocalNodeId, Connection>>,
    connection_locks: Arc<DashMap<LocalNodeId, Arc<Mutex<()>>>>,
}

impl<M: MessagePayload> ServerHandle<M> {
    pub async fn start_server(&self) -> Result<(), Report> {
        info!("Server listen on: {}", self.endpoint.local_addr()?);
        // 接受传入连接
        loop {
            // 接受新连接
            match self.endpoint.accept().await {
                Some(conn) => match conn.await {
                    Ok(connection) => {
                        let handle = self.clone();
                        tokio::spawn(async move {
                            handle.handle_connection(connection).await;
                        });
                    }
                    Err(e) => {
                        warn!("Server accept connect err: {e}");
                    }
                },
                None => break,
            }
        }
        Ok(())
    }

    pub async fn stop_server(&self) -> Result<(), Report> {
        self.endpoint.close(0u32.into(), b"done");
        self.endpoint.wait_idle().await;
        Ok(())
    }

    /// 监听连接
    #[tracing::instrument(skip_all, fields(me = self.get_local_nodeid().expect("never fails").short(), remote))]
    async fn handle_connection(&self, conn: Connection) {
        let (node_id_tx, mut node_id_rx) = tokio::sync::mpsc::channel::<NodeId>(1);
        let node_id_op = Arc::new(Mutex::new(None));
        let node_id_op_clone = node_id_op.clone();
        let handle = self.clone();
        let conn_cloned = conn.clone();
        let span = tracing::Span::current();
        tokio::spawn(
            async move {
                if let Some(node_id) = node_id_rx.recv().await {
                    // 存储反向连接（从远程节点发起的连接）
                    handle.remote_nodes.insert(*node_id.local_id(), conn_cloned);
                    node_id_op_clone.lock().await.replace(node_id);
                    node_id_rx.close();
                }
            }
            .instrument(span.clone()),
        );

        let async_handle = async || -> Result<(), Report> {
            loop {
                match conn.accept_uni().await {
                    Ok(rx) => {
                        let handle = self.clone();
                        let node_id_tx = node_id_tx.clone();
                        tokio::spawn(
                            async move {
                                if let Err(err) = handle.handle_recv_stream(rx, node_id_tx).await {
                                    warn!("Handle recv stream err: {err}");
                                };
                            }
                            .instrument(span.clone()),
                        );
                    }
                    Err(quinn::ConnectionError::ApplicationClosed(ApplicationClose {
                        error_code,
                        reason,
                    })) => {
                        info!(
                            "Connection closed: error_code={}, reason={}",
                            error_code,
                            String::from_utf8_lossy(&reason)
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(Report::from(e));
                    }
                }
            }
        };

        if let Err(e) = async_handle().await {
            error!("Handle connection err: {e}");
        }
        // 已经退出循环了，发送断联消息
        if let Ok(mut node_id_guard) = node_id_op.try_lock_owned()
            && let Some(node_id) = node_id_guard.take()
        {
            self.notify_self_disconnect(&node_id);
        };
    }

    /// 处理recv stream
    async fn handle_recv_stream(
        &self,
        rx: RecvStream,
        node_id_tx: mpsc::Sender<NodeId>,
    ) -> Result<(), Report> {
        let mut framed_reader = FramedRead::new(rx, LengthDelimitedCodec::new());
        while let Some(bytes_mut) = framed_reader.next().await {
            let bytes = bytes_mut?;
            let p2p_node_protocol_message: P2pNodeProtocolMessage<M> =
                serde_json::from_slice(&bytes)?;
            debug!("Fetch a protocol message: {p2p_node_protocol_message:?}");
            // 专递node_id的通道没有关闭的话就发送通知，否则就是已经传递过了
            if !node_id_tx.is_closed() {
                let node_id = match &p2p_node_protocol_message {
                    P2pNodeProtocolMessage::Hyparview(protocol_message) => {
                        Some(protocol_message.sender().clone())
                    }
                    P2pNodeProtocolMessage::Plumtree(protocol_message) => {
                        Some(protocol_message.sender().clone())
                    }
                    P2pNodeProtocolMessage::PlumtreeStream(_plumtree_stream_message) => None,
                };
                if let Some(node_id) = node_id {
                    tracing::Span::current().record("remote", node_id.to_string());
                    let _ = node_id_tx.send(node_id).await;
                }
            }

            // 将消息传递给本地node
            if let Ok(read) = self.local_node.read()
                && let Some((_id, node)) = read.as_ref()
            {
                node.message_tx.send(p2p_node_protocol_message)?;
            }
        }

        Ok(())
    }

    /// 注册本地的节点
    pub(crate) fn register_local_node(&self, node: NodeHandle<M>) {
        if let Ok(mut write) = self.local_node.write() {
            write.replace((node.local_id, node));
        }
    }

    pub fn remove_remote_node(&self, node: &LocalNodeId) {
        self.remote_nodes.remove(node);
    }

    pub fn get_local_nodeid(&self) -> Option<LocalNodeId> {
        if let Ok(read) = self.local_node.read()
            && let Some((id, _node)) = read.as_ref()
        {
            Some(*id)
        } else {
            None
        }
    }

    #[tracing::instrument(skip_all, fields(me = self.get_local_nodeid().expect("never fails").short(), remote))]
    async fn get_connection(&self, nodeid: &NodeId) -> Result<Connection, Report> {
        // 首先检查是否已存在连接
        if let Some(conn) = self.remote_nodes.get(nodeid.local_id()) {
            return Ok(conn.clone());
        }

        let lock = self
            .connection_locks
            .entry(*nodeid.local_id())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        let _guard = lock.lock().await;

        // 双重检查，确保在获取锁后没有其他任务已经建立了连接
        if let Some(conn) = self.remote_nodes.get(nodeid.local_id()) {
            return Ok(conn.clone());
        }

        // 如果没有现成的连接，创建一个新的
        let addrs = nodeid.address();
        let mut connection = None;
        for addr in addrs {
            // 请求超时5s就放弃
            match tokio::time::timeout(
                Duration::from_secs(5),
                self.endpoint
                    .connect(*addr, &tls::name::encode(nodeid.local_id().public()))?,
            )
            .await
            {
                Ok(timeout) => match timeout {
                    Ok(conn) => {
                        connection.replace(conn);
                        break;
                    }
                    Err(_err) => {
                        continue;
                    }
                },
                Err(_cr) => {
                    warn!("Connect to: {} timeout", addr);
                    continue;
                }
            };
        }

        match connection {
            Some(connection) => {
                info!("Connected to server: {:?}", connection.remote_address());
                let handle = self.clone();
                let conn_cloned = connection.clone();
                // 接收连接消息
                tokio::spawn(async move {
                    handle.handle_connection(conn_cloned).await;
                });
                // 将连接存入缓存
                self.remote_nodes
                    .insert(*nodeid.local_id(), connection.clone());
                return Ok(connection);
            }
            None => {
                rootcause::bail!("连接到节点: {:?}出错", addrs);
            }
        }
    }

    /// 异步的消息发送
    pub fn send_protocol_message_sync(
        &self,
        destination: NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) {
        let handle = self.clone();
        tokio::spawn(async move {
            if let Err(e) = handle
                .send_protocol_message_async(destination.clone(), protocol_message)
                .await
            {
                // 由于在新行程里执行的异步消息发送，所以无法直接返回错误，使用消息管道进行发送
                handle.notify_self_disconnect(&destination);
                warn!("Quinn cannot send message to {destination:?}: {e}");
            };
        });
    }

    /// 同步的消息发送
    pub fn send_protocol_message(
        &self,
        destination: &NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        // 构建一个 tokio 运行时： Runtime
        let rt = tokio::runtime::Handle::current();
        let handle = self.clone();
        tokio::task::block_in_place(move || -> Result<(), Report> {
            rt.block_on(handle.send_protocol_message_async(destination.clone(), protocol_message))?;
            Ok(())
        })?;
        Ok(())
    }

    /// 执行异步的消息发送
    /// 实际的消息发送将会在这里发送
    pub async fn send_protocol_message_async(
        &self,
        destination: NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        debug!("Send a protocol message: {protocol_message:?} to {destination:?}");
        let protocol_message = serde_json::to_vec(&protocol_message)?;
        let connection = self.get_connection(&destination).await?;
        let tx = connection.open_uni().await?;
        let mut framed_writer = FramedWrite::new(tx, LengthDelimitedCodec::new());
        framed_writer.send(Bytes::from(protocol_message)).await?;
        tokio::time::timeout(Duration::from_secs(5), framed_writer.close()).await??;
        Ok(())
    }

    pub async fn send_protocol_message_stream_async(
        &self,
        destination: NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        Ok(())
    }

    /// 提醒断联
    /// 发送给本地节点，destination需要断联
    pub fn notify_self_disconnect(&self, destination: &NodeId) {
        // 无法发送消息，发送断联
        let message = DisconnectMessage {
            sender: destination.clone(),
            alive: false,
        };
        let message = ProtocolMessage::Disconnect(message);
        let message = P2pNodeProtocolMessage::Hyparview(message);
        if let Ok(read) = self.local_node.read()
            && let Some((_id, node)) = read.as_ref()
        {
            let _ = node.message_tx.send(message);
        }
        self.remove_remote_node(destination.local_id());
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NodeHandle<M: MessagePayload> {
    local_id: LocalNodeId,
    message_tx: UnboundedSender<P2pNodeProtocolMessage<M>>,
}

impl<M: MessagePayload> NodeHandle<M> {
    pub fn new(
        local_id: LocalNodeId,
        message_tx: UnboundedSender<P2pNodeProtocolMessage<M>>,
    ) -> Self {
        Self {
            local_id,
            message_tx,
        }
    }
}
