pub mod mcp;
pub mod output;
pub mod scanner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    mcp::server::run_server().await
}
