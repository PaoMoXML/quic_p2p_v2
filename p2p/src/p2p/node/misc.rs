use std::marker::PhantomData;

use rand::rngs::StdRng;

use crate::p2p::node::{message::MessageId, node_id::NodeId};

pub type HyparviewNode = hyparview::Node<NodeId, StdRng>;
pub(crate) type HyparviewAction = hyparview::Action<NodeId>;

pub type PlumtreeNode<M> = plumtree::Node<PlumtreeSystem<M>>;
pub(crate) type PlumtreeAction<M> = plumtree::Action<PlumtreeSystem<M>>;
pub(crate) type PlumtreeAppMessage<M> = plumtree::message::Message<PlumtreeSystem<M>>;

#[derive(Debug, Clone)]

pub struct PlumtreeSystem<M>(PhantomData<M>);

impl<M: Clone> plumtree::System for PlumtreeSystem<M> {
    type NodeId = NodeId;
    type MessageId = MessageId;
    type MessagePayload = M;
}
