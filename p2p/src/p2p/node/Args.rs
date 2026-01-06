use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// 证书地址
    #[arg(short, long)]
    pub cert_addr: PathBuf,
    /// 服务名
    #[arg(short='s', long)]
    pub server_name: String,
    /// 启动地址
    #[arg(short, long)]
    pub local_addr: SocketAddr,
    /// 连接到的远程地址
    #[arg(long, default_value = None)]
    pub remote_addr: Option<SocketAddr>,
    /// 是否是ai节点
    #[arg(long, default_value = "false")]
    pub ai_agent: bool,
    /// 是否启动ui
    #[arg(long, default_value = "false")]
    pub without_ui: bool,
}
