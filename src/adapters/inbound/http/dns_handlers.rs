use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::dns::{DnsRecord, DnsRecordType, DnsZone, DnssecStatus, Ttl};

#[derive(Debug, Deserialize)]
pub struct CreateZoneRequest {
    pub domain: String,
    pub enable_dnssec: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertRecordRequest {
    pub id: Option<String>,
    pub name: String,
    pub record_type: DnsRecordType,
    pub value: String,
    pub ttl_seconds: Option<u32>,
    pub priority: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct PropagationQuery {
    pub record_type: DnsRecordType,
    pub expected_value: String,
}

#[derive(Debug, Serialize)]
pub struct PropagationResponse {
    pub domain: String,
    pub propagated: bool,
}

pub async fn create_dns_zone_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateZoneRequest>,
) -> Result<(StatusCode, Json<DnsZone>), ApiError> {
    let dnssec = if payload.enable_dnssec.unwrap_or(true) {
        DnssecStatus::Enabled
    } else {
        DnssecStatus::Disabled
    };
    let zone_id = format!("zone-{}", payload.domain.replace('.', "-"));
    let zone = DnsZone::new(zone_id, payload.domain, dnssec)?;
    let created = state.dns_service.create_dns_zone(zone).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn get_dns_zone_handler(
    State(state): State<AppState>,
    Path(domain): Path<String>,
) -> Result<Json<DnsZone>, ApiError> {
    let zone = state.dns_service.get_dns_zone(&domain).await?;
    Ok(Json(zone))
}

pub async fn list_dns_zones_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<DnsZone>>, ApiError> {
    let zones = state.dns_service.list_dns_zones().await?;
    Ok(Json(zones))
}

pub async fn upsert_dns_record_handler(
    State(state): State<AppState>,
    Path(domain): Path<String>,
    Json(payload): Json<UpsertRecordRequest>,
) -> Result<(StatusCode, Json<DnsRecord>), ApiError> {
    let ttl = if let Some(secs) = payload.ttl_seconds {
        Ttl::custom(secs)?
    } else {
        Ttl::standard()
    };

    let record_id = payload.id.unwrap_or_else(|| {
        format!("rec-{}-{}", payload.name.replace('.', "-"), payload.value.replace('.', "-"))
    });

    let record = DnsRecord::new(
        record_id,
        payload.name,
        payload.record_type,
        payload.value,
        ttl,
        payload.priority,
    )?;

    let upserted = state.dns_service.upsert_dns_record(&domain, record).await?;
    Ok((StatusCode::CREATED, Json(upserted)))
}

pub async fn delete_dns_record_handler(
    State(state): State<AppState>,
    Path((domain, record_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state.dns_service.delete_dns_record(&domain, &record_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn verify_dns_propagation_handler(
    State(state): State<AppState>,
    Path(domain): Path<String>,
    Query(query): Query<PropagationQuery>,
) -> Result<Json<PropagationResponse>, ApiError> {
    let propagated = state
        .dns_service
        .verify_dns_propagation(&domain, query.record_type, &query.expected_value)
        .await?;
    Ok(Json(PropagationResponse { domain, propagated }))
}
