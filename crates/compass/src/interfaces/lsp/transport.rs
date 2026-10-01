use tower_lsp::{Client, LspService, Server};

use super::argus_server::ArgusServer;

/// Serve the LSP on stdio, building the server with `make_server`
pub(crate) async fn serve_stdio(make_server: fn(Client) -> ArgusServer) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(make_server);
    Server::new(stdin, stdout, socket).serve(service).await;
}

/// Serve the LSP on TCP (for debugging), building a server per connection
/// with `make_server`
pub(crate) async fn serve_tcp(
    port: u16,
    make_server: fn(Client) -> ArgusServer,
) -> std::io::Result<()> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!("Argus LSP server listening on port {}", port);

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::info!("Client connected from {}", addr);

        let (read, write) = tokio::io::split(stream);
        let (service, socket) = LspService::new(make_server);

        tokio::spawn(async move {
            Server::new(read, write, socket).serve(service).await;
        });
    }
}
