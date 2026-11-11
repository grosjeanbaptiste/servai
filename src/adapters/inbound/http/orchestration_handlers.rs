use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapters::inbound::http::{error_response::ApiError, server_handlers::AppState};
use crate::domain::{
    orchestration::{ExecutionStatus, JobId, OrchestrationJob},
    repository::{DriftReport, RepoId},
    server::ServerId,
    workflow::{
        DesiredServerState, FleetJobResult, FleetRolloutCommand, JobExecutionLogs,
        ReconciliationResult, ScheduledJob, ScheduledJobCommand, ScheduleId,
        WorkflowDefinition, WorkflowId, WorkflowStep,
    },
};

#[derive(Debug, Deserialize)]
pub struct ListJobsQuery {
    pub server_id: Option<Uuid>,
    pub status: Option<ExecutionStatus>,
}

#[derive(Debug, Deserialize)]
pub struct TriggerOrchestrationRequest {
    pub server_id: Uuid,
    pub repo_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteWorkflowRequest {
    pub server_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct DriftDetectionRequest {
    pub server_id: Uuid,
    pub repo_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct DriftResponse {
    pub has_drift: bool,
    pub report: DriftReport,
}

#[derive(Debug, Deserialize)]
pub struct CancelJobRequest {
    pub reason: Option<String>,
}

pub async fn list_orchestrations(
    State(state): State<AppState>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Json<Vec<OrchestrationJob>>, ApiError> {
    let server_id = query.server_id.map(ServerId::from_uuid);
    let jobs = state
        .orchestrator_service
        .list_jobs(server_id.as_ref(), query.status)
        .await?;
    Ok(Json(jobs))
}

pub async fn get_orchestration_logs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<JobExecutionLogs>, ApiError> {
    let logs = state
        .orchestrator_service
        .get_job_logs(&JobId::from_uuid(id))
        .await?;
    Ok(Json(logs))
}

pub async fn create_workflow_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowDefinition>), ApiError> {
    let wf = state
        .orchestrator_service
        .create_workflow(&payload.name, &payload.description, payload.steps)
        .await?;
    Ok((StatusCode::CREATED, Json(wf)))
}

pub async fn list_workflows_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<WorkflowDefinition>>, ApiError> {
    let list = state.orchestrator_service.list_workflows().await?;
    Ok(Json(list))
}

pub async fn get_workflow_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkflowDefinition>, ApiError> {
    let wf = state
        .orchestrator_service
        .get_workflow(&WorkflowId::from_uuid(id))
        .await?;
    Ok(Json(wf))
}

pub async fn execute_workflow_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExecuteWorkflowRequest>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let server_id = ServerId::from_uuid(payload.server_id);
    let job = state
        .orchestrator_service
        .execute_workflow(&server_id, &WorkflowId::from_uuid(id))
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}

pub async fn execute_fleet_rollout_handler(
    State(state): State<AppState>,
    Json(cmd): Json<FleetRolloutCommand>,
) -> Result<Json<FleetJobResult>, ApiError> {
    let res = state.orchestrator_service.execute_fleet_rollout(cmd).await?;
    Ok(Json(res))
}

pub async fn create_schedule_handler(
    State(state): State<AppState>,
    Json(cmd): Json<ScheduledJobCommand>,
) -> Result<(StatusCode, Json<ScheduledJob>), ApiError> {
    let sched = state.orchestrator_service.create_schedule(cmd).await?;
    Ok((StatusCode::CREATED, Json(sched)))
}

pub async fn list_schedules_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<ScheduledJob>>, ApiError> {
    let list = state.orchestrator_service.list_schedules().await?;
    Ok(Json(list))
}

pub async fn get_schedule_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ScheduledJob>, ApiError> {
    let sched = state
        .orchestrator_service
        .get_schedule(&ScheduleId::from_uuid(id))
        .await?;
    Ok(Json(sched))
}

pub async fn delete_schedule_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .orchestrator_service
        .delete_schedule(&ScheduleId::from_uuid(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_schedule_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let job = state
        .orchestrator_service
        .run_schedule_now(&ScheduleId::from_uuid(id))
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}

pub async fn reconcile_server_state_handler(
    State(state): State<AppState>,
    Json(desired): Json<DesiredServerState>,
) -> Result<Json<ReconciliationResult>, ApiError> {
    let res = state.orchestrator_service.reconcile_server_state(desired).await?;
    Ok(Json(res))
}

pub async fn trigger_orchestration(
    State(state): State<AppState>,
    Json(payload): Json<TriggerOrchestrationRequest>,
) -> Result<(StatusCode, Json<OrchestrationJob>), ApiError> {
    let server_id = ServerId::from_uuid(payload.server_id);
    let repo_id = RepoId::from_uuid(payload.repo_id);

    let job = state
        .orchestrator_service
        .trigger_orchestration(&server_id, &repo_id)
        .await?;
    Ok((StatusCode::CREATED, Json(job)))
}

pub async fn get_orchestration(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<OrchestrationJob>, ApiError> {
    let job = state
        .orchestrator_service
        .get_job(&JobId::from_uuid(id))
        .await?;
    Ok(Json(job))
}

pub async fn detect_drift_handler(
    State(state): State<AppState>,
    Json(payload): Json<DriftDetectionRequest>,
) -> Result<Json<DriftResponse>, ApiError> {
    let server_id = ServerId::from_uuid(payload.server_id);
    let repo_id = RepoId::from_uuid(payload.repo_id);

    let report = state
        .orchestrator_service
        .detect_drift(&server_id, &repo_id)
        .await?;

    let has_drift = report.has_drift();
    Ok(Json(DriftResponse { has_drift, report }))
}

pub async fn cancel_orchestration(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CancelJobRequest>,
) -> Result<Json<OrchestrationJob>, ApiError> {
    let reason = payload.reason.unwrap_or_else(|| "User cancelled".to_string());
    let job = state
        .orchestrator_service
        .cancel_job(&JobId::from_uuid(id), &reason)
        .await?;
    Ok(Json(job))
}
