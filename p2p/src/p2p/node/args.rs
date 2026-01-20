use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// 启动地址
    #[arg(short = 'l', long, default_value = None)]
    pub local_addr: Option<SocketAddr>,
    /// 公网地址
    #[arg(short = 'p', long, default_value = None)]
    pub public_addr: Option<SocketAddr>,
    /// 连接到的远程地址
    #[arg(long, default_value = None, requires = "server_name")]
    pub connect_to: Option<SocketAddr>,
    /// 服务名
    #[arg(short = 's', long, default_value = None)]
    pub server_name: Option<String>,
    /// 是否是ai节点
    #[arg(long, default_value = "false")]
    pub ai_agent: bool,
    /// 是否启动ui
    #[arg(long, default_value = "false")]
    pub without_ui: bool,
}
