use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use vectordb_core::VectorDb;
use vectordb_server::app;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing Production Vector Database Server...");

    let db = Arc::new(VectorDb::new());

    // Spawn background snapshotting timer (every 5 minutes)
    let db_cron = Arc::clone(&db);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = db_cron.save_snapshot() {
                // Ignore "already in progress" error, print others
                if !e.to_string().contains("already in progress") {
                    eprintln!("Background snapshot notice: {}", e);
                }
            }
        }
    });

    let router = app(db);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Vector DB Server listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
