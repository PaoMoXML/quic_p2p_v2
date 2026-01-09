use std::task::Poll;

use clap::Parser;
use futures::StreamExt;
use rootcause::Report;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug_span, warn};
use tracing_subscriber::{
    fmt::{time, writer::MakeWriterExt},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

use crate::{
    chat::{ChatMessage, ChatNode},
    p2p::node::{
        P2PNode,
        args::Args,
        message::MessagePayload,
        node_id::{LocalNodeId, NodeId},
        node_server::P2PNodeServer,
        uuid,
    },
};

mod chat;
mod p2p;

#[tokio::main]
async fn main() -> Result<(), Report> {
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_timer(time::LocalTime::rfc_3339())
        .with_ansi(true)
        .with_writer(std::io::stderr.with_max_level(tracing::Level::INFO));

    tracing_subscriber::registry().with(stderr_layer).init();
    let span = debug_span!("Init");
    let _enter = span.enter();

    let args = Args::parse();
    let addr = args.local_addr;
    let endpoint = p2p::node::create_endpoint(addr)?;
    let local_id = LocalNodeId::new(uuid());
    let node_id = NodeId::new(endpoint.local_addr()?, local_id.clone());
    let server = P2PNodeServer::new(endpoint, args.server_name);
    let mut p2pnode = P2PNode::<ChatMessage>::new(node_id, server.handle())?;

    if let Some(remote_addr) = args.remote_addr {
        p2pnode.join(NodeId::new(remote_addr, LocalNodeId::new(uuid())));
    }

    let (tx, rx) = mpsc::unbounded_channel();
    let token = CancellationToken::new();
    let cloned_token = token.clone();
    let chat_node = ChatNode::new(p2pnode, rx, cloned_token);

    let handle = server.handle();
    tokio::spawn(server);
    tokio::spawn(chat_node);

    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            let user = local_id.clone();
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let msg = ChatMessage::new(
                        chrono::Local::now(),
                        line.trim().to_string(),
                        chat::UserType::User(user),
                    );
                    if tx.send(msg).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    warn!("Error reading stdin: {}", e);
                    break;
                }
            }
        }
    });

    // tokio::signal::ctrl_c().await?;
    token.cancelled().await;
    handle.stop_server().await?;
    Ok(())
}
