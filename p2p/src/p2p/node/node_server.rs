use quinn::Connection;
use rootcause::Report;

use crate::p2p::node::{P2PNode, message::MessagePayload};

impl<M: MessagePayload> P2PNode<M> {
    pub async fn start_server(&self) -> Result<(), Report> {
        log::info!("Server listen on: {}", self.endpoint.local_addr()?);
        log::info!("Your peer id: {}", self.hyparview_node.id().local_id());

        // 接受传入连接
        loop {
            tokio::select! {
                // 接受新连接
                conn = self.endpoint.accept() => {
                    match conn {
                        Some(conn) => {
                            let connection = conn.await?;
                            self.handle_connection(connection)?;
                        }
                        None => break,
                    }
                }
            }
        }
        Ok(())
    }

    pub fn handle_connection(&self, conn: Connection) -> Result<(), Report> {
        tokio::spawn(async move {
            let remote_addr = conn.remote_address();
        });
        Ok(())
    }
}
