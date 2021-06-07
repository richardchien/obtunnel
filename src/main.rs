extern crate obtunnel;

#[tokio::main]
async fn main() {
    obtunnel::main().await.unwrap();
}
