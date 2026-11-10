use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use servai::{
    adapters::{
        inbound::http::create_router,
        outbound::{
            git::git_cli::GitCliProvider,
            memory::{
                job_store::InMemoryJobStore, repo_store::InMemoryRepoStore,
                server_store::InMemoryServerStore,
            },
            system::ssh_executor::SshServerExecutor,
        },
    },
    application::{
        orchestration_service::OrchestrationService,
        server_service::ServerService,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Servai Orchestrator v{}", env!("CARGO_PKG_VERSION"));

    let server_store = Arc::new(InMemoryServerStore::new());
    let repo_store = Arc::new(InMemoryRepoStore::new());
    let job_store = Arc::new(InMemoryJobStore::new());
    let executor = Arc::new(SshServerExecutor::new());
    let git_provider = Arc::new(GitCliProvider::default());

    let server_service = Arc::new(ServerService::new(server_store.clone(), executor.clone()));
    let orchestrator_service = Arc::new(OrchestrationService::new(
        server_store,
        repo_store,
        job_store,
        executor,
        git_provider,
    ));

    let router = create_router(server_service, orchestrator_service)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Servai API for Bioxus UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
