use std::sync::Arc;

use ed25519_dalek::{VerifyingKey, pkcs8::EncodePublicKey};
use log::warn;
use rcgen::Certificate;
use rustls::{
    CertificateError, SignatureScheme, SupportedProtocolVersion,
    client::danger::ServerCertVerifier,
    crypto::{WebPkiSupportedAlgorithms, verify_tls13_signature_with_raw_key},
    pki_types::SubjectPublicKeyInfoDer,
    server::danger::ClientCertVerifier,
};
pub static PROTOCOL_VERSIONS: &[&SupportedProtocolVersion] = &[&rustls::version::TLS13];

#[derive(Debug)]
pub struct ServerCertificateVerifier;

fn public_key_to_spki(remote_peer_id: &VerifyingKey) -> SubjectPublicKeyInfoDer<'static> {
    let der_key = remote_peer_id.to_public_key_der().expect("valid key");
    SubjectPublicKeyInfoDer::from(der_key.into_vec())
}

impl ServerCertificateVerifier {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl ServerCertVerifier for ServerCertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let rustls::pki_types::ServerName::DnsName(dns_name) = server_name else {
            return Err(rustls::Error::UnsupportedNameType);
        };

        let Some(remote_peer_id) = super::name::decode(dns_name.as_ref()) else {
            return Err(rustls::Error::InvalidCertificate(
                CertificateError::NotValidForName,
            ));
        };

        if !intermediates.is_empty() {
            return Err(rustls::Error::InvalidCertificate(
                CertificateError::UnknownIssuer,
            ));
        }

        let end_entity_as_spki = SubjectPublicKeyInfoDer::from(end_entity.as_ref());

        if public_key_to_spki(&remote_peer_id) != end_entity_as_spki {
            return Err(rustls::Error::InvalidCertificate(
                CertificateError::UnknownIssuer,
            ));
        }

        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature_with_raw_key(
            message,
            &SubjectPublicKeyInfoDer::from(cert.as_ref()),
            dss,
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        // vec![
        //     rustls::SignatureScheme::RSA_PKCS1_SHA256,
        //     rustls::SignatureScheme::ED25519,
        //     rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
        // ]
        rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
    
    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

#[derive(Default, Debug)]
pub struct ClientCertificateVerifier;

impl ClientCertificateVerifier {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

/// We requires either following of X.509 client certificate chains:
///
/// - a valid raw public key configuration
impl ClientCertVerifier for ClientCertificateVerifier {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &[][..] // 原始公钥不需要根证书提示
    }

    fn verify_client_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        // 对于原始公钥，不应该有中间证书
        if !intermediates.is_empty() {
            return Err(rustls::Error::InvalidCertificate(
                CertificateError::UnknownIssuer,
            ));
        }

        Ok(rustls::server::danger::ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // TLS 1.2 不支持原始公钥
        Err(rustls::Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // 使用原始公钥验证
        verify_tls13_signature_with_raw_key(
            message,
            &SubjectPublicKeyInfoDer::from(cert.as_ref()),
            dss,
            // todo 修改为某些指定的
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}
