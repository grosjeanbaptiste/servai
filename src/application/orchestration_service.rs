use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::adapters::outbound::{
    ansible::MockAnsibleEngine,
    catalog::StandardServerAdminCatalog,
    crossplane::MockCrossplaneEngine,
    memory::stack_store::InMemoryStackStore,
    opentofu::MockOpenTofuEngine,
    registry::MultiSourceAdminRegistry,
};
use crate::domain::{
    catalog::{AdminCategory, CatalogItem},
    engines::{
        ansible::AnsibleRunResult,
        crossplane::{
            CrossplaneClaim, CrossplaneProviderStatus, CrossplaneReconcileResult,
            CrossplaneResource,
        },
        opentofu::{TofuApplyResult, TofuPlanResult, TofuState},
    },
    errors::DomainError,
    orchestration::{JobId, OrchestrationJob, RollbackPolicy},
    registry::{AdminPackage, RegistryQuery, ServerProfilePreset},
    repository::{DriftReport, GitRef, ManagedRepository, RepoId, RepoKind, SyncStatus},
    server::{ServerId, ServerStatus},
    stack::{MultiRepoStack, StackId, StackRepoEntry},
    workflow::{
        DesiredServerState, FleetJobResult, FleetRolloutCommand,
        JobExecutionLogs, ReconciliationResult, ScheduledJob, ScheduledJobCommand, ScheduleId,
        StepLogEntry, WorkflowAction, WorkflowDefinition, WorkflowId, WorkflowStep,
    },
};
use crate::ports::{
    inbound::repo_orchestrator::{RegisterRepoCommand, RepoOrchestratorPort},
    outbound::{
        admin_registry::AdminRegistryPort,
        ansible_engine::AnsibleEnginePort,
        crossplane_engine::CrossplaneEnginePort,
        git_provider::GitProviderPort,
        job_store::JobStorePort,
        opentofu_engine::OpenTofuEnginePort,
        repo_catalog::RepoCatalogPort,
        repo_store::RepoStorePort,
        server_executor::ServerExecutorPort,
        server_store::ServerStorePort,
        stack_store::StackStorePort,
    },
};

pub struct OrchestrationService {
    server_store: Arc<dyn ServerStorePort>,
    repo_store: Arc<dyn RepoStorePort>,
    job_store: Arc<dyn JobStorePort>,
    executor: Arc<dyn ServerExecutorPort>,
    git_provider: Arc<dyn GitProviderPort>,
    catalog: Arc<dyn RepoCatalogPort>,
    stack_store: Arc<dyn StackStorePort>,
    registry: Arc<dyn AdminRegistryPort>,
    tofu_engine: Arc<dyn OpenTofuEnginePort>,
    ansible_engine: Arc<dyn AnsibleEnginePort>,
    crossplane_engine: Arc<dyn CrossplaneEnginePort>,
    workflows: Arc<RwLock<HashMap<WorkflowId, WorkflowDefinition>>>,
    schedules: Arc<RwLock<HashMap<ScheduleId, ScheduledJob>>>,
}

impl OrchestrationService {
    pub fn new(
        server_store: Arc<dyn ServerStorePort>,
        repo_store: Arc<dyn RepoStorePort>,
        job_store: Arc<dyn JobStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
        git_provider: Arc<dyn GitProviderPort>,
    ) -> Self {
        Self::new_full(
            server_store,
            repo_store,
            job_store,
            executor,
            git_provider,
            Arc::new(StandardServerAdminCatalog::default()),
            Arc::new(InMemoryStackStore::new()),
            Arc::new(MultiSourceAdminRegistry::default()),
        )
    }

    pub fn new_with_catalog(
        server_store: Arc<dyn ServerStorePort>,
        repo_store: Arc<dyn RepoStorePort>,
        job_store: Arc<dyn JobStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
        git_provider: Arc<dyn GitProviderPort>,
        catalog: Arc<dyn RepoCatalogPort>,
    ) -> Self {
        Self::new_full(
            server_store,
            repo_store,
            job_store,
            executor,
            git_provider,
            catalog,
            Arc::new(InMemoryStackStore::new()),
            Arc::new(MultiSourceAdminRegistry::default()),
        )
    }

