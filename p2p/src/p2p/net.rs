use bytecodec::{DecodeExt, EncodeExt};
use quinn::EndpointConfig;
use rootcause::{IntoReport, Report};
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV4;
use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::sync::Arc;
use stun_codec::rfc5389::Attribute;
use stun_codec::{Message, MessageClass, MessageEncoder, rfc5389};
use stun_codec::{TransactionId, rfc5389::methods};
use tracing::debug;

pub struct Endpoint {}

pub const PUB_STUN: [&str; 4] = [
    "stun.chat.bilibili.com:3478",
    "stun.miwifi.com:3478",
    "stun.hitv.com:3478",
    "stun.cdnbye.com:3478",
];
pub async fn create_endpoint(
    addr: Option<SocketAddr>,
) -> Result<(quinn::Endpoint, SocketAddr), Report> {
    let bind_addr = addr.unwrap_or(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into());
    let socket = UdpSocket::bind(bind_addr)?;
    let socket_cloned = socket.try_clone()?;
    let ip = stun_addr(&socket_cloned).await;

    let mut endpoint_config = EndpointConfig::default();
    // 不接收0开头的数据包
    endpoint_config.grease_quic_bit(false);

    let endpoint =
        quinn::Endpoint::new(endpoint_config, None, socket, Arc::new(quinn::TokioRuntime))?;
    debug!("endpoint addr: {:?}", endpoint.local_addr());
    Ok((endpoint, ip?))
}

// 使用STUN协议获取公网IP地址
pub async fn stun_addr(socket: &UdpSocket) -> Result<std::net::SocketAddr, Report> {
    // 尝试连接到可用的STUN服务器
    let mut last_err = None;

    for &server_addr in PUB_STUN.iter() {
        match get_external_ip_from_stun(socket, server_addr).await {
            Ok(ip) => return Ok(ip),
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        }
    }

    // 如果所有服务器都失败，则返回最后一个错误
    match last_err {
        Some(err) => Err(err),
        None => Err(Report::from(std::io::Error::other(
            "No STUN servers available",
        ))),
    }
}

async fn get_external_ip_from_stun(
    socket: &UdpSocket,
    server_addr: &str,
) -> Result<std::net::SocketAddr, Report> {
    // 解析STUN服务器地址
    let addr = server_addr
        .to_socket_addrs()
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Failed to parse address {}: {}", server_addr, e),
            )
        })?
        .next()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("No valid addresses for {}", server_addr),
            )
        })?;

    // 构建STUN请求
    let mut request = Message::<Attribute>::new(
        MessageClass::Request,
        methods::BINDING,
        TransactionId::new([3; 12]),
    );
    request.add_attribute(Attribute::Software(
        rfc5389::attributes::Software::new("rust-stun-client".to_string()).unwrap(),
    ));

    // 编码消息
    let mut encoder = MessageEncoder::new();
    let buffer = encoder.encode_into_bytes(request)?;

    // 发送STUN请求
    socket.send_to(&buffer, addr).map_err(|e| e.into_report())?;

    // 接收STUN响应
    let mut response_buffer = vec![0; 1024];
    let (len, _) = socket.recv_from(&mut response_buffer)?;

    // 解码响应
    let mut decoder = stun_codec::MessageDecoder::<Attribute>::new();
    let response = decoder.decode_from_bytes(&response_buffer[..len])?;
    if let Ok(response) = response {
        // 检查是否是成功响应
        if response.class() != MessageClass::SuccessResponse {
            return Err(Report::from(std::io::Error::other(
                "STUN server returned error",
            )));
        }

        // 查找 XOR_MAPPED_ADDRESS 属性
        for attr in response.attributes() {
            if let Attribute::XorMappedAddress(address) = attr {
                return Ok(address.address());
            }
        }

        // 如果没有找到 XOR_MAPPED_ADDRESS，尝试 MAPPED_ADDRESS
        for attr in response.attributes() {
            if let Attribute::MappedAddress(address) = attr {
                return Ok(address.address());
            }
        }
    }

    Err(Report::from(std::io::Error::other(
        "No mapped address found in STUN response",
    )))
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

    use tracing::info;
    use tracing_subscriber::{
        fmt::{time, writer::MakeWriterExt},
        layer::SubscriberExt,
        util::SubscriberInitExt,
    };

    use crate::p2p::net::{create_endpoint, stun_addr};

    #[tokio::test]
    async fn test_get_local_ip() {
        let stderr_layer = tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_timer(time::LocalTime::rfc_3339())
            .with_ansi(true)
            .with_writer(std::io::stderr.with_max_level(tracing::Level::DEBUG));

        tracing_subscriber::registry().with(stderr_layer).init();
        let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        let socket = UdpSocket::bind(bind_addr).unwrap();
        let ip = stun_addr(&socket).await;
        info!(ip=?ip);
    }
}
