pub mod scanner;
pub mod mcp;
pub mod output;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    mcp::server::run_server().await
}
