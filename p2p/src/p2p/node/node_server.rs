use std::{sync::Arc, task::Poll};

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use hyparview::message::{DisconnectMessage, ProtocolMessage};
use quinn::{Connection, Endpoint};
use rootcause::{Report, prelude::ResultExt};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
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
    command_rx: UnboundedReceiver<Command<M>>,
    handle: ServerHandle<M>,
}

impl<M: MessagePayload> P2PNodeServer<M> {
    pub fn new(endpoint: Endpoint, server_name: String) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let handle = ServerHandle {
            server_name,
            endpoint,
            local_nodes: Arc::new(DashMap::new()),
            remote_nodes: Arc::new(DashMap::new()),
            command_tx,
        };
        P2PNodeServer { command_rx, handle }
    }

    pub fn handle(&self) -> ServerHandle<M> {
        self.handle.clone()
    }
}

impl<M: MessagePayload> Future for P2PNodeServer<M> {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let server = self.get_mut();
        // 只处理当前可用的命令，避免无限循环
        if let Poll::Ready(Some(command)) = server.command_rx.poll_recv(cx) {
            debug!("P2PNodeServer fetch a command: {command:?}");
            match command {
                Command::Register(node_handle) => {
                    if server
                        .handle
                        .local_nodes
                        .contains_key(&node_handle.local_id)
                    {
                        warn!("Local node has been registered"); // 修复拼写错误
                    } else {
                        info!("Registers a local node: {:?}", node_handle.local_id);
                        server
                            .handle
                            .local_nodes
                            .insert(node_handle.local_id.clone(), node_handle);
                    }
                }
                Command::Deregister(local_node_id) => {
                    info!("Deregisters a local node: {:?}", local_node_id);
                    server.handle.local_nodes.remove(&local_node_id);
                }
            }
            // 有命令处理后，立即重新调度以处理更多命令
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        } else {
            // 如果没有更多命令待处理，则返回 Pending，等待新命令时由接收器唤醒
            std::task::Poll::Pending
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerHandle<M: MessagePayload> {
    server_name: String,
    endpoint: Endpoint,
    local_nodes: Arc<DashMap<LocalNodeId, NodeHandle<M>>>,
    command_tx: UnboundedSender<Command<M>>,
    remote_nodes: Arc<DashMap<LocalNodeId, Connection>>,
}

impl<M: MessagePayload> ServerHandle<M> {

    /// 启动quic客户端
    pub async fn start_server(&self) -> Result<(), Report> {
        info!("Server listen on: {}", self.endpoint.local_addr()?);
        // 接受传入连接
        loop {
            // 接受新连接
            match self.endpoint.accept().await {
                Some(conn) => {
                    let connection = conn.await?;
                    let handle = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle.handle_connection(connection).await {
                            error!("Handle connection err: {e}");
                        }
                    });
                }
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
    async fn handle_connection(&self, conn: Connection) -> Result<(), Report> {
        loop {
            let (_tx, rx) = conn.accept_bi().await?;
            let mut framed_reader = FramedRead::new(rx, LengthDelimitedCodec::new());
            while let Some(bytes_mut) = framed_reader.next().await {
                let bytes = bytes_mut?;
                let p2p_node_protocol_message: P2pNodeProtocolMessage<M> =
                    serde_json::from_slice(&bytes)?;
                debug!("Fetch a message: {p2p_node_protocol_message:?}");

                match self.local_nodes.iter().last() {
                    Some(node) => {
                        node.message_tx.send(p2p_node_protocol_message)?;
                    }
                    None => {
                        warn!("Local nodes is Empty")
                    }
                }
            }
        }
    }

    /// 注册本地的节点
    pub(crate) fn register_local_node(&self, node: NodeHandle<M>) {
        let command = Command::Register(node);
        let _ = self.command_tx.send(command);
    }

    pub(crate) fn deregister_local_node(&self, node: LocalNodeId) {
        let command = Command::Deregister(node);
        let _ = self.command_tx.send(command);
    }

    pub fn remove_remote_node(&self, node: &LocalNodeId) {
        self.remote_nodes.remove(node);
    }

    async fn get_connection(&self, nodeid: &NodeId) -> Result<Connection, Report> {
        let addr = nodeid.address();
        let conn = if let Some(conn) = self.remote_nodes.get(nodeid.local_id()) {
            conn.clone()
        } else {
            // 发起连接
            let connection = self
                .endpoint
                .connect(
                    addr,
                    &self.server_name, // 必须与证书中的域名匹配
                )?
                .await
                .context(format!("连接到节点: {addr}出错"))?;
            log::info!("Connected to server: {:?}", connection.remote_address());
            self.remote_nodes
                .insert(nodeid.local_id().clone(), connection.clone());
            connection
        };
        Ok(conn)
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
                handle.notify_disconnect(&destination);
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
    pub async fn send_protocol_message_async(
        &self,
        destination: NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        let protocol_message = serde_json::to_vec(&protocol_message)?;
        let connection = self.get_connection(&destination).await?;
        let (tx, _rx) = connection.open_bi().await?;
        let mut framed_writer = FramedWrite::new(tx, LengthDelimitedCodec::new());
        framed_writer.send(Bytes::from(protocol_message)).await?;
        Ok(())
    }

    /// 提醒断联
    /// 发送给本地节点，destination需要断联
    fn notify_disconnect(&self, destination: &NodeId) {
        // 无法发送消息，发送断联
        let message = DisconnectMessage {
            sender: destination.clone(),
            alive: false,
        };
        let message = ProtocolMessage::Disconnect(message);
        let message = P2pNodeProtocolMessage::Hyparview(message);
        if !self.local_nodes.is_empty() {
            let _ = self
                .local_nodes
                .iter()
                .last()
                .expect("never fails")
                .message_tx
                .send(message);
            self.remove_remote_node(destination.local_id());
        }
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
