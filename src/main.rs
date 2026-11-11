use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use servai::{
    adapters::{
        inbound::http::create_router_full,
        outbound::{
            dns::MockDnsProvider,
            git::git_cli::GitCliProvider,
            memory::{
                job_store::InMemoryJobStore, repo_store::InMemoryRepoStore,
                server_store::InMemoryServerStore,
            },
            registrar::MockRegistrarProvider,
            system::ssh_executor::SshServerExecutor,
            tls::MockTlsManager,
        },
    },
    application::{
        dns_service::DnsService,
        orchestration_service::OrchestrationService,
        registrar_service::RegistrarService,
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
    let tofu_engine = Arc::new(servai::adapters::outbound::opentofu::CliOpenTofuAdapter::default());
    let ansible_engine = Arc::new(servai::adapters::outbound::ansible::CliAnsibleAdapter::default());
    let crossplane_engine = Arc::new(servai::adapters::outbound::crossplane::CliCrossplaneAdapter::default());
    let wireguard_engine = Arc::new(servai::adapters::outbound::wireguard::CliWireGuardAdapter::default());
    let dns_provider = Arc::new(MockDnsProvider::default());
    let registrar_provider = Arc::new(MockRegistrarProvider::default());
    let tls_manager = Arc::new(MockTlsManager::default());

    let dns_service = Arc::new(DnsService::new(dns_provider.clone()));
    let registrar_service = Arc::new(RegistrarService::new(registrar_provider));

    let server_service = Arc::new(
        ServerService::new_with_engines(
            server_store.clone(),
            executor.clone(),
            ansible_engine.clone(),
            wireguard_engine,
        )
        .with_dns_provider(dns_provider)
        .with_tls_manager(tls_manager),
    );
    let orchestrator_service = Arc::new(
        OrchestrationService::new(
            server_store,
            repo_store,
            job_store,
            executor,
            git_provider,
        )
        .with_tofu_engine(tofu_engine)
        .with_ansible_engine(ansible_engine)
        .with_crossplane_engine(crossplane_engine),
    );

    let router = create_router_full(
        server_service,
        orchestrator_service,
        dns_service,
        registrar_service,
    )
    .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Servai API for Bioxus UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
