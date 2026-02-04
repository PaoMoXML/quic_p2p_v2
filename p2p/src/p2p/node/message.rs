use std::fmt::Debug;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::p2p::node::{misc::PlumtreeSystem, node_id::NodeId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MessageId {
    node: NodeId,
    seqno: u64,
    timestamp: u64, // 添加全局时间戳字段
}
impl MessageId {
    /// Returns the node identifier part of the message identifier.
    pub fn node(&self) -> NodeId {
        self.node.clone()
    }

    /// Returns the sequence number part of the message identifier.
    pub fn seqno(&self) -> u64 {
        self.seqno
    }

    /// Returns the timestamp part of the message identifier.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
    pub(crate) fn new(node: NodeId, seqno: u64, timestamp: u64) -> Self {
        MessageId {
            node,
            seqno,
            timestamp,
        }
    }
}

pub trait MessagePayload:
    Sized + Clone + Send + Sync + 'static + std::marker::Unpin + Debug + Serialize + DeserializeOwned
{
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum P2pNodeProtocolMessage<T: Clone> {
    Hyparview(hyparview::message::ProtocolMessage<NodeId>),
    Plumtree(plumtree::message::ProtocolMessage<PlumtreeSystem<T>>),
}
