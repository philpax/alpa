mod command;
mod config;
mod host;
mod keycode;
mod window;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // This executable does double-duty as both the input window and the host.
    // I attempted to get this working with a single process, but it seems that
    // the input window will not focus if it's created by an already existing
    // process.
    //
    // This is a workaround that should always work by virtue of a new process
    // being spawned.
    if let Some(args) = std::env::args().nth(1) {
        window::main(&args).await
    } else {
        host::main()
    }
}
