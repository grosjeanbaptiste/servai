# Servai

**Servai** est un orchestrateur en Rust dédié à l'administration intégrale de serveurs à partir de dépôts de configuration existants (Ansible Playbooks, Docker Compose, manifests Shell, configurations brutes). Il expose une API REST réactive conçue pour être consommée directement par l'interface **Bioxus**.

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
|    - ManagedRepository, RepoId, GitRef, RepoKind, SyncStatus            |
|    - OrchestrationJob, JobId, StepExecution, ExecutionStatus            |
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

## 🧪 Tests TDD (100% Passing)

Pour lancer l'intégralité de la suite de tests :
```bash
cargo test
```

### Découpage des suites de tests :
- **`tests/domain_tests.rs`** : Validation stricte des invariants métier, state machine du cycle de vie serveur, validation syntaxique, cycle de vie des jobs d'orchestration.
- **`tests/application_tests.rs`** : Cas d'utilisation applicatifs avec doubles de test (healthchecks, rejet d'orchestration sur serveur hors ligne, flux nominal d'exécution).
- **`tests/api_bioxus_tests.rs`** : Contrats d'API complets pour l'UI Bioxus (codes HTTP 200, 201, 400, 404, 409, sérialisation JSON).

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
| `POST` | `/api/v1/repos` | Enregistrer un dépôt existant |
| `GET` | `/api/v1/repos` | Lister les dépôts orchestrés |
| `POST` | `/api/v1/repos/:id/sync` | Synchroniser le dépôt Git |
| `POST` | `/api/v1/orchestrations` | Déclencher l'orchestration sur un serveur |
| `GET` | `/api/v1/orchestrations/:id` | Suivi et journaux d'exécution du job |

---

## 🚀 Démarrage du serveur

```bash
cargo run
```
Le serveur écoute par défaut sur `http://0.0.0.0:8080` avec le support CORS activé pour l'UI **Bioxus**.