    pub fn new_full(
        server_store: Arc<dyn ServerStorePort>,
        repo_store: Arc<dyn RepoStorePort>,
        job_store: Arc<dyn JobStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
        git_provider: Arc<dyn GitProviderPort>,
        catalog: Arc<dyn RepoCatalogPort>,
        stack_store: Arc<dyn StackStorePort>,
        registry: Arc<dyn AdminRegistryPort>,
    ) -> Self {
        Self::new_with_engines(
            server_store,
            repo_store,
            job_store,
            executor,
            git_provider,
            catalog,
            stack_store,
            registry,
            Arc::new(MockOpenTofuEngine::default()),
            Arc::new(MockAnsibleEngine::default()),
            Arc::new(MockCrossplaneEngine::default()),
        )
    }

    pub fn new_with_engines(
        server_store: Arc<dyn ServerStorePort>,
        repo_store: Arc<dyn RepoStorePort>,
        job_store: Arc<dyn JobStorePort>,
        executor: Arc<dyn ServerExecutorPort>,
        git_provider: Arc<dyn GitProviderPort>,
        catalog: Arc<dyn RepoCatalogPort>,
        stack_store: Arc<dyn StackStorePort>,
        registry: Arc<dyn AdminRegistryPort>,
        tofu_engine: Arc<dyn OpenTofuEnginePort>,
        ansible_engine: Arc<dyn AnsibleEnginePort>,
        crossplane_engine: Arc<dyn CrossplaneEnginePort>,
    ) -> Self {
        Self {
            server_store,
            repo_store,
            job_store,
            executor,
            git_provider,
            catalog,
            stack_store,
            registry,
            tofu_engine,
            ansible_engine,
            crossplane_engine,
            workflows: Arc::new(RwLock::new(HashMap::new())),
            schedules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_tofu_engine(mut self, engine: Arc<dyn OpenTofuEnginePort>) -> Self {
        self.tofu_engine = engine;
        self
    }

    pub fn with_ansible_engine(mut self, engine: Arc<dyn AnsibleEnginePort>) -> Self {
        self.ansible_engine = engine;
        self
    }

    pub fn with_crossplane_engine(mut self, engine: Arc<dyn CrossplaneEnginePort>) -> Self {
        self.crossplane_engine = engine;
        self
    }
}

#[async_trait]
impl RepoOrchestratorPort for OrchestrationService {
    async fn register_repo(&self, cmd: RegisterRepoCommand) -> Result<ManagedRepository, DomainError> {
        let repo = ManagedRepository::new(
            RepoId::new(),
            cmd.name,
            cmd.url,
            cmd.target_ref,
            cmd.kind,
        )?;
        self.repo_store.save(repo.clone()).await?;
        Ok(repo)
    }

    async fn get_repo(&self, id: &RepoId) -> Result<ManagedRepository, DomainError> {
        self.repo_store
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "ManagedRepository",
                id: id.to_string(),
            })
    }

    async fn list_repos(&self) -> Result<Vec<ManagedRepository>, DomainError> {
        self.repo_store.list().await
    }

    async fn sync_repo(&self, id: &RepoId) -> Result<ManagedRepository, DomainError> {
        let mut repo = self.get_repo(id).await?;
        repo.set_sync_status(SyncStatus::Syncing);
        self.repo_store.save(repo.clone()).await?;

        match self.git_provider.sync_repository(&repo).await {
            Ok(commit_hash) => {
                repo.set_sync_status(SyncStatus::Synced);
                repo.set_last_commit_hash(commit_hash);
                self.repo_store.save(repo.clone()).await?;
                Ok(repo)
            }
            Err(e) => {
                repo.set_sync_status(SyncStatus::Failed);
                self.repo_store.save(repo).await?;
                Err(e)
            }
        }
    }

    async fn trigger_orchestration(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
    ) -> Result<OrchestrationJob, DomainError> {
        self.trigger_orchestration_with_options(server_id, repo_id, RollbackPolicy::Halt)
            .await
    }

    async fn trigger_orchestration_with_options(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
        rollback_policy: RollbackPolicy,
    ) -> Result<OrchestrationJob, DomainError> {
        let mut server = self
            .server_store
            .find_by_id(server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            })?;

