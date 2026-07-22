use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use vectordb_core::VectorDb;
use vectordb_server::app;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing Production Vector Database Server...");

    let db = Arc::new(VectorDb::new());
    let router = app(db);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Vector DB Server listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
