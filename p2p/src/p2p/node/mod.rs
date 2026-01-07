use std::{net::SocketAddr, sync::Arc, task::Poll, thread::sleep, time::Duration};

use chrono::{DateTime, Utc};
use crypto::{digest::Digest, sha2::Sha256};
use futures::Stream;
use plumtree::time::NodeTime;
use quinn::{
    Endpoint, ServerConfig,
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rootcause::Report;
use rustls::{
    RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, pem::PemObject},
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tracing::{debug, info, warn};

use crate::p2p::node::{
    message::{MessageId, MessagePayload, P2pNodeProtocolMessage},
    misc::{HyparviewAction, HyparviewNode, PlumtreeAction, PlumtreeAppMessage, PlumtreeNode},
    node_id::NodeId,
    node_server::{NodeHandle, ServerHandle},
};

pub mod args;
pub mod message;
mod misc;
pub mod node_id;
pub mod node_server;

const TICK_FPS: f64 = 30.0;

#[derive(Debug)]
pub struct P2PNode<M: MessagePayload> {
    hyparview_node: HyparviewNode,
    plumtree_node: PlumtreeNode<M>,
    message_seqno: u64,
    server: ServerHandle<M>,
    message_rx: UnboundedReceiver<P2pNodeProtocolMessage<M>>,
    params: Parameters,
    hyparview_shuffle_time: NodeTime,
    hyparview_sync_active_view_time: NodeTime,
    hyparview_fill_active_view_time: NodeTime,
    last_tick_time: DateTime<Utc>,
}

impl<M: MessagePayload> P2PNode<M> {
    pub fn new(node_id: NodeId, server: ServerHandle<M>) -> Result<Self, Report> {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let node_handle = NodeHandle::new(node_id.local_id().clone(), message_tx);
        server.register_local_node(node_handle);
        let plumtree_node = plumtree::Node::new(node_id.clone());
        let now = plumtree_node.clock().now();
        let params = Parameters::default();
        let hyparview_shuffle_time = now + gen_interval(params.hyparview_shuffle_interval);
        let hyparview_sync_active_view_time =
            now + gen_interval(params.hyparview_sync_active_view_interval);
        let hyparview_fill_active_view_time =
            now + gen_interval(params.hyparview_fill_active_view_interval);

        Ok(Self {
            hyparview_node: hyparview::Node::new(node_id, StdRng::from_seed(rand::rng().random())),
            plumtree_node,
            message_seqno: 0,
            server,
            message_rx,
            params,
            hyparview_shuffle_time,
            hyparview_sync_active_view_time,
            hyparview_fill_active_view_time,
            last_tick_time: Utc::now(),
        })
    }

    /// 加入给定联系人节点所属的集群。
    pub fn join(&mut self, contact_node: NodeId) {
        info!("Joins a cluster by contacting to {:?}", contact_node);
        self.hyparview_node.join(contact_node);
    }

    /// 广播一条消息。
    ///
    /// 请注意，该消息也会发送给发送节点。
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

    /// 返回节点的标识符。
    pub fn id(&self) -> NodeId {
        self.plumtree_node().id().clone()
    }

    /// Returns a reference to the underlying Plumtree node.
    pub fn plumtree_node(&self) -> &PlumtreeNode<M> {
        &self.plumtree_node
    }

    fn handle_hyparview_action(&mut self, action: HyparviewAction) {
        match action {
            hyparview::Action::Send {
                destination,
                message,
            } => {
                debug!(
                    "Sends a HyParView message to {:?}: {:?}",
                    destination, message
                );
                self.server.send_protocol_message_sync(
                    destination.clone(),
                    P2pNodeProtocolMessage::Hyparview(message),
                );
            }
            hyparview::Action::Notify { event } => match event {
                hyparview::Event::NeighborUp { node } => {
                    info!(
                        "Neighbor up: {:?} (active_view={:?})",
                        node,
                        self.hyparview_node.active_view()
                    );
                    self.plumtree_node.handle_neighbor_up(&node);
                }
                hyparview::Event::NeighborDown { node } => {
                    info!(
                        "Neighbor down: {:?} (active_view={:?})",
                        node,
                        self.hyparview_node.active_view()
                    );
                    self.plumtree_node.handle_neighbor_down(&node);
                }
            },
            hyparview::Action::Disconnect { node } => {
                self.hyparview_node.disconnect(&node, false);
                self.server.remove_remote_node(node.local_id());
                info!("Disconnected: {:?}", node);
            }
        }
    }

    fn handle_plumtree_action(
        &mut self,
        action: PlumtreeAction<M>,
    ) -> Option<PlumtreeAppMessage<M>> {
        match action {
            plumtree::Action::Send {
                destination,
                message,
            } => {
                debug!("Sends a Plumtree message to {:?}", destination);
                self.server.send_protocol_message_sync(
                    destination.clone(),
                    P2pNodeProtocolMessage::Plumtree(message),
                );
                None
            }
            plumtree::Action::Deliver { message } => {
                debug!("Delivers an application message: {:?}", message.id);
                Some(PlumtreeAppMessage::from(message))
            }
        }
    }

    fn handle_p2pnode_protocol_message(&mut self, message: P2pNodeProtocolMessage<M>) -> bool {
        match message {
            P2pNodeProtocolMessage::Hyparview(m) => {
                debug!("Received a HyParView message: {:?}", m);
                self.hyparview_node.handle_protocol_message(m);
                true
            }
            P2pNodeProtocolMessage::Plumtree(m) => {
                debug!("Received a Plumtree message");
                if !self.plumtree_node.handle_protocol_message(m) {
                    warn!("Unknown plumtree node errors")
                }
                false
            }
        }
    }

    fn handle_tick(&mut self) {
        self.plumtree_node
            .clock_mut()
            .tick(self.params.tick_interval);

        let now = self.plumtree_node.clock().now();
        if now >= self.hyparview_shuffle_time {
            self.hyparview_node.shuffle_passive_view();
            self.hyparview_shuffle_time =
                now + gen_interval(self.params.hyparview_shuffle_interval);
        }
        if now >= self.hyparview_sync_active_view_time {
            self.hyparview_node.sync_active_view();
            self.hyparview_sync_active_view_time =
                now + gen_interval(self.params.hyparview_sync_active_view_interval);
        }
        if now >= self.hyparview_fill_active_view_time {
            self.hyparview_node.fill_active_view();
            self.hyparview_fill_active_view_time =
                now + gen_interval(self.params.hyparview_fill_active_view_interval);
        }
    }

    pub fn leave(&mut self) {
        use hyparview::message::{DisconnectMessage, ProtocolMessage};

        info!(
            "Leaves the current cluster: active_view={:?}",
            self.hyparview_node.active_view()
        );
        for peer in self.hyparview_node.active_view().iter().cloned() {
            let message = DisconnectMessage {
                sender: self.id(),
                alive: false,
            };
            let message = ProtocolMessage::Disconnect(message);
            let message = P2pNodeProtocolMessage::Hyparview(message);
            self.server.notify_self_disconnect(&peer);
            if let Err(e) = self.server.send_protocol_message(peer, message) {
                warn!("Leave err: {e}");
            }
        }
    }
}

// todo 好像没有执行完就结束了
// impl<M: MessagePayload> Drop for P2PNode<M> {
//     fn drop(&mut self) {
//         self.leave();
//     }
// }

impl<M: MessagePayload> Stream for P2PNode<M> {
    type Item = PlumtreeAppMessage<M>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let node = self.get_mut();
        let now = Utc::now();
        if now < node.last_tick_time + node.params.tick_interval {
            let x = now + node.params.tick_interval - node.last_tick_time;            
            sleep(Duration::from_millis(x.num_milliseconds().unsigned_abs()));
        }
        node.handle_tick();

        let mut did_something = true;
        while did_something {
            did_something = false;
            while let Some(action) = node.hyparview_node.poll_action() {
                node.handle_hyparview_action(action);
                did_something = true;
            }

            while let Some(action) = node.plumtree_node.poll_action() {
                if let Some(message) = node.handle_plumtree_action(action) {
                    return Poll::Ready(Some(message));
                }
                did_something = true;
            }

            while let Poll::Ready(Some(message)) = node.message_rx.poll_recv(cx) {
                did_something = true;
                if node.handle_p2pnode_protocol_message(message) {
                    break;
                }
            }
        }
        node.last_tick_time = Utc::now();
        cx.waker().wake_by_ref();
        Poll::Pending
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

#[derive(Debug, Clone)]
struct Parameters {
    tick_interval: Duration,
    hyparview_shuffle_interval: Duration,
    hyparview_sync_active_view_interval: Duration,
    hyparview_fill_active_view_interval: Duration,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_secs_f64(1.0 / TICK_FPS),
            hyparview_shuffle_interval: Duration::from_secs(300),
            hyparview_sync_active_view_interval: Duration::from_secs(60),
            hyparview_fill_active_view_interval: Duration::from_secs(30),
        }
    }
}

fn gen_interval(base: Duration) -> Duration {
    let millis = base.as_secs() * 1000 + u64::from(base.subsec_millis());
    let jitter = rand::random::<u64>() % (millis / 10);
    base + Duration::from_millis(jitter)
}

#[test]
fn test() {}
