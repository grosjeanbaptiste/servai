use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

use crate::domain::{
    engines::opentofu::{TofuApplyResult, TofuPlanResult, TofuState},
    errors::DomainError,
};

#[async_trait]
pub trait OpenTofuEnginePort: Send + Sync {
    async fn init(&self, repo_path: &Path) -> Result<(), DomainError>;
    async fn plan(&self, repo_path: &Path, vars: HashMap<String, String>) -> Result<TofuPlanResult, DomainError>;
    async fn apply(&self, repo_path: &Path, vars: HashMap<String, String>) -> Result<TofuApplyResult, DomainError>;
    async fn destroy(&self, repo_path: &Path, vars: HashMap<String, String>) -> Result<TofuApplyResult, DomainError>;
    async fn get_state(&self, repo_path: &Path) -> Result<TofuState, DomainError>;
}
