# Servai

**Servai** est un orchestrateur en Rust conçu pour l'administration intégrale de serveurs à partir de dépôts de configuration existants (Ansible Playbooks, Docker Compose, manifests Shell, configurations brutes). Il intègre un **catalogue standardisé de dépôts d'administration système** et expose une API REST réactive conçue pour être consommée directement par l'interface **Bioxus**.

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
|    - CatalogItem, ConfigVariable, AdminCategory                         |
|    - ManagedRepository, RepoId, GitRef, RepoKind, SyncStatus            |
|    - OrchestrationJob, JobId, StepExecution, ExecutionStatus            |
|    - RollbackPolicy, CompensatingSteps, DriftReport                     |
|    - DomainError (Validation, NotFound, Conflict, ExecutionFailed)      |
|                                                                         |
|  Driven Ports                        v                                  |
|  [ServerStore]   [RepoStore]   [JobStore]   [Executor]   [CatalogPort]  |
+-------------------------------------------------------------------------+
        |               |             |            |              |
        v               v             v            v              v
+---------------+ +-----------+ +-----------+ +----------+ +--------------+
| InMemoryStore | | InMemory  | | InMemory  | | SSH      | | Standard     |
| / SQLite      | | Store     | | Job Store | | Executor | | AdminCatalog |
+---------------+ +-----------+ +-----------+ +----------+ +--------------+
```

---

## 📦 Réutilisation Maximale de Dépôts d'Administration Serveur

Servai intègre nativement un registre de dépôts et collections open source éprouvés pour l'administration de serveurs :

| Recette / Dépôt | Catégorie | Dépôt Amont Réutilisé | Rôle |
| :--- | :--- | :--- | :--- |
| `linux-hardening` | `Security` | [dev-sec/ansible-collection-hardening](https://github.com/dev-sec/ansible-collection-hardening.git) | Hardening Linux benchmark CIS (SSH, sysctl, auditd, limits). |
| `docker-engine-compose` | `Containers` | [geerlingguy/ansible-role-docker](https://github.com/geerlingguy/ansible-role-docker.git) | Moteur Docker & Docker Compose en production. |
| `prometheus-node-exporter`| `Monitoring` | [prometheus-community/ansible-node-exporter](https://github.com/prometheus-community/ansible-node-exporter.git) | Collecte des métriques physiques et OS de serveurs. |
| `fail2ban-ufw-security` | `Security` | [geerlingguy/ansible-role-security](https://github.com/geerlingguy/ansible-role-security.git) | Pare-feu UFW persistant et bannissement automatique Fail2ban. |
| `traefik-proxy` | `Networking` | [traefik/traefik](https://github.com/traefik/traefik.git) | Reverse proxy edge router avec certificats Let's Encrypt automatiques. |
| `wireguard-vpn` | `Networking` | [angristan/wireguard-install](https://github.com/angristan/wireguard-install.git) | Passerelle VPN sécurisée haute performance. |
| `borgbackup-server` | `Backup` | [borgbackup/borg](https://github.com/borgbackup/borg.git) | Sauvegardes chiffrées dédupliquées automatisées. |
| `caddy-webserver` | `Web` | [caddyserver/caddy](https://github.com/caddyserver/caddy.git) | Serveur HTTP/3 sécurisé à mémoire saine avec TLS automatique. |

---

## 🧪 Tests TDD (33/33 Passing - 100% Couverture)

Pour exécuter l'ensemble des suites de tests TDD :
```bash
cargo test
```

### Découpage des suites de tests :
- **`tests/domain_tests.rs` (13 tests)** : Cycle de vie, verrous, validation, métriques, rollback, dérive.
- **`tests/catalog_tests.rs` (2 tests)** : Validations du catalogue, compatibilité OS des recettes.
- **`tests/application_tests.rs` (11 tests)** : Administration, healthchecks, services systemd, métriques, rollback, drift, navigation et import catalogue, application directe de recettes aux serveurs.
- **`tests/api_bioxus_tests.rs` (7 tests)** : Contrats REST complets pour l'UI Bioxus (200, 201, 400, 404, 409, consultation catalogue, import, déploiement recette).

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
| `POST` | `/api/v1/servers/:id/recipes/:recipe_id/apply` | **Déployer une recette d'administration en un clic sur le serveur** |
| `GET` | `/api/v1/catalog` | **Parcourir les dépôts d'administration (filtrable par `?category=`)** |
| `POST` | `/api/v1/catalog/:id/import` | **Importer un dépôt du catalogue dans Servai** |
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
