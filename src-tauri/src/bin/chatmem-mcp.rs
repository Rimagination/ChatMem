use chatmem::chatmem_memory::{mcp::ChatMemMcpService, store::MemoryStore};
use rmcp::{transport::stdio, ServiceExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = MemoryStore::open_app()?;
    let service = ChatMemMcpService::new(store);
    let server = service.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
