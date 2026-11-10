use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::domain::{
    errors::DomainError,
    stack::{MultiRepoStack, StackId},
};
use crate::ports::outbound::stack_store::StackStorePort;

#[derive(Default, Clone)]
pub struct InMemoryStackStore {
    stacks: Arc<Mutex<Vec<MultiRepoStack>>>,
}

impl InMemoryStackStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl StackStorePort for InMemoryStackStore {
    async fn save(&self, stack: MultiRepoStack) -> Result<(), DomainError> {
        let mut list = self.stacks.lock().unwrap();
        if let Some(pos) = list.iter().position(|s| s.id() == stack.id()) {
            list[pos] = stack;
        } else {
            list.push(stack);
        }
        Ok(())
    }

    async fn find_by_id(&self, id: &StackId) -> Result<Option<MultiRepoStack>, DomainError> {
        let list = self.stacks.lock().unwrap();
        Ok(list.iter().find(|s| s.id() == *id).cloned())
    }

    async fn list(&self) -> Result<Vec<MultiRepoStack>, DomainError> {
        let list = self.stacks.lock().unwrap();
        Ok(list.clone())
    }
}
