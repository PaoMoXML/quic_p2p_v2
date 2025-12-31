use rootcause::Report;
use tracing_subscriber::{
    fmt::{time, writer::MakeWriterExt},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

use crate::p2p::node::{P2PNode, message::MessagePayload, node_id::NodeId, uuid};

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

    let endpoint = p2p::node::create_endpoint("127.0.0.1:8001".parse()?)?;
    let mut p2pnode = P2PNode::<String>::new(endpoint)?;
    // p2pnode.run().await;
    p2pnode.join(NodeId::new("127.0.0.1:8002".parse()?, uuid()));
    p2pnode.broadcast("message_payload".into());
    tokio::spawn(async move {
        p2pnode.run().await;
    });

    tokio::signal::ctrl_c().await?;

    Ok(())
}

impl MessagePayload for String {}
