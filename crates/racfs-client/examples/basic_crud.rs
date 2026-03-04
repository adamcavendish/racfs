//! Basic CRUD operations using the RACFS HTTP client.
//!
//! Run a server first: `cargo run -p racfs-server`
//! Then: `cargo run -p racfs-client --example basic_crud`

use racfs_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base =
        std::env::var("RACFS_SERVER").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let client = Client::new(&base);

    println!("Server: {}", base);
    let health = client.health().await?;
    println!("Health: {} {}", health.status, health.version);

    let dir = "/memfs/example";
    println!("\n1. Create directory {}", dir);
    client.mkdir(dir, Some(0o755)).await?;

    let path = format!("{}/hello.txt", dir);
    println!("2. Create and write file {}", path);
    client.create_file(&path).await?;
    client.write_file(&path, "Hello, RACFS!", None).await?;

    println!("3. Read file");
    let content = client.read_file(&path).await?;
    assert_eq!(content, "Hello, RACFS!");
    println!("   Content: {}", content);

    println!("4. List directory {}", dir);
    let list = client.read_dir(dir).await?;
    for e in &list.entries {
        println!("   {} {}", e.permissions, e.path);
    }

    println!("5. Stat {}", path);
    let meta = client.stat(&path).await?;
    println!("   type={} size={}", meta.file_type, meta.size);

    println!("6. Remove file and directory");
    client.remove(&path).await?;
    client.remove(dir).await?;

    println!("\nDone.");
    Ok(())
}
