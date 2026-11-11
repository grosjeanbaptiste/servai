use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::{
    engines::{
        ansible::{AnsibleFacts, AnsibleModuleResult, AnsibleRunResult},
        crossplane::{
            CrossplaneClaim, CrossplaneProviderStatus, CrossplaneReconcileResult,
            CrossplaneResource,
        },
        opentofu::{TofuApplyResult, TofuPlanResult, TofuState},
        wireguard::{WireGuardDeviceStatus, WireGuardInterface, WireGuardPeer},
    },
    repository::RepoId,
    server::ServerId,
};

#[derive(Debug, Deserialize)]
pub struct TofuPlanRequest {
    pub repo_id: Uuid,
    pub vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct TofuApplyRequest {
    pub repo_id: Uuid,
    pub vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct AnsiblePlaybookRequest {
    pub playbook: String,
    pub vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct AnsibleModuleRequest {
    pub module: String,
    pub args: String,
}

pub async fn plan_tofu_handler(
    State(state): State<AppState>,
    Json(payload): Json<TofuPlanRequest>,
) -> Result<Json<TofuPlanResult>, ApiError> {
    let repo_id = RepoId::from_uuid(payload.repo_id);
    let result = state
        .orchestrator_service
        .plan_tofu(&repo_id, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn apply_tofu_handler(
    State(state): State<AppState>,
    Json(payload): Json<TofuApplyRequest>,
) -> Result<Json<TofuApplyResult>, ApiError> {
    let repo_id = RepoId::from_uuid(payload.repo_id);
    let result = state
        .orchestrator_service
        .apply_tofu(&repo_id, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn destroy_tofu_handler(
    State(state): State<AppState>,
    Json(payload): Json<TofuApplyRequest>,
) -> Result<Json<TofuApplyResult>, ApiError> {
    let repo_id = RepoId::from_uuid(payload.repo_id);
    let result = state
        .orchestrator_service
        .destroy_tofu(&repo_id, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn get_tofu_state_handler(
    State(state): State<AppState>,
    Path(repo_id): Path<Uuid>,
) -> Result<Json<TofuState>, ApiError> {
    let id = RepoId::from_uuid(repo_id);
    let state_res = state.orchestrator_service.get_tofu_state(&id).await?;
    Ok(Json(state_res))
}

pub async fn run_ansible_playbook_handler(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    Json(payload): Json<AnsiblePlaybookRequest>,
) -> Result<Json<AnsibleRunResult>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let result = state
        .orchestrator_service
        .run_ansible_playbook(&id, &payload.playbook, payload.vars.unwrap_or_default())
        .await?;
    Ok(Json(result))
}

pub async fn run_ansible_module_handler(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    Json(payload): Json<AnsibleModuleRequest>,
) -> Result<Json<AnsibleModuleResult>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let result = state
        .server_service
        .run_ansible_module(&id, &payload.module, &payload.args)
        .await?;
    Ok(Json(result))
}

pub async fn gather_ansible_facts_handler(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
) -> Result<Json<AnsibleFacts>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let facts = state.server_service.gather_ansible_facts(&id).await?;
    Ok(Json(facts))
}

#[derive(Debug, Deserialize)]
pub struct CrossplaneClaimRequest {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub composition_ref: Option<String>,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct NamespaceQuery {
    pub namespace: Option<String>,
}

pub async fn apply_crossplane_claim_handler(
    State(state): State<AppState>,
    Json(payload): Json<CrossplaneClaimRequest>,
) -> Result<(StatusCode, Json<CrossplaneReconcileResult>), ApiError> {
    let claim = CrossplaneClaim::new(
        payload.api_version,
        payload.kind,
        payload.name,
        payload.namespace.unwrap_or_else(|| "default".to_string()),
        payload.composition_ref,
        payload.parameters.unwrap_or_default(),
    )?;
    let result = state.orchestrator_service.apply_crossplane_claim(claim).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

pub async fn get_crossplane_claim_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Json<CrossplaneReconcileResult>, ApiError> {
    let result = state
        .orchestrator_service
        .get_crossplane_claim_status(&namespace, &name)
        .await?;
    Ok(Json(result))
}

pub async fn delete_crossplane_claim_handler(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state
        .orchestrator_service
        .delete_crossplane_claim(&namespace, &name)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_crossplane_resources_handler(
    State(state): State<AppState>,
    Query(query): Query<NamespaceQuery>,
) -> Result<Json<Vec<CrossplaneResource>>, ApiError> {
    let resources = state
        .orchestrator_service
        .list_crossplane_resources(query.namespace)
        .await?;
    Ok(Json(resources))
}

pub async fn list_crossplane_providers_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<CrossplaneProviderStatus>>, ApiError> {
    let providers = state
        .orchestrator_service
        .list_crossplane_providers()
        .await?;
    Ok(Json(providers))
}

#[derive(Debug, Deserialize)]
pub struct WireGuardPeerRequest {
    pub public_key: String,
    pub preshared_key: Option<String>,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigureWireGuardRequest {
    pub name: String,
    pub private_key: String,
    pub public_key: String,
    pub listen_port: u16,
    pub address: String,
    pub peers: Option<Vec<WireGuardPeerRequest>>,
}

#[derive(Debug, Serialize)]
pub struct WireGuardKeypairResponse {
    pub private_key: String,
    pub public_key: String,
}

pub async fn configure_wireguard_handler(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
    Json(payload): Json<ConfigureWireGuardRequest>,
) -> Result<StatusCode, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let mut peers = Vec::new();
    if let Some(peer_reqs) = payload.peers {
        for p in peer_reqs {
            let peer = WireGuardPeer::new(
                p.public_key,
                p.preshared_key,
                p.endpoint,
                p.allowed_ips,
                p.persistent_keepalive,
            )?;
            peers.push(peer);
        }
    }

    let iface = WireGuardInterface::new(
        payload.name,
        payload.private_key,
        payload.public_key,
        payload.listen_port,
        payload.address,
        peers,
    )?;

    state.server_service.configure_wireguard(&id, iface).await?;
    Ok(StatusCode::CREATED)
}

pub async fn get_wireguard_status_handler(
    State(state): State<AppState>,
    Path((server_id, iface)): Path<(Uuid, String)>,
) -> Result<Json<WireGuardDeviceStatus>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let status = state.server_service.get_wireguard_status(&id, &iface).await?;
    Ok(Json(status))
}

pub async fn list_wireguard_peers_handler(
    State(state): State<AppState>,
    Path((server_id, iface)): Path<(Uuid, String)>,
) -> Result<Json<Vec<WireGuardPeer>>, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let peers = state.server_service.list_wireguard_peers(&id, &iface).await?;
    Ok(Json(peers))
}

pub async fn add_wireguard_peer_handler(
    State(state): State<AppState>,
    Path((server_id, iface)): Path<(Uuid, String)>,
    Json(payload): Json<WireGuardPeerRequest>,
) -> Result<StatusCode, ApiError> {
    let id = ServerId::from_uuid(server_id);
    let peer = WireGuardPeer::new(
        payload.public_key,
        payload.preshared_key,
        payload.endpoint,
        payload.allowed_ips,
        payload.persistent_keepalive,
    )?;
    state.server_service.add_wireguard_peer(&id, &iface, peer).await?;
    Ok(StatusCode::CREATED)
}

pub async fn remove_wireguard_peer_handler(
    State(state): State<AppState>,
    Path((server_id, iface, pubkey)): Path<(Uuid, String, String)>,
) -> Result<StatusCode, ApiError> {
    let id = ServerId::from_uuid(server_id);
    state.server_service.remove_wireguard_peer(&id, &iface, &pubkey).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn generate_wireguard_keypair_handler(
    State(state): State<AppState>,
) -> Result<Json<WireGuardKeypairResponse>, ApiError> {
    let (private_key, public_key) = state.server_service.generate_wireguard_keypair().await?;
    Ok(Json(WireGuardKeypairResponse {
        private_key,
        public_key,
    }))
}
