use std::{fmt::Display, net::SocketAddr, ops::Deref};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LocalNodeId(String);

impl LocalNodeId {
    /// Makes a new `LocalNodeId` instance.
    pub fn new(id: String) -> Self {
        LocalNodeId(id)
    }

    /// Returns the value of the identifier.
    pub fn value(self) -> String {
        self.0
    }
}

impl Deref for LocalNodeId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for LocalNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId {
    address: SocketAddr,
    local_id: LocalNodeId,
}

impl NodeId {
    /// Makes a new `NodeId` instance.
    pub fn new(address: SocketAddr, local_id: LocalNodeId) -> Self {
        NodeId { address, local_id }
    }

    /// Returns the RPC server address part of the identifier.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Returns the local node identifier part of the identifier.
    pub fn local_id(&self) -> &LocalNodeId {
        &self.local_id
    }
}
