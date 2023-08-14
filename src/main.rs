mod config;
mod host;
mod window;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Some(args) = std::env::args().nth(1) {
        window::main(&args).await
    } else {
        host::main()
    }
}
