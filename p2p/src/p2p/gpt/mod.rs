use rootcause::Report;

pub mod common;
pub mod deepseek_v3;
pub mod render;

pub trait Chat {
    async fn chat(&self, input: String, user: String) -> Result<String, Report>;
}
