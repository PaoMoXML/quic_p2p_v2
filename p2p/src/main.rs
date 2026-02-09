use std::str::FromStr;

use clap::Parser;
use rootcause::Report;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
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
        node_id::{LocalNodeId, NodeId, PublicKey, SecretKey},
        node_server::P2PNodeServer,
    },
};

mod chat;
mod p2p;

const APLN: &[u8] = b"quic-p2p-v2";

#[tokio::main]
async fn main() -> Result<(), Report> {
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_timer(time::LocalTime::rfc_3339())
        .with_ansi(true)
        .with_writer(std::io::stderr.with_max_level(tracing::Level::INFO));

    tracing_subscriber::registry().with(stderr_layer).init();

    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let args = Args::parse();
    let secret = SecretKey::generate();
    let local_id = LocalNodeId::new(secret.public());

    let server = P2PNodeServer::<ChatMessage>::builder()
        .secret_key(secret)
        .alpn_protocols(vec![APLN.to_vec()])
        .bind(args.local_addr)
        .await?;
    let node_id = NodeId::new(server.get_addresses().iter().cloned().collect(), local_id);

    println!(
        "cargo run -- --connect-to={:?} -s={}",
        node_id
            .address()
            .iter()
            .map(|addr| addr.to_string())
            .collect::<Vec<_>>()
            .join(" "),
        local_id
    );

    let mut p2pnode = P2PNode::<ChatMessage>::new(node_id, server.handle())?;
    if let Some(remote_addr) = args.connect_to {
        p2pnode.join(NodeId::new(
            remote_addr,
            LocalNodeId::new(PublicKey::from_str(&args.server_name.unwrap())?),
        ));
    }

    let (tx, rx) = mpsc::channel(1);
    let token = CancellationToken::new();
    let cloned_token = token.clone();
    let chat_node = ChatNode::new(p2pnode, rx, cloned_token);

    let handle = server.handle();
    tokio::spawn(server);
    tokio::spawn(chat_node);
    std::thread::spawn(move || input_loop(tx, local_id));

    // tokio::spawn(async move {
    //     use tokio::io::{AsyncBufReadExt, BufReader};

    //     let stdin = tokio::io::stdin();
    //     let mut reader = BufReader::new(stdin);
    //     let mut line = String::new();

    //     loop {
    //         let user = local_id;
    //         line.clear();
    //         match reader.read_line(&mut line).await {
    //             Ok(0) => break, // EOF
    //             Ok(_) => {
    //                 let msg = ChatMessage::new(
    //                     chrono::Local::now(),
    //                     line.trim().to_string(),
    //                     chat::UserType::User(user),
    //                 );
    //                 if tx.send(msg).is_err() {
    //                     break;
    //                 }
    //             }
    //             Err(e) => {
    //                 warn!("Error reading stdin: {}", e);
    //                 break;
    //             }
    //         }
    //     }
    // });

    // tokio::signal::ctrl_c().await?;
    token.cancelled().await;
    handle.stop_server().await?;
    Ok(())
}

fn input_loop(
    line_tx: tokio::sync::mpsc::Sender<ChatMessage>,
    user: LocalNodeId,
) -> Result<(), Report> {
    let mut buffer = String::new();
    let stdin = std::io::stdin(); // We get `Stdin` here.
    loop {
        stdin.read_line(&mut buffer)?;
        let msg = ChatMessage::new(
            chrono::Local::now(),
            buffer.trim().to_string(),
            chat::UserType::User(user),
        );
        line_tx.blocking_send(msg)?;
        buffer.clear();
    }
}
