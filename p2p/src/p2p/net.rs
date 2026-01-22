use bytecodec::{DecodeExt, EncodeExt};
use rootcause::{IntoReport, Report};
use std::net::ToSocketAddrs;
use std::time::Duration;
use stun_codec::rfc5389::Attribute;
use stun_codec::{Message, MessageClass, MessageEncoder, rfc5389};
use stun_codec::{TransactionId, rfc5389::methods};
use tokio::net::UdpSocket;

pub const PUB_STUN: [&'static str; 4] = [
    "stun.chat.bilibili.com:3478",
    "stun.miwifi.com:3478",
    "stun.hitv.com:3478",
    "stun.cdnbye.com:3478",
];

// 使用STUN协议获取公网IP地址
pub async fn stun_ip() -> Result<std::net::IpAddr, Report> {
    // 创建UDP套接字
    let socket = UdpSocket::bind("0.0.0.0:15004").await?;
    // 尝试连接到可用的STUN服务器
    let mut last_err = None;

    for &server_addr in PUB_STUN.iter() {
        match get_external_ip_from_stun(&socket, server_addr).await {
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
        None => Err(Report::from(std::io::Error::new(
            std::io::ErrorKind::Other,
            "No STUN servers available",
        ))),
    }
}

async fn get_external_ip_from_stun(
    socket: &UdpSocket,
    server_addr: &str,
) -> Result<std::net::IpAddr, Report> {
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
    socket
        .send_to(&buffer, addr)
        .await
        .map_err(|e| e.into_report())?;

    // 接收STUN响应
    let mut response_buffer = vec![0; 1024];
    let (len, _) = tokio::time::timeout(
        Duration::from_secs(5),
        socket.recv_from(&mut response_buffer),
    )
    .await
    .map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::TimedOut, "STUN request timed out").into_report()
    })??;

    // 解码响应
    let mut decoder = stun_codec::MessageDecoder::<Attribute>::new();
    let response = decoder.decode_from_bytes(&response_buffer[..len])?;
    match response {
        Ok(response) => {
            // 检查是否是成功响应
            if response.class() != MessageClass::SuccessResponse {
                return Err(Report::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "STUN server returned error",
                )));
            }

            // 查找 XOR_MAPPED_ADDRESS 属性
            for attr in response.attributes() {
                if let Attribute::XorMappedAddress(address) = attr {
                    dbg!(address.address());
                    return Ok(address.address().ip());
                }
            }

            // 如果没有找到 XOR_MAPPED_ADDRESS，尝试 MAPPED_ADDRESS
            for attr in response.attributes() {
                if let Attribute::MappedAddress(address) = attr {
                    return Ok(address.address().ip());
                }
            }
        }
        Err(_) => {}
    }

    Err(Report::from(std::io::Error::new(
        std::io::ErrorKind::Other,
        "No mapped address found in STUN response",
    )))
}

#[tokio::test]
async fn test_get_local_ip() {
    let result = stun_ip().await;
    dbg!(result);
}
