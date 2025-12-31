use std::{net::SocketAddr, sync::Arc};

use crypto::{digest::Digest, sha2::Sha256};
use dashmap::DashMap;
use futures::{Stream, StreamExt};
use quinn::{
    Connection, Endpoint, ServerConfig,
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rootcause::Report;
use rustls::{
    RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, pem::PemObject},
};
use tracing::{debug, info};

use crate::p2p::node::{
    message::{MessageId, MessagePayload},
    misc::{HyparviewAction, HyparviewNode, PlumtreeAction, PlumtreeAppMessage, PlumtreeNode},
    node_id::NodeId,
};

pub mod message;
mod misc;
mod node_client;
pub mod node_id;
mod node_server;

#[derive(Debug)]
pub struct P2PNode<M: MessagePayload> {
    hyparview_node: HyparviewNode,
    plumtree_node: PlumtreeNode<M>,
    endpoint: Endpoint,
    connections: DashMap<NodeId, Connection>,
    message_seqno: u64,
}

impl<M: MessagePayload> P2PNode<M> {
    pub fn new(endpoint: Endpoint) -> Result<Self, Report> {
        let node_id = NodeId::new(endpoint.local_addr()?, uuid());
        Ok(Self {
            hyparview_node: hyparview::Node::new(
                node_id.clone(),
                StdRng::from_seed(rand::rng().random()),
            ),
            plumtree_node: plumtree::Node::new(node_id),
            endpoint: endpoint,
            connections: DashMap::new(),
            message_seqno: 0,
        })
    }

    /// Joins the cluster to which the given contact node belongs.
    pub fn join(&mut self, contact_node: NodeId) {
        info!("Joins a cluster by contacting to {:?}", contact_node);
        self.hyparview_node.join(contact_node);
    }

    /// Broadcasts a message.
    ///
    /// Note that the message will also be delivered to the sender node.
    pub fn broadcast(&mut self, message_payload: M) -> MessageId {
        let id = MessageId::new(self.id(), self.message_seqno);
        self.message_seqno += 1;
        debug!("Starts broadcasting a message: {:?}", id);

        let m = PlumtreeAppMessage {
            id: id.clone(),
            payload: message_payload,
        };
        self.plumtree_node.broadcast_message(m);
        id
    }

    /// Returns the identifier of the node.
    pub fn id(&self) -> NodeId {
        self.plumtree_node().id().clone()
    }

    /// Returns a reference to the underlying Plumtree node.
    pub fn plumtree_node(&self) -> &PlumtreeNode<M> {
        &self.plumtree_node
    }

    fn handle_hyparview_action(&mut self, action: HyparviewAction) {
        tracing::info!(HyparviewAction=?action);
        match action {
            hyparview::Action::Send {
                destination,
                message,
            } => {}
            hyparview::Action::Disconnect { node } => todo!(),
            hyparview::Action::Notify { event } => todo!(),
        }
    }

    fn handle_plumtree_action(
        &mut self,
        action: PlumtreeAction<M>,
    ) -> Option<PlumtreeAppMessage<M>> {
        tracing::info!(PlumtreeAction=?action);
        match action {
            plumtree::Action::Send {
                destination,
                message,
            } => match message {
                plumtree::message::ProtocolMessage::Gossip(gossip_message) => todo!(),
                plumtree::message::ProtocolMessage::Ihave(ihave_message) => todo!(),
                plumtree::message::ProtocolMessage::Graft(graft_message) => todo!(),
                plumtree::message::ProtocolMessage::Prune(prune_message) => todo!(),
            },
            plumtree::Action::Deliver { message } => todo!(),
        }
    }

    pub async fn run(&mut self) {
        while let Some(next) = self.next().await {}
    }
}

impl<M: MessagePayload> Stream for P2PNode<M> {
    type Item = PlumtreeAppMessage<M>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let node = self.get_mut();
        let action = node.hyparview_node.poll_action();
        if let Some(action) = action {
            node.handle_hyparview_action(action);
        }

        if let Some(action) = node.plumtree_node.poll_action() {
            if let Some(message) = node.handle_plumtree_action(action) {
                return std::task::Poll::Ready(Some(message));
            };
        }

        std::task::Poll::Pending
    }
}

pub fn uuid() -> String {
    let mut hasher = Sha256::new();
    let mut rng = rand::rng();
    let r: [u8; 32] = rng.random();
    hasher.input(&r);
    hasher.result_str()
}

/// 创建端点
pub fn create_endpoint(addr: SocketAddr) -> Result<Endpoint, Report> {
    let cert = CertificateDer::from_pem_file("./cert.pem").unwrap();
    let key = PrivatePkcs8KeyDer::from_pem_file("./key.pem").unwrap();
    // 配置服务器TLS
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.to_owned()], PrivateKeyDer::Pkcs8(key))?;

    // 创建QUIC服务器配置
    let mut server_config =
        ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_config)?));
    // 启用QUIC keep-alive
    if let Some(transport_config) = Arc::get_mut(&mut server_config.transport) {
        transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(2)));
    }

    // 验证服务器
    let mut certs = RootCertStore::empty();
    // 从文件读取pem转换为der
    certs.add(cert)?;
    let client_config = rustls::client::ClientConfig::builder()
        .with_root_certificates(certs)
        .with_no_client_auth();

    let mut endpoint = Endpoint::client(addr)?;
    endpoint.set_server_config(Some(server_config));
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(
        QuicClientConfig::try_from(client_config)?,
    )));

    Ok(endpoint)
}
