# Servai

**Servai** est un orchestrateur en Rust conçu pour l'administration intégrale de serveurs à partir de dépôts de configuration existants (Ansible Playbooks, Docker Compose, manifests Shell, configurations brutes). Il intègre un **catalogue standardisé de dépôts d'administration système**, un moteur de **composition multi-dépôts (Admin Stacks)** et expose une API REST réactive conçue pour être consommée directement par l'interface **Bioxus**.

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
|    - MultiRepoStack, StackId, StackRepoEntry                            |
|    - CatalogItem, ConfigVariable, AdminCategory                         |
|    - ManagedRepository, RepoId, GitRef, RepoKind, SyncStatus            |
|    - OrchestrationJob, JobId, StepExecution, ExecutionStatus            |
|    - RollbackPolicy, CompensatingSteps, DriftReport                     |
|    - DomainError (Validation, NotFound, Conflict, ExecutionFailed)      |
|                                                                         |
|  Driven Ports                        v                                  |
|  [ServerStore] [RepoStore] [JobStore] [StackStore] [Executor] [Catalog] |
+-------------------------------------------------------------------------+
        |            |           |           |           |          |
        v            v           v           v           v          v
+---------------+ +--------+ +-------+ +----------+ +--------+ +-----------+
| InMemoryStore | | Mem    | | Mem   | | MemStack | | SSH    | | Standard  |
| / SQLite      | | Repos  | | Jobs  | | Store    | | Remote | | Catalog   |
+---------------+ +--------+ +-------+ +----------+ +--------+ +-----------+
```

---

## 📦 Réutilisation & Composition de Dépôts d'Administration

Servai permet de réutiliser et d'enchaîner n'importe quel dépôt d'administration système existant :

### 1. Dépôts Ouverts Intégrés Nativement (Catalogue)
- **Hardening Linux** : [dev-sec/ansible-collection-hardening](https://github.com/dev-sec/ansible-collection-hardening.git) (CIS benchmark).
- **Moteur Docker & Compose** : [geerlingguy/ansible-role-docker](https://github.com/geerlingguy/ansible-role-docker.git).
- **Surveillance & Métriques** : [prometheus-community/ansible-node-exporter](https://github.com/prometheus-community/ansible-node-exporter.git).
- **Sécurité Réseau & Pare-feu** : [geerlingguy/ansible-role-security](https://github.com/geerlingguy/ansible-role-security.git) (UFW + Fail2ban).
- **Edge Router & Reverse Proxy** : [traefik/traefik](https://github.com/traefik/traefik.git) & [caddyserver/caddy](https://github.com/caddyserver/caddy.git).
- **VPN Sécurisé** : [angristan/wireguard-install](https://github.com/angristan/wireguard-install.git).
- **Sauvegardes Dédupliquées** : [borgbackup/borg](https://github.com/borgbackup/borg.git).

### 2. Multi-Repo Stacks (Composition Atomique)
Combinez plusieurs dépôts d'administration existants dans un pipeline séquentiel unique :
- Exécution ordonnée (`order: 1`, `order: 2`, etc.).
- Surcharge de variables par étape.
- **Rollback automatique inversé** : si une étape d'un dépôt échoue, toutes les étapes réussies des dépôts précédents sont annulées dans l'ordre inverse.

---

## 🧪 Tests TDD (38/38 Passing - 100% Couverture)

Pour exécuter l'ensemble des suites de tests TDD :
```bash
cargo test
```

### Découpage des suites de tests :
- **`tests/domain_tests.rs` (13 tests)** : Cycle de vie, verrous de concurrence, validation, métriques, rollback, dérive.
- **`tests/catalog_tests.rs` (2 tests)** : Validations du catalogue, compatibilité OS des recettes.
- **`tests/inspector_and_stack_tests.rs` (3 tests)** : Manifestes de détection d'administration, composition multi-dépôts, compilation des étapes.
- **`tests/application_tests.rs` (12 tests)** : Administration, healthchecks, services systemd, métriques, rollback, drift, navigation et import catalogue, application directe de recettes, exécution de stacks multi-dépôts.
- **`tests/api_bioxus_tests.rs` (8 tests)** : Contrats REST complets pour l'UI Bioxus (200, 201, 400, 404, 409, catalogue, import, recettes, création et application de stacks).

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
| `POST` | `/api/v1/servers/:id/recipes/:recipe_id/apply` | Déployer une recette d'administration en un clic sur le serveur |
| `POST` | `/api/v1/servers/:id/stacks/:stack_id/apply` | **Appliquer un stack multi-dépôts sur le serveur** |
| `GET` | `/api/v1/catalog` | Parcourir les dépôts d'administration (filtrable par `?category=`) |
| `POST` | `/api/v1/catalog/:id/import` | Importer un dépôt du catalogue dans Servai |
| `POST` | `/api/v1/stacks` | **Créer un stack combinant plusieurs dépôts existants** |
| `GET` | `/api/v1/stacks` | **Lister les stacks multi-dépôts configurés** |
| `POST` | `/api/v1/repos` | Enregistrer un dépôt existant personnalisé |
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
