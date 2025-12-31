use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::p2p::node::{misc::PlumtreeSystem, node_id::NodeId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MessageId {
    node: NodeId,
    seqno: u64,
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

    pub(crate) fn new(node: NodeId, seqno: u64) -> Self {
        MessageId { node, seqno }
    }
}

pub trait MessagePayload:
    Sized + Clone + Send + 'static + std::marker::Unpin + Debug + Serialize + Deserialize<'static>
{
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum P2pNodeProtocolMessage<T: Clone> {
    Hyparview(hyparview::message::ProtocolMessage<T>),
    Plumtree(plumtree::message::ProtocolMessage<PlumtreeSystem<T>>),
}
