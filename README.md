# Servai

**Servai** est un orchestrateur en Rust conçu pour l'administration intégrale de serveurs à partir de dépôts de configuration existants (Ansible Playbooks, Docker Compose, manifests Shell, configurations brutes). Il expose une API REST réactive conçue pour être consommée directement par l'interface **Bioxus**.

Le projet est développé selon une méthodologie **100% pure TDD (Test-Driven Development)** et suit rigoureusement les principes de l'**Architecture Hexagonale (Ports & Adapteurs)**.

---

## 🏛️ Architecture Hexagonale

```text
                           +------------------------+
                           |       UI Bioxus        |
                           +-----------+------------+
                                       |
                                       v (REST JSON)
                 +--------------------------------------------+
                 |       Inbound Adapter (Axum HTTP)          |
                 +---------------------+----------------------+
                                       |
                   Driving Ports       v
+-------------------------------------------------------------------------+
|  [ServerAdminPort]                              [RepoOrchestratorPort]  |
|                                                                         |
|  ================ Application Services (Use Cases) ===================  |
|          ServerService                     OrchestrationService         |
|                                                                         |
|  ========================= Domain Model ==============================  |
|    - Server, ServerId, HostAddress, ServerStatus, ServerSpec            |
|    - ServerMetrics, ServiceAction, ServiceStatus, ServerLock            |
|    - ManagedRepository, RepoId, GitRef, RepoKind, SyncStatus            |
|    - OrchestrationJob, JobId, StepExecution, ExecutionStatus            |
|    - RollbackPolicy, CompensatingSteps, DriftReport                     |
|    - DomainError (Validation, NotFound, Conflict, ExecutionFailed)      |
|                                                                         |
|  Driven Ports                        v                                  |
|  [ServerStorePort]   [RepoStorePort]   [JobStorePort]   [ExecutorPort]  |
+-------------------------------------------------------------------------+
        |                    |                 |                 |
        v                    v                 v                 v
+---------------+    +---------------+  +-------------+  +----------------+
| InMemoryStore |    | InMemoryStore |  | InMemoryJob |  | SshExecutor    |
| / SQLite      |    | / SQLite      |  | Store       |  | GitCliProvider |
+---------------+    +---------------+  +-------------+  +----------------+
```

---

## 🧪 Tests TDD (28/28 Passing - 100% Couverture)

Pour exécuter l'ensemble des suites de tests TDD :
```bash
cargo test
```

### Découpage des suites de tests :
- **`tests/domain_tests.rs` (13 tests)** :
  - Validation stricte des invariants métier, state machine du cycle de vie serveur (`Pending`, `Online`, `Maintenance`, `Degraded`, `Offline`).
  - Verrous de concurrence serveur (`acquire_lock` / `release_lock`).
  - Validation et pourcentages des métriques (`ServerMetrics`).
  - Commandes et états de services système (`ServiceAction`, `ServiceStatus`).
  - Machine à états d'orchestration, gestion des échecs, annulation (`cancel`).
  - Politique de compensation et rollback automatique (`RollbackOnFailure`, `RolledBack`).
  - Rapports de dérive de configuration (`DriftReport`).
- **`tests/application_tests.rs` (9 tests)** :
  - Enregistrement, healthcheck dynamique et détection des nœuds injoignables.
  - Gestion des services systemd (`manage_service`).
  - Collecte des métriques en temps réel (`get_metrics`).
  - Redémarrage sécurisé avec transition en maintenance (`reboot_server`).
  - Rejet d'orchestration sur serveur hors ligne.
  - Orchestration avec rollback automatique et libération du verrou serveur.
  - Détection de drift en continu (`detect_drift`).
- **`tests/api_bioxus_tests.rs` (6 tests)** :
  - Contrats d'API complets consommés par l'UI Bioxus.
  - Codes HTTP conformes : 200, 201, 400 (Bad Request), 404 (Not Found), 409 (Conflict).

---

## 🌐 Endpoints API Bioxus

| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/api/v1/health` | Statut de l'API Servai |
| `POST` | `/api/v1/servers` | Enregistrer un nouveau serveur |
| `GET` | `/api/v1/servers` | Lister les serveurs enregistrés |
| `GET` | `/api/v1/servers/:id` | Détails d'un serveur |
| `POST` | `/api/v1/servers/:id/health` | Ping & détection des specs système |
| `POST` | `/api/v1/servers/:id/maintenance` | Activer/désactiver le mode maintenance |
| `POST` | `/api/v1/servers/:id/execute` | Exécuter une commande administrative |
| `GET` | `/api/v1/servers/:id/metrics` | Métriques temps réel (CPU, RAM, Disque, Uptime) |
| `POST` | `/api/v1/servers/:id/services/:service/action` | Gérer un service (`Start`, `Stop`, `Restart`, `Reload`, `Status`) |
| `POST` | `/api/v1/servers/:id/reboot` | Déclencher le redémarrage du serveur |
| `POST` | `/api/v1/repos` | Enregistrer un dépôt existant |
| `GET` | `/api/v1/repos` | Lister les dépôts orchestrés |
| `POST` | `/api/v1/repos/:id/sync` | Synchroniser le dépôt Git |
| `POST` | `/api/v1/orchestrations` | Déclencher l'orchestration sur un serveur |
| `GET` | `/api/v1/orchestrations/:id` | Suivi et journaux d'exécution du job |
| `POST` | `/api/v1/orchestrations/:id/cancel` | Annuler un job d'orchestration en cours |
| `POST` | `/api/v1/orchestrations/drift` | Détecter les écarts de configuration (Drift) |

---

## 🚀 Démarrage du serveur

```bash
cargo run
```
Le serveur écoute par défaut sur `http://0.0.0.0:8080` avec le support CORS activé pour l'UI **Bioxus**.
