use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId {
    address: SocketAddr,
    local_id: String,
}

impl NodeId {
    /// Makes a new `NodeId` instance.
    pub fn new(address: SocketAddr, local_id: String) -> Self {
        NodeId { address, local_id }
    }

    /// Returns the RPC server address part of the identifier.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Returns the local node identifier part of the identifier.
    pub fn local_id(&self) -> &str {
        &self.local_id
    }
}
