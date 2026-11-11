use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::registrar::{
    ContactInfo, DomainRegistration, DomainStatus, NameserverConfig, RegistrarProvider,
};

#[derive(Debug, Deserialize)]
pub struct AvailabilityQuery {
    pub domain: String,
}

#[derive(Debug, Serialize)]
pub struct AvailabilityResponse {
    pub domain: String,
    pub available: bool,
}

#[derive(Debug, Deserialize)]
pub struct RegisterDomainRequest {
    pub domain: String,
    pub provider: RegistrarProvider,
    pub contact_name: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub contact_city: String,
    pub contact_country: String,
    pub nameservers: Vec<String>,
    pub auto_renew: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNameserversRequest {
    pub nameservers: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetAutoRenewRequest {
    pub enable: bool,
}

pub async fn check_availability_handler(
    State(state): State<AppState>,
    Query(query): Query<AvailabilityQuery>,
) -> Result<Json<AvailabilityResponse>, ApiError> {
    let available = state
        .registrar_service
        .check_domain_availability(&query.domain)
        .await?;
    Ok(Json(AvailabilityResponse {
        domain: query.domain,
        available,
    }))
}

pub async fn register_domain_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterDomainRequest>,
) -> Result<(StatusCode, Json<DomainRegistration>), ApiError> {
    let contact = ContactInfo::new(
        payload.contact_name,
        payload.contact_email,
        payload.contact_phone,
        payload.contact_city,
        payload.contact_country,
    )?;

    let ns = NameserverConfig::new(payload.nameservers)?;

    let registration = DomainRegistration::new(
        payload.domain,
        payload.provider,
        DomainStatus::Active,
        contact,
        ns,
        payload.auto_renew.unwrap_or(true),
    )?;

    let registered = state.registrar_service.register_domain(registration).await?;
    Ok((StatusCode::CREATED, Json(registered)))
}

pub async fn get_domain_handler(
    State(state): State<AppState>,
    Path(domain): Path<String>,
) -> Result<Json<DomainRegistration>, ApiError> {
    let reg = state.registrar_service.get_domain(&domain).await?;
    Ok(Json(reg))
}

pub async fn list_domains_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<DomainRegistration>>, ApiError> {
    let domains = state.registrar_service.list_domains().await?;
    Ok(Json(domains))
}

pub async fn update_nameservers_handler(
    State(state): State<AppState>,
    Path(domain): Path<String>,
    Json(payload): Json<UpdateNameserversRequest>,
) -> Result<StatusCode, ApiError> {
    let ns = NameserverConfig::new(payload.nameservers)?;
    state.registrar_service.update_domain_nameservers(&domain, ns).await?;
    Ok(StatusCode::OK)
}

pub async fn set_auto_renew_handler(
    State(state): State<AppState>,
    Path(domain): Path<String>,
    Json(payload): Json<SetAutoRenewRequest>,
) -> Result<StatusCode, ApiError> {
    state.registrar_service.set_domain_auto_renew(&domain, payload.enable).await?;
    Ok(StatusCode::OK)
}
