use std::sync::Arc;

use quinn::{
    ClientConfig, ServerConfig,
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
};
use rootcause::Report;

use crate::p2p::{node::node_id::SecretKey, tls::resolver::AlwaysResolvesCert};

pub mod name;
pub mod resolver;
pub mod verifier;

#[derive(Debug)]
pub(crate) struct TlsConfig {
    secret_key: SecretKey,
    cert_resolver: Arc<AlwaysResolvesCert>,
    server_verifier: Arc<verifier::ServerCertificateVerifier>,
    client_verifier: Arc<verifier::ClientCertificateVerifier>,
}

impl TlsConfig {
    pub(crate) fn new(secret_key: SecretKey) -> Self {
        let cert_resolver = Arc::new(
            AlwaysResolvesCert::new(&secret_key).expect("Client cert key DER is valid; qed"),
        );
        Self {
            secret_key,
            cert_resolver,
            server_verifier: Arc::new(verifier::ServerCertificateVerifier),
            client_verifier: Arc::new(verifier::ClientCertificateVerifier),
        }
    }

    pub(crate) fn make_client_config(
        &self,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<ClientConfig, Report> {
        // 配置客户端
        let mut client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(self.server_verifier.clone())
            .with_client_cert_resolver(self.cert_resolver.clone());
        client_crypto.alpn_protocols = alpn_protocols;
        let client_config =
            quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(client_crypto)?));
        Ok(client_config)
    }

    pub(crate) fn make_server_config(
        &self,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Result<ServerConfig, Report> {
        // 配置服务器TLS
        let mut server_crypto = rustls::ServerConfig::builder()
            .with_client_cert_verifier(self.client_verifier.clone())
            .with_cert_resolver(self.cert_resolver.clone());
        // 设置 ALPN 协议标识，用于标识我们的P2P应用
        server_crypto.alpn_protocols = alpn_protocols;
        // 创建QUIC服务器配置
        let mut server_config =
            ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_crypto)?));
        // 启用QUIC keep-alive
        if let Some(transport_config) = Arc::get_mut(&mut server_config.transport) {
            transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(2)));
        }
        Ok(server_config)
    }
}
