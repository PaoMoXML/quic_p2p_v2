use quinn::Connection;
use rootcause::{Report, prelude::ResultExt};

use crate::p2p::node::{
    P2PNode,
    message::{MessagePayload, P2pNodeProtocolMessage},
    node_id::NodeId,
};

impl<M: MessagePayload> P2PNode<M> {
    async fn get_connection(&self, nodeid: NodeId) -> Result<Connection, Report> {
        let addr = nodeid.address();
        let conn = if let Some(conn) = self.connections.get(&nodeid) {
            conn.clone()
        } else {
            // 发起连接
            let connection = self
                .endpoint
                .connect(
                    addr,
                    self.hyparview_node.id().local_id(), // 必须与证书中的域名匹配
                )?
                .await
                .context(format!("连接到节点: {addr}出错"))?;
            log::info!("Connected to server: {:?}", connection.remote_address());
            self.connections.insert(nodeid, connection.clone());
            connection
        };
        return Ok(conn);
    }

    pub async fn send_message(
        &self,
        protocol_message: P2pNodeProtocolMessage<M>,
    ) -> Result<(), Report> {
        match protocol_message {
            P2pNodeProtocolMessage::Hyparview(protocol_message) => {
                match protocol_message {
                    hyparview::message::ProtocolMessage::Join(join_message) => {
                    },
                    hyparview::message::ProtocolMessage::ForwardJoin(forward_join_message) => {
                    }
                    hyparview::message::ProtocolMessage::Neighbor(neighbor_message) => todo!(),
                    hyparview::message::ProtocolMessage::Shuffle(shuffle_message) => todo!(),
                    hyparview::message::ProtocolMessage::ShuffleReply(shuffle_reply_message) => todo!(),
                    hyparview::message::ProtocolMessage::Disconnect(disconnect_message) => todo!(),
                }
            },
            P2pNodeProtocolMessage::Plumtree(protocol_message) => todo!(),
        }
        Ok(())
    }
}
