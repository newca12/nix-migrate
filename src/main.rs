pub mod mcp;
pub mod output;
pub mod scanner;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    mcp::server::run_server().await
}
