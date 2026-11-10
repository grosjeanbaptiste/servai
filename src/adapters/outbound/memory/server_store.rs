use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::domain::{
    errors::DomainError,
    server::{Server, ServerId},
};
use crate::ports::outbound::server_store::ServerStorePort;

#[derive(Default, Clone)]
pub struct InMemoryServerStore {
    servers: Arc<Mutex<Vec<Server>>>,
}

impl InMemoryServerStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ServerStorePort for InMemoryServerStore {
    async fn save(&self, server: Server) -> Result<(), DomainError> {
        let mut list = self.servers.lock().unwrap();
        if let Some(pos) = list.iter().position(|s| s.id() == server.id()) {
            list[pos] = server;
        } else {
            list.push(server);
        }
        Ok(())
    }

    async fn find_by_id(&self, id: &ServerId) -> Result<Option<Server>, DomainError> {
        let list = self.servers.lock().unwrap();
        Ok(list.iter().find(|s| s.id() == *id).cloned())
    }

    async fn list(&self) -> Result<Vec<Server>, DomainError> {
        let list = self.servers.lock().unwrap();
        Ok(list.clone())
    }

    async fn delete(&self, id: &ServerId) -> Result<(), DomainError> {
        let mut list = self.servers.lock().unwrap();
        list.retain(|s| s.id() != *id);
        Ok(())
    }
}