        if server.status() != ServerStatus::Online {
            return Err(DomainError::Conflict(format!(
                "Cannot orchestrate on server '{}' with status {:?}",
                server.name().as_str(),
                server.status()
            )));
        }

        let repo = self.get_repo(repo_id).await?;
        let steps = self.git_provider.resolve_plan(&repo).await?;

        let mut job = OrchestrationJob::new_with_policy(
            JobId::new(),
            *server_id,
            *repo_id,
            steps,
            rollback_policy,
        )?;

        // Acquire server concurrency lock
        server.acquire_lock(job.id())?;
        self.server_store.save(server.clone()).await?;

        job.start()?;
        self.job_store.save(job.clone()).await?;

        // Execute steps sequentially
        let step_count = job.steps().len();
        let mut step_failed = false;

        for i in 0..step_count {
            let step_name = job.steps()[i].step.name.clone();
            let cmd_to_run = format!("servai-run-step:{}", step_name);

            match self.executor.execute_command(&server, &cmd_to_run).await {
                Ok(res) => {
                    if res.exit_code == 0 {
                        job.record_step_success(i, res.stdout)?;
                    } else {
                        job.record_step_failure(
                            i,
                            format!("Step failed with exit code {}", res.exit_code),
                            res.stderr,
                        )?;
                        step_failed = true;
                        break;
                    }
                }
                Err(err) => {
                    job.record_step_failure(i, "Step execution error", err.to_string())?;
                    step_failed = true;
                    break;
                }
            }
        }

        // Check if rollback needed
        if step_failed && rollback_policy == RollbackPolicy::RollbackOnFailure {
            let rollback_steps = job.compensating_rollback_steps();
            for rb_step in rollback_steps {
                if let Some(rb_cmd) = rb_step.rollback_command {
                    let _ = self.executor.execute_command(&server, &rb_cmd).await;
                }
            }
            job.record_rollback_completed("Compensating rollback finished")?;
        }

        // Release server lock
        if let Ok(Some(mut fresh_server)) = self.server_store.find_by_id(server_id).await {
            let _ = fresh_server.release_lock(job.id());
            let _ = self.server_store.save(fresh_server).await;
        }

