use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::domain::{
    errors::DomainError,
    repository::{ManagedRepository, RepoId},
};
use crate::ports::outbound::repo_store::RepoStorePort;

#[derive(Default, Clone)]
pub struct InMemoryRepoStore {
    repos: Arc<Mutex<Vec<ManagedRepository>>>,
}

impl InMemoryRepoStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RepoStorePort for InMemoryRepoStore {
    async fn save(&self, repo: ManagedRepository) -> Result<(), DomainError> {
        let mut list = self.repos.lock().unwrap();
        if let Some(pos) = list.iter().position(|r| r.id() == repo.id()) {
            list[pos] = repo;
        } else {
            list.push(repo);
        }
        Ok(())
    }

    async fn find_by_id(&self, id: &RepoId) -> Result<Option<ManagedRepository>, DomainError> {
        let list = self.repos.lock().unwrap();
        Ok(list.iter().find(|r| r.id() == *id).cloned())
    }

    async fn list(&self) -> Result<Vec<ManagedRepository>, DomainError> {
        let list = self.repos.lock().unwrap();
        Ok(list.clone())
    }
}
