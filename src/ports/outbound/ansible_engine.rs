use async_trait::async_trait;
use std::collections::HashMap;

use crate::domain::{
    engines::ansible::{AnsibleFacts, AnsibleModuleResult, AnsibleRunResult},
    errors::DomainError,
    server::Server,
};

#[async_trait]
pub trait AnsibleEnginePort: Send + Sync {
    async fn run_playbook(
        &self,
        server: &Server,
        playbook_name: &str,
        extra_vars: HashMap<String, String>,
    ) -> Result<AnsibleRunResult, DomainError>;

    async fn run_module(
        &self,
        server: &Server,
        module: &str,
        args: &str,
    ) -> Result<AnsibleModuleResult, DomainError>;

    async fn gather_facts(&self, server: &Server) -> Result<AnsibleFacts, DomainError>;
}
