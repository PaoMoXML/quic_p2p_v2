use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct PeerViewModel {
    pub peers: HashSet<String>,
}

impl PeerViewModel {
    pub fn addPeer(&mut self, peer: String) -> bool {
        self.peers.insert(peer)
    }

    pub fn removePeer(&mut self, peer: &str) -> bool {
        self.peers.remove(peer)
    }
}
