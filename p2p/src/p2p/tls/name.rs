use data_encoding::BASE32_DNSSEC;
use ed25519_dalek::VerifyingKey;
use tracing::{debug, warn};

use crate::p2p::node::node_id::PublicKey;

pub(crate) fn encode(node_id: PublicKey) -> String {
    format!(
        "{}.upaomo.invalid",
        BASE32_DNSSEC.encode(node_id.as_bytes())
    )
}

pub(crate) fn decode(name: &str) -> Option<VerifyingKey> {
    debug!("name: {}", name);
    let [base32_node_id, "upaomo", "invalid"] = name.split(".").collect::<Vec<_>>()[..] else {
        return None;
    };
    let bytes = BASE32_DNSSEC.decode(base32_node_id.as_bytes()).ok()?;
    VerifyingKey::from_bytes(&bytes.try_into().ok()?).ok()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{SECRET_KEY_LENGTH, SigningKey};
    use rand::Rng;
    use rootcause::Report;
    #[test]
    fn test_sign() -> Result<(), Report> {
        let msg = "hello world".as_bytes();
        let msg2 = "hello world2".as_bytes();

        let mut secret_key_bytes: [u8; SECRET_KEY_LENGTH] = [0; SECRET_KEY_LENGTH];
        let mut rng = rand::rng();
        for i in 0..SECRET_KEY_LENGTH {
            secret_key_bytes[i] = rng.random::<u8>();
        }
        let signing_key = SigningKey::from_bytes(&secret_key_bytes);
        let public_key = signing_key.verifying_key();
        println!(
            "public_key: {}",
            data_encoding::HEXLOWER.encode(public_key.as_bytes())
        );
        let secret_key = signing_key.to_bytes();
        println!(
            "secret_key: {}",
            data_encoding::HEXLOWER.encode(&secret_key)
        );

        Ok(())
    }
}
