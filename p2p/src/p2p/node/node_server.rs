use std::{sync::Arc, task::Poll};

use dashmap::DashMap;
use quinn::{Connection, Endpoint};
use rootcause::{Report, prelude::ResultExt};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
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
            tracing::info!("P2PNodeServer fetch message: {command:?}");
            match command {
                Command::Register(node_handle) => {
                    if server
                        .handle
                        .local_nodes
                        .contains_key(&node_handle.local_id)
                    {
                        warn!("Local node has been registered"); // 修复拼写错误
                    } else {
                        info!("Registers a local node: {:?}", node_handle);
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

    async fn handle_connection(&self, conn: Connection) -> Result<(), Report> {
        loop {
            let (_tx, mut rx) = conn.accept_bi().await?;
            if let Ok(items) = rx.read_to_end(10480).await {
                let p2p_node_protocol_message: P2pNodeProtocolMessage<M> =
                    serde_json::from_slice(&items)?;
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
        Ok(())
    }

    pub(crate) fn register_local_node(&self, node: NodeHandle<M>) {
        let command = Command::Register(node);
        let _ = self.command_tx.send(command);
    }

    pub(crate) fn deregister_local_node(&self, node: LocalNodeId) {
        let command = Command::Deregister(node);
        let _ = self.command_tx.send(command);
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
        return Ok(conn);
    }

    pub fn send_protocol_message(
        &self,
        destination: NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        let server = self.clone();
        tokio::spawn(async move {
            if let Err(e) = server
                .do_send_protocol_message(destination.clone(), protocol_message)
                .await
            {
                warn!("Quinn cannot send message to {destination:?}: {e}");
            };
        });
        Ok(())
    }

    async fn do_send_protocol_message(
        &self,
        destination: NodeId,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        let protocol_message = serde_json::to_vec(&protocol_message)?;
        let connection = self.get_connection(&destination).await?;
        let (mut tx, _rx) = connection.open_bi().await?;
        tx.write_all(&protocol_message[..]).await?;
        tx.finish()?;
        Ok(())
    }
}

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
