use std::{
    sync::{Arc, RwLock},
    task::Poll,
};

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use hyparview::message::{DisconnectMessage, ProtocolMessage};
use quinn::{ApplicationClose, Connection, Endpoint};
use rootcause::{Report, prelude::ResultExt};
use tokio::sync::{
    Mutex,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use tokio_util::{
    bytes::Bytes,
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
};
use tracing::{debug, error, info, warn};

use crate::p2p::node::{
    message::{MessagePayload, P2pNodeProtocolMessage},
    node_id::{LocalNodeId, NodeId},
};

#[derive(Debug)]
pub struct P2PNodeServer<M: MessagePayload> {
    server_is_running: bool,
    handle: ServerHandle<M>,
}

impl<M: MessagePayload> P2PNodeServer<M> {
    pub fn new(endpoint: Endpoint, server_name: String) -> Self {
        let handle = ServerHandle {
            server_name,
            endpoint,
            local_nodes: Arc::new(RwLock::new(None)),
            remote_nodes: Arc::new(DashMap::new()),
            connection_locks: Arc::new(DashMap::new()),
        };
        P2PNodeServer {
            server_is_running: false,
            handle,
        }
    }

    pub fn handle(&self) -> ServerHandle<M> {
        self.handle.clone()
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
    server_name: String,
    endpoint: Endpoint,
    local_nodes: Arc<RwLock<Option<(LocalNodeId, NodeHandle<M>)>>>,
    remote_nodes: Arc<DashMap<LocalNodeId, Connection>>,
    connection_locks: Arc<DashMap<LocalNodeId, Arc<Mutex<()>>>>,
}

impl<M: MessagePayload> ServerHandle<M> {
    /// 启动quic客户端
    #[tracing::instrument(skip_all, fields(local_addr=%self.endpoint.local_addr().expect("never fails").to_string()))]
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
    #[tracing::instrument(skip_all, fields(sender, remote_addr=conn.remote_address().to_string()))]
    async fn handle_connection(&self, conn: Connection) {
        let addr = conn.remote_address();
        let mut connection_id = None;
        let mut hanle = async || -> Result<(), Report> {
            loop {
                match conn.accept_bi().await {
                    Ok((_tx, rx)) => {
                        let mut framed_reader = FramedRead::new(rx, LengthDelimitedCodec::new());
                        while let Some(bytes_mut) = framed_reader.next().await {
                            let bytes = bytes_mut?;
                            let p2p_node_protocol_message: P2pNodeProtocolMessage<M> =
                                serde_json::from_slice(&bytes)?;
                            debug!("Fetch a protocol message: {p2p_node_protocol_message:?}");
                            if connection_id.is_none() {
                                let node_id = match &p2p_node_protocol_message {
                                    P2pNodeProtocolMessage::Hyparview(protocol_message) => {
                                        protocol_message.sender().clone()
                                    }
                                    P2pNodeProtocolMessage::Plumtree(protocol_message) => {
                                        protocol_message.sender().clone()
                                    }
                                };
                                debug!("Handle a connection from: {node_id:?}");
                                tracing::Span::current().record(
                                    "sender",
                                    format!(
                                        "NodeId ( address: {}, local_id: {} )",
                                        node_id.address(),
                                        node_id.local_id()
                                    ),
                                );
                                // self.set_connection(&node_id, conn.clone()).await;
                                connection_id = Some(node_id);
                            }

                            if let Ok(read) = self.local_nodes.read() {
                                if let Some((_id, node)) = read.as_ref() {
                                    node.message_tx.send(p2p_node_protocol_message)?;
                                }
                            }
                        }
                    }
                    Err(quinn::ConnectionError::ApplicationClosed(ApplicationClose {
                        error_code,
                        reason,
                    })) => {
                        debug!(
                            "IP: {} Closed: error_code={}, reason={}",
                            addr,
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

        if let Err(e) = hanle().await {
            error!("Handle connection err: {e}");
        }
        // 已经退出循环了，发送断联消息
        if let Some(connection_id) = connection_id {
            self.notify_self_disconnect(&connection_id);
        }
    }

    /// 注册本地的节点
    pub(crate) fn register_local_node(&self, node: NodeHandle<M>) {
        if let Ok(mut write) = self.local_nodes.write() {
            write.replace((node.local_id.clone(), node));
        }
    }

    pub fn remove_remote_node(&self, node: &LocalNodeId) {
        self.remote_nodes.remove(node);
    }

    async fn get_connection(&self, nodeid: &NodeId) -> Result<Connection, Report> {
        let addr = nodeid.address();

        // 首先检查是否已存在连接
        if let Some(conn) = self.remote_nodes.get(nodeid.local_id()) {
            return Ok(conn.clone());
        }

        let lock = self
            .connection_locks
            .entry(nodeid.local_id().clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        let _guard = lock.lock().await;

        // 双重检查，确保在获取锁后没有其他任务已经建立了连接
        if let Some(conn) = self.remote_nodes.get(nodeid.local_id()) {
            return Ok(conn.clone());
        }

        // 如果没有现成的连接，创建一个新的
        let connection = self
            .endpoint
            .connect(
                addr,
                &self.server_name, // 必须与证书中的域名匹配
            )?
            .await
            .context(format!("连接到节点: {addr}出错"))?;
        info!("Connected to server: {:?}", connection.remote_address());

        // 将连接存入缓存
        self.remote_nodes
            .insert(nodeid.local_id().clone(), connection.clone());

        Ok(connection)
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
        destination: NodeId,
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
    #[tracing::instrument(skip_all, fields(sender = %self.endpoint.local_addr().expect("never fails").to_string()))]
    pub async fn send_protocol_message_async(
        &self,
        destination: NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        debug!("Send a protocol message: {protocol_message:?} to {destination:?}");
        let protocol_message = serde_json::to_vec(&protocol_message)?;
        let connection = self.get_connection(&destination).await?;
        let (tx, _rx) = connection.open_bi().await?;
        let mut framed_writer = FramedWrite::new(tx, LengthDelimitedCodec::new());
        framed_writer.send(Bytes::from(protocol_message)).await?;
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
        if let Ok(read) = self.local_nodes.read() {
            if let Some((_id, node)) = read.as_ref() {
                let _ = node.message_tx.send(message);
            }
        }
        self.remove_remote_node(destination.local_id());
    }
}

// impl<M: MessagePayload> Future for ServerHandle<M> {
//     type Output = ();

//     fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
//         let handle = self.get_mut();
//         while let Poll::Ready(Some(conn)) = pin!(handle.endpoint.accept()).poll_unpin(cx) {
//             if let Ok(mut accept) = conn.accept() {
//                 if let Poll::Ready(Ok(connection)) = accept.poll_unpin(cx) {
//                     let handle_cloned = handle.clone();
//                     tokio::spawn(async move {
//                         if let Err(e) = handle_cloned.handle_connection(connection).await {
//                             error!("Handle connection err: {e}");
//                         }
//                     });
//                 }
//             };
//         }
//         cx.waker().wake_by_ref();
//         Poll::Pending
//     }
// }

#[derive(Debug)]
enum Command<M: MessagePayload> {
    Register(NodeHandle<M>),
    Deregister(LocalNodeId),
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
