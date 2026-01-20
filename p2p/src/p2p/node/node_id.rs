use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
    ops::Deref,
    str::FromStr,
};

use data_encoding::DecodeError;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, Signature, SignatureError, SigningKey, VerifyingKey,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeyParsingError {
    #[error("DecodeError")]
    DecodeError {
        #[from]
        err: DecodeError,
    },
    #[error("SignatureError")]
    SignatureError {
        #[from]
        err: SignatureError,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        self.0.as_bytes()
    }
}

impl Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.debug_tuple("PublicKey").field(&self.0).finish()
        write!(
            f,
            "PublicKey({})",
            data_encoding::HEXLOWER.encode(self.as_bytes())
        )
    }
}

impl Deref for PublicKey {
    type Target = VerifyingKey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialOrd for PublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PublicKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl TryFrom<&[u8; 32]> for PublicKey {
    type Error = SignatureError;

    #[inline]
    fn try_from(bytes: &[u8; 32]) -> Result<Self, Self::Error> {
        Ok(Self(VerifyingKey::from_bytes(bytes)?))
    }
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl FromStr for PublicKey {
    type Err = KeyParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        match data_encoding::HEXLOWER.decode_mut(s.as_bytes(), &mut bytes) {
            Ok(_) => {}
            Err(err) => {
                return Err(KeyParsingError::DecodeError { err: err.error });
            }
        };
        Ok(Self::try_from(&bytes)?)
    }
}

#[derive(Debug, Clone)]
pub struct SecretKey(SigningKey);
impl SecretKey {
    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    pub fn secret(&self) -> &SigningKey {
        &self.0
    }

    pub fn generate() -> Self {
        let mut secret_key_bytes: [u8; SECRET_KEY_LENGTH] = [0; SECRET_KEY_LENGTH];
        let mut rng = rand::rng();
        for i in 0..SECRET_KEY_LENGTH {
            secret_key_bytes[i] = rng.random::<u8>();
        }
        Self(SigningKey::from_bytes(&secret_key_bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LocalNodeId(PublicKey);

impl LocalNodeId {
    /// Makes a new `LocalNodeId` instance.
    pub fn new(id: PublicKey) -> Self {
        LocalNodeId(id)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    pub fn short(&self) -> String {
        self.to_string().split_at(5).0.to_string()
    }

    /// Returns the [`VerifyingKey`] for this `PublicKey`.
    pub fn public(&self) -> PublicKey {
        self.0
    }

    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), SignatureError> {
        self.public().verify_strict(message, signature)
    }

    pub const LENGTH: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;
}

impl Deref for LocalNodeId {
    type Target = PublicKey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for LocalNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", data_encoding::HEXLOWER.encode(self.as_bytes()))
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