        self.job_store.save(job.clone()).await?;
        Ok(job)
    }

    async fn get_job(&self, id: &JobId) -> Result<OrchestrationJob, DomainError> {
        self.job_store
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "OrchestrationJob",
                id: id.to_string(),
            })
    }

    async fn cancel_job(&self, id: &JobId, reason: &str) -> Result<OrchestrationJob, DomainError> {
        let mut job = self.get_job(id).await?;
        job.cancel(reason)?;

        // Ensure server lock is released on cancel
        if let Ok(Some(mut server)) = self.server_store.find_by_id(&job.server_id()).await {
            let _ = server.release_lock(job.id());
            let _ = self.server_store.save(server).await;
        }

        self.job_store.save(job.clone()).await?;
        Ok(job)
    }

    async fn detect_drift(
        &self,
        server_id: &ServerId,
        repo_id: &RepoId,
    ) -> Result<DriftReport, DomainError> {
        let _server = self.server_store.find_by_id(server_id).await?.ok_or_else(|| {
            DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            }
        })?;
        let repo = self.get_repo(repo_id).await?;

        self.git_provider.detect_drift(server_id, &repo).await
    }

    async fn list_catalog(
        &self,
        category: Option<AdminCategory>,
    ) -> Result<Vec<CatalogItem>, DomainError> {
        let items = self.catalog.list_catalog().await?;
        if let Some(cat) = category {
            Ok(items.into_iter().filter(|i| i.category() == cat).collect())
        } else {
            Ok(items)
        }
    }

    async fn import_from_catalog(
        &self,
        catalog_id: &str,
        custom_name: Option<String>,
    ) -> Result<ManagedRepository, DomainError> {
        let item = self
            .catalog
            .find_catalog_item(catalog_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "CatalogItem",
                id: catalog_id.to_string(),
            })?;

        let name = custom_name.unwrap_or_else(|| item.name().to_string());
        let repo = ManagedRepository::new(
            RepoId::new(),
            name,
            item.upstream_url().to_string(),
            item.target_ref().clone(),
            item.kind(),
        )?;

        self.repo_store.save(repo.clone()).await?;
        Ok(repo)
    }

    async fn apply_recipe(
        &self,
        server_id: &ServerId,
        catalog_id: &str,
        _params: HashMap<String, String>,
    ) -> Result<OrchestrationJob, DomainError> {
        let server = self
            .server_store
            .find_by_id(server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            })?;

        let item = self
            .catalog
            .find_catalog_item(catalog_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "CatalogItem",
                id: catalog_id.to_string(),
            })?;

        if let Some(spec) = server.spec() {
            if !item.is_compatible_with(spec) {
                return Err(DomainError::ValidationError(format!(
                    "Recipe '{}' is not compatible with server OS '{}'",
                    item.name(),
                    spec.os
                )));
            }
        }

        let repo = self
            .import_from_catalog(catalog_id, Some(format!("{}-applied", item.id())))
            .await?;

        self.trigger_orchestration_with_options(server_id, &repo.id(), RollbackPolicy::RollbackOnFailure)
            .await
    }

    async fn create_stack(
        &self,
        name: String,
        description: String,
        entries: Vec<StackRepoEntry>,
    ) -> Result<MultiRepoStack, DomainError> {
        for entry in &entries {
            let _ = self.get_repo(&entry.repo_id).await?;
        }

        let stack = MultiRepoStack::new(StackId::new(), name, description, entries)?;
        self.stack_store.save(stack.clone()).await?;
        Ok(stack)
    }

    async fn list_stacks(&self) -> Result<Vec<MultiRepoStack>, DomainError> {
        self.stack_store.list().await
    }

    async fn get_stack(&self, id: &StackId) -> Result<MultiRepoStack, DomainError> {
        self.stack_store
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "MultiRepoStack",
                id: id.to_string(),
            })
    }

    async fn execute_stack(
        &self,
        server_id: &ServerId,
        stack_id: &StackId,
    ) -> Result<OrchestrationJob, DomainError> {
        let mut server = self
            .server_store
            .find_by_id(server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            })?;

        if server.status() != ServerStatus::Online {
            return Err(DomainError::Conflict(format!(
                "Cannot execute stack on server '{}' with status {:?}",
                server.name().as_str(),
                server.status()
            )));
        }

        let stack = self.get_stack(stack_id).await?;
        let steps = stack.compile_to_orchestration_steps();

        let anchor_repo_id = stack
            .entries()
            .first()
            .map(|e| e.repo_id)
            .unwrap_or_else(RepoId::new);

        let mut job = OrchestrationJob::new_with_policy(
            JobId::new(),
            *server_id,
            anchor_repo_id,
            steps,
            RollbackPolicy::RollbackOnFailure,
        )?;

        server.acquire_lock(job.id())?;
        self.server_store.save(server.clone()).await?;

        job.start()?;
        self.job_store.save(job.clone()).await?;

        let step_count = job.steps().len();
        let mut failed = false;

        for i in 0..step_count {
            let step_name = job.steps()[i].step.name.clone();
            let cmd = format!("servai-run-stack-step:{}", step_name);

            match self.executor.execute_command(&server, &cmd).await {
                Ok(res) => {
                    if res.exit_code == 0 {
                        job.record_step_success(i, res.stdout)?;
                    } else {
                        job.record_step_failure(
                            i,
                            format!("Stack step failed: {}", res.exit_code),
                            res.stderr,
                        )?;
                        failed = true;
                        break;
                    }
                }
                Err(err) => {
                    job.record_step_failure(i, "Stack execution error", err.to_string())?;
                    failed = true;
                    break;
                }
            }
        }

        if failed {
            let rollback_steps = job.compensating_rollback_steps();
            for rb in rollback_steps {
                if let Some(cmd) = rb.rollback_command {
                    let _ = self.executor.execute_command(&server, &cmd).await;
                }
            }
            job.record_rollback_completed("Stack rollback finished")?;
        }

        if let Ok(Some(mut fresh)) = self.server_store.find_by_id(server_id).await {
            let _ = fresh.release_lock(job.id());
            let _ = self.server_store.save(fresh).await;
        }

        self.job_store.save(job.clone()).await?;
        Ok(job)
    }

    async fn search_registry(
        &self,
        query: RegistryQuery,
    ) -> Result<Vec<AdminPackage>, DomainError> {
        self.registry.search(&query).await
    }

    async fn list_profile_presets(&self) -> Result<Vec<ServerProfilePreset>, DomainError> {
        self.registry.list_presets().await
    }

    async fn install_package_with_dependencies(
        &self,
        server_id: &ServerId,
        package_id: &str,
        _params: HashMap<String, String>,
    ) -> Result<OrchestrationJob, DomainError> {
        let server = self
            .server_store
            .find_by_id(server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            })?;

        let resolved_packages = self.registry.resolve_dependencies(package_id).await?;

        // Check compatibility
        if let Some(spec) = server.spec() {
            for pkg in &resolved_packages {
                if !pkg.is_compatible_with(spec) {
                    return Err(DomainError::ValidationError(format!(
                        "Package '{}' is not compatible with server OS '{}'",
                        pkg.name(),
                        spec.os
                    )));
                }
            }
        }

        // Import all resolved packages into managed repositories
        let mut entries = Vec::new();
        for (idx, pkg) in resolved_packages.iter().enumerate() {
            let repo = ManagedRepository::new(
                RepoId::new(),
                format!("{}-managed", pkg.id()),
                pkg.repo_url().to_string(),
                GitRef::Branch(pkg.target_ref().to_string()),
                RepoKind::Playbook,
            )?;
            self.repo_store.save(repo.clone()).await?;

            entries.push(StackRepoEntry {
                repo_id: repo.id(),
                order: (idx + 1) as u32,
                step_name: format!("install-{}", pkg.id()),
                command: format!("ansible-playbook -i inventory deploy-{}.yml", pkg.id()),
                rollback_command: Some(format!("ansible-playbook -i inventory rollback-{}.yml", pkg.id())),
                overrides: HashMap::new(),
            });
        }

        let stack = self
            .create_stack(
                format!("{}-bundle-stack", package_id),
                format!("Installation stack for {}", package_id),
                entries,
            )
            .await?;

        self.execute_stack(server_id, &stack.id()).await
    }

    async fn apply_profile_preset(
        &self,
        server_id: &ServerId,
        preset_id: &str,
    ) -> Result<OrchestrationJob, DomainError> {
        let presets = self.registry.list_presets().await?;
        let preset = presets
            .iter()
            .find(|p| p.id() == preset_id)
            .ok_or_else(|| DomainError::NotFound {
                entity: "ServerProfilePreset",
                id: preset_id.to_string(),
            })?;

        let mut entries = Vec::new();
        for (idx, pkg_id) in preset.package_ids().iter().enumerate() {
            let pkg = self
                .registry
                .get_package(pkg_id)
                .await?
                .ok_or_else(|| DomainError::NotFound {
                    entity: "AdminPackage",
                    id: pkg_id.clone(),
                })?;

            let repo = ManagedRepository::new(
                RepoId::new(),
                format!("{}-preset-repo", pkg.id()),
                pkg.repo_url().to_string(),
                GitRef::Branch(pkg.target_ref().to_string()),
                RepoKind::Playbook,
            )?;
            self.repo_store.save(repo.clone()).await?;

            entries.push(StackRepoEntry {
                repo_id: repo.id(),
                order: (idx + 1) as u32,
                step_name: format!("deploy-preset-{}", pkg.id()),
                command: format!("ansible-playbook deploy-{}.yml", pkg.id()),
                rollback_command: None,
                overrides: HashMap::new(),
            });
        }

        let stack = self
            .create_stack(
                format!("{}-preset-stack", preset.id()),
                preset.description().to_string(),
                entries,
            )
            .await?;

        self.execute_stack(server_id, &stack.id()).await
    }

    async fn plan_tofu(
        &self,
        repo_id: &RepoId,
        vars: HashMap<String, String>,
    ) -> Result<TofuPlanResult, DomainError> {
        let repo = self.get_repo(repo_id).await?;
        let path = std::path::Path::new("/tmp").join(repo.name());
        self.tofu_engine.init(&path).await?;
        self.tofu_engine.plan(&path, vars).await
    }

    async fn apply_tofu(
        &self,
        repo_id: &RepoId,
        vars: HashMap<String, String>,
    ) -> Result<TofuApplyResult, DomainError> {
        let repo = self.get_repo(repo_id).await?;
        let path = std::path::Path::new("/tmp").join(repo.name());
        self.tofu_engine.init(&path).await?;
        self.tofu_engine.apply(&path, vars).await
    }

    async fn destroy_tofu(
        &self,
        repo_id: &RepoId,
        vars: HashMap<String, String>,
    ) -> Result<TofuApplyResult, DomainError> {
        let repo = self.get_repo(repo_id).await?;
        let path = std::path::Path::new("/tmp").join(repo.name());
        self.tofu_engine.init(&path).await?;
        self.tofu_engine.destroy(&path, vars).await
    }

    async fn get_tofu_state(&self, repo_id: &RepoId) -> Result<TofuState, DomainError> {
        let repo = self.get_repo(repo_id).await?;
        let path = std::path::Path::new("/tmp").join(repo.name());
        self.tofu_engine.get_state(&path).await
    }

    async fn run_ansible_playbook(
        &self,
        server_id: &ServerId,
        playbook: &str,
        vars: HashMap<String, String>,
    ) -> Result<AnsibleRunResult, DomainError> {
        let server = self
            .server_store
            .find_by_id(server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            })?;
        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot run ansible playbook on offline server {}",
                server_id
            )));
        }
        self.ansible_engine.run_playbook(&server, playbook, vars).await
    }

    async fn apply_crossplane_claim(
        &self,
        claim: CrossplaneClaim,
    ) -> Result<CrossplaneReconcileResult, DomainError> {
        self.crossplane_engine.apply_claim(&claim).await
    }

    async fn get_crossplane_claim_status(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<CrossplaneReconcileResult, DomainError> {
        self.crossplane_engine.get_claim_status(namespace, name).await
    }

    async fn delete_crossplane_claim(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<(), DomainError> {
        self.crossplane_engine.delete_claim(namespace, name).await
    }

    async fn list_crossplane_resources(
        &self,
        namespace: Option<String>,
    ) -> Result<Vec<CrossplaneResource>, DomainError> {
        self.crossplane_engine
            .list_managed_resources(namespace.as_deref())
            .await
    }

    async fn list_crossplane_providers(
        &self,
    ) -> Result<Vec<CrossplaneProviderStatus>, DomainError> {
        self.crossplane_engine.list_providers().await
    }

    async fn list_jobs(
        &self,
        server_id: Option<&ServerId>,
        status: Option<crate::domain::orchestration::ExecutionStatus>,
    ) -> Result<Vec<OrchestrationJob>, DomainError> {
        let all = self.job_store.list_all().await?;
        let filtered = all
            .into_iter()
            .filter(|j| {
                if let Some(sid) = server_id {
                    if j.server_id() != *sid {
                        return false;
                    }
                }
                if let Some(st) = status {
                    if j.status() != st {
                        return false;
                    }
                }
                true
            })
            .collect();
        Ok(filtered)
    }

    async fn get_job_logs(&self, id: &JobId) -> Result<JobExecutionLogs, DomainError> {
        let job = self
            .job_store
            .find_by_id(id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "OrchestrationJob",
                id: id.to_string(),
            })?;

        let steps: Vec<StepLogEntry> = job
            .steps()
            .iter()
            .enumerate()
            .map(|(idx, s)| StepLogEntry {
                step_index: idx,
                name: s.step.name.clone(),
                status: s.status,
                output: s.output.clone(),
                error: s.error.clone(),
                duration_ms: 50,
            })
            .collect();

        let total_duration_ms = (steps.len() as u64) * 50;

        Ok(JobExecutionLogs {
            job_id: *id,
            status: job.status(),
            steps,
            total_duration_ms,
        })
    }

    async fn create_workflow(
        &self,
        name: &str,
        description: &str,
        steps: Vec<WorkflowStep>,
    ) -> Result<WorkflowDefinition, DomainError> {
        let wf = WorkflowDefinition::new(name, description, steps)?;
        self.workflows.write().unwrap().insert(wf.id, wf.clone());
        Ok(wf)
    }

    async fn list_workflows(&self) -> Result<Vec<WorkflowDefinition>, DomainError> {
        Ok(self.workflows.read().unwrap().values().cloned().collect())
    }

    async fn get_workflow(&self, id: &WorkflowId) -> Result<WorkflowDefinition, DomainError> {
        self.workflows
            .read()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| DomainError::NotFound {
                entity: "WorkflowDefinition",
                id: id.to_string(),
            })
    }

    async fn execute_workflow(
        &self,
        server_id: &ServerId,
        workflow_id: &WorkflowId,
    ) -> Result<OrchestrationJob, DomainError> {
        let server = self
            .server_store
            .find_by_id(server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: server_id.to_string(),
            })?;

        if server.status() == ServerStatus::Offline {
            return Err(DomainError::Conflict(format!(
                "Cannot execute workflow on offline server {}",
                server_id
            )));
        }

        let wf = self.get_workflow(workflow_id).await?;
        let job_id = JobId::new();
        let orchestration_steps: Vec<crate::domain::orchestration::OrchestrationStep> = wf
            .steps
            .iter()
            .map(|s| {
                let mut os = crate::domain::orchestration::OrchestrationStep::new(
                    s.name.clone(),
                    s.description.clone(),
                );
                os.rollback_command = s.rollback_command.clone();
                os
            })
            .collect();

        let mut job = OrchestrationJob::new_with_policy(
            job_id,
            *server_id,
            RepoId::new(),
            orchestration_steps,
            RollbackPolicy::RollbackOnFailure,
        )?;
        job.start()?;

        // Execute each step
        for (idx, step) in wf.steps.iter().enumerate() {
            let res = match &step.action {
                WorkflowAction::ShellCommand { command } => {
                    self.executor.execute_command(&server, command).await
                }
                WorkflowAction::DnsRecordUpsert { domain, record_type, value } => {
                    self.executor
                        .execute_command(
                            &server,
                            &format!("servai-dns-record-upsert {} {} {}", domain, record_type, value),
                        )
                        .await
                }
                WorkflowAction::HttpHealthCheck { url, expected_status: _ } => {
                    self.executor
                        .execute_command(
                            &server,
                            &format!("curl -sf -o /dev/null -w '%{{http_code}}' {}", url),
                        )
                        .await
                }
                WorkflowAction::ServiceAction { service, action } => {
                    self.executor
                        .execute_command(&server, &format!("systemctl {} {}", action, service))
                        .await
                }
                WorkflowAction::PackageInstall { package_id } => {
                    self.executor
                        .execute_command(&server, &format!("servai-package-install {}", package_id))
                        .await
                }
            };

            match res {
                Ok(cmd_res) if cmd_res.exit_code == 0 => {
                    let _ = job.record_step_success(idx, cmd_res.stdout);
                }
                Ok(cmd_res) => {
                    let err = format!("Step {} failed with exit code {}", step.name, cmd_res.exit_code);
                    let _ = job.record_step_failure(idx, &err, "Step execution non-zero exit");
                    self.job_store.save(job.clone()).await?;
                    return Ok(job);
                }
                Err(e) => {
                    let err = format!("Step {} failed: {}", step.name, e);
                    let _ = job.record_step_failure(idx, &err, "Step execution error");
                    self.job_store.save(job.clone()).await?;
                    return Ok(job);
                }
            }
        }

        self.job_store.save(job.clone()).await?;
        Ok(job)
    }

    async fn execute_fleet_rollout(
        &self,
        cmd: FleetRolloutCommand,
    ) -> Result<FleetJobResult, DomainError> {
        let servers = self.server_store.list().await?;
        let target_servers: Vec<ServerId> = if let Some(tag) = &cmd.target_tag {
            servers
                .into_iter()
                .filter(|s| s.tags().contains(tag))
                .map(|s| s.id())
                .collect()
        } else if !cmd.server_ids.is_empty() {
            cmd.server_ids
        } else {
            servers.into_iter().map(|s| s.id()).collect()
        };

        let total_targets = target_servers.len();
        let mut succeeded_count = 0;
        let mut failed_count = 0;
        let mut jobs = Vec::new();

        // Build single-step workflow for the fleet action
        let wf_step = WorkflowStep::new("fleet-action", "Action distribuée sur la flotte", cmd.action);
        let wf = self.create_workflow("fleet-workflow", "Fleet rollout", vec![wf_step]).await?;

        for server_id in target_servers {
            match self.execute_workflow(&server_id, &wf.id).await {
                Ok(job) if job.status() == crate::domain::orchestration::ExecutionStatus::Succeeded => {
                    succeeded_count += 1;
                    jobs.push(job);
                }
                Ok(job) => {
                    failed_count += 1;
                    jobs.push(job);
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }

        Ok(FleetJobResult {
            total_targets,
            succeeded_count,
            failed_count,
            jobs,
        })
    }

    async fn create_schedule(
        &self,
        cmd: ScheduledJobCommand,
    ) -> Result<ScheduledJob, DomainError> {
        let schedule = ScheduledJob {
            id: ScheduleId::new(),
            name: cmd.name,
            cron_expression: cmd.cron_expression,
            server_id: cmd.server_id,
            action: cmd.action,
            enabled: true,
            last_run: None,
            created_at: chrono::Utc::now(),
        };
        self.schedules
            .write()
            .unwrap()
            .insert(schedule.id, schedule.clone());
        Ok(schedule)
    }

    async fn list_schedules(&self) -> Result<Vec<ScheduledJob>, DomainError> {
        Ok(self.schedules.read().unwrap().values().cloned().collect())
    }

    async fn get_schedule(&self, id: &ScheduleId) -> Result<ScheduledJob, DomainError> {
        self.schedules
            .read()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| DomainError::NotFound {
                entity: "ScheduledJob",
                id: id.to_string(),
            })
    }

    async fn delete_schedule(&self, id: &ScheduleId) -> Result<(), DomainError> {
        self.schedules.write().unwrap().remove(id);
        Ok(())
    }

    async fn run_schedule_now(&self, id: &ScheduleId) -> Result<OrchestrationJob, DomainError> {
        let schedule = self.get_schedule(id).await?;
        let wf_step = WorkflowStep::new(
            format!("sched-{}", schedule.name),
            "Exécution planifiée programmée",
            schedule.action.clone(),
        );
        let wf = self
            .create_workflow(&format!("run-{}", schedule.name), "Scheduled run", vec![wf_step])
            .await?;

        // Update last_run
        if let Some(s) = self.schedules.write().unwrap().get_mut(id) {
            s.last_run = Some(chrono::Utc::now());
        }

        self.execute_workflow(&schedule.server_id, &wf.id).await
    }

    async fn reconcile_server_state(
        &self,
        desired: DesiredServerState,
    ) -> Result<ReconciliationResult, DomainError> {
        let server = self
            .server_store
            .find_by_id(&desired.server_id)
            .await?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Server",
                id: desired.server_id.to_string(),
            })?;

        let mut remediation_jobs = Vec::new();

        // 1. Reconcile packages
        for pkg_id in &desired.required_packages {
            let job = self
                .install_package_with_dependencies(&desired.server_id, pkg_id, HashMap::new())
                .await?;
            remediation_jobs.push(job);
        }

        // 2. Reconcile bound domain
        if let Some(domain) = &desired.bound_domain {
            let step = WorkflowStep::new(
                "reconcile-domain",
                format!("Bind domain {}", domain),
                WorkflowAction::DnsRecordUpsert {
                    domain: domain.clone(),
                    record_type: "A".to_string(),
                    value: server.host().ip().to_string(),
                },
            );
            let wf = self.create_workflow("domain-reconcile", "Bind domain", vec![step]).await?;
            let job = self.execute_workflow(&desired.server_id, &wf.id).await?;
            remediation_jobs.push(job);
        }

        // 3. Reconcile WireGuard
        if desired.wireguard_enabled {
            let step = WorkflowStep::new(
                "reconcile-wireguard",
                "Ensure WireGuard VPN is active",
                WorkflowAction::ServiceAction {
                    service: "wg-quick@wg0".to_string(),
                    action: "start".to_string(),
                },
            );
            let wf = self.create_workflow("wg-reconcile", "Ensure WireGuard", vec![step]).await?;
            let job = self.execute_workflow(&desired.server_id, &wf.id).await?;
            remediation_jobs.push(job);
        }

        let drift_detected = !remediation_jobs.is_empty();

        Ok(ReconciliationResult {
            server_id: desired.server_id,
            drift_detected,
            remediation_jobs,
        })
    }
}
