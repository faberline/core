use tower_lsp::{LspService, Server};

use super::argus_server::ArgusServer;

/// Run the LSP server on stdio
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(ArgusServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

/// Run the LSP server on TCP (for debugging)
pub async fn run_server_tcp(port: u16) -> std::io::Result<()> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!("Argus LSP server listening on port {}", port);

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::info!("Client connected from {}", addr);

        let (read, write) = tokio::io::split(stream);
        let (service, socket) = LspService::new(ArgusServer::new);

        tokio::spawn(async move {
            Server::new(read, write, socket).serve(service).await;
        });
    }
}
