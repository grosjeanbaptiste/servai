use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::domain::{
    errors::DomainError,
    orchestration::{JobId, OrchestrationJob},
    server::ServerId,
};
use crate::ports::outbound::job_store::JobStorePort;

#[derive(Default, Clone)]
pub struct InMemoryJobStore {
    jobs: Arc<Mutex<Vec<OrchestrationJob>>>,
}

impl InMemoryJobStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl JobStorePort for InMemoryJobStore {
    async fn save(&self, job: OrchestrationJob) -> Result<(), DomainError> {
        let mut list = self.jobs.lock().unwrap();
        if let Some(pos) = list.iter().position(|j| j.id() == job.id()) {
            list[pos] = job;
        } else {
            list.push(job);
        }
        Ok(())
    }

    async fn find_by_id(&self, id: &JobId) -> Result<Option<OrchestrationJob>, DomainError> {
        let list = self.jobs.lock().unwrap();
        Ok(list.iter().find(|j| j.id() == *id).cloned())
    }

    async fn list_by_server(&self, server_id: &ServerId) -> Result<Vec<OrchestrationJob>, DomainError> {
        let list = self.jobs.lock().unwrap();
        Ok(list
            .iter()
            .filter(|j| j.server_id() == *server_id)
            .cloned()
            .collect())
    }

    async fn list_all(&self) -> Result<Vec<OrchestrationJob>, DomainError> {
        let list = self.jobs.lock().unwrap();
        Ok(list.clone())
    }
}
