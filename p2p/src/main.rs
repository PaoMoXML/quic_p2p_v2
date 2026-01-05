use std::task::Poll;

use futures::StreamExt;
use rootcause::Report;
use tokio::sync::mpsc;
use tracing::{debug_span, warn};
use tracing_subscriber::{
    fmt::{time, writer::MakeWriterExt},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

use crate::p2p::node::{
    P2PNode,
    message::MessagePayload,
    node_id::{LocalNodeId, NodeId},
    node_server::P2PNodeServer,
    uuid,
};

mod p2p;
#[tokio::main]
async fn main() -> Result<(), Report> {
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_timer(time::LocalTime::rfc_3339())
        .with_ansi(true)
        .with_writer(std::io::stderr.with_max_level(tracing::Level::DEBUG));

    tracing_subscriber::registry().with(stderr_layer).init();

    let addr = "127.0.0.1:8002";
    let span = debug_span!("init", address = addr);
    let _enter = span.enter();
    let endpoint = p2p::node::create_endpoint(addr.parse()?)?;
    let local_id = LocalNodeId::new(uuid());
    let node_id = NodeId::new(endpoint.local_addr()?, local_id.clone());
    let server = P2PNodeServer::new(endpoint, "localhost".to_string());
    let mut p2pnode = P2PNode::<String>::new(node_id, server.handle())?;

    p2pnode.join(NodeId::new(
        "127.0.0.1:8001".parse()?,
        LocalNodeId::new(uuid()),
    ));

    let (tx, rx) = mpsc::unbounded_channel();
    let chat_node = ChatNode {
        inner: p2pnode,
        message_rx: rx,
    };

    let han = server.handle();
    tokio::spawn(async move {
        let span = debug_span!("run", address = addr);
        let _enter = span.enter();
        if let Err(e) = han.start_server().await {
            warn!("start server err: {e:?}");
        };
    });

    tokio::spawn(async move {
        let span = debug_span!("server", address = addr);
        let _enter = span.enter();
        server.await;
    });

    tokio::spawn(async move {
        let span = debug_span!("start_server", address = addr);
        let _enter = span.enter();
        chat_node.await;
    });

    tokio::spawn(async move {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let line = if let Ok(line) = line {
                line
            } else {
                break;
            };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    tokio::signal::ctrl_c().await?;
    Ok(())
}

impl MessagePayload for String {}

struct ChatNode {
    inner: P2PNode<String>,
    message_rx: mpsc::UnboundedReceiver<String>,
}

impl Future for ChatNode {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let span = debug_span!("ChatNode poll", address = "127.0.0.1:8002");
        let _enter = span.enter();

        let chat_node = self.get_mut();
        let mut processed_messages = false;

        // 处理来自内部P2P节点的消息
        while let Poll::Ready(Some(m)) = chat_node.inner.poll_next_unpin(cx) {
            println!("# MESSAGE: {:?}", m);
            processed_messages = true;
        }

        // 处理来自消息接收器的消息
        while let Poll::Ready(Some(msg)) = chat_node.message_rx.poll_recv(cx) {
            chat_node.inner.broadcast(msg);
            processed_messages = true;
        }

        // 如果处理了消息，需要重新调度以检查是否有更多消息
        // 但为了避免繁忙等待，我们只在有新消息时才唤醒
        if processed_messages {
            cx.waker().wake_by_ref();
        }

        Poll::Pending
    }
}
