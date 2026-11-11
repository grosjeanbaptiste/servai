# Servai

**Servai** est un orchestrateur en Rust conçu pour l'administration intégrale de serveurs en **réutilisant un maximum de dépôts d'administration existants** (Ansible Galaxy, LinuxServer.io, collections GitHub Sysadmin & Homelab). Il intègre un **catalogue standardisé de dépôts d'administration**, un **moteur d'écosystème multi-sources (Ansible Galaxy, LinuxServer.io, GitHub Community)** avec **résolution automatique de dépendances**, un système de **profils prédéfinis (Presets)**, un moteur de **composition multi-dépôts (Admin Stacks)** et expose une API REST réactive conçue pour être consommée directement par l'interface **Bioxus**.

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
|    - AdminPackage, RegistryQuery, ServerProfilePreset, RegistrySource   |
|    - MultiRepoStack, StackId, StackRepoEntry                            |
|    - CatalogItem, ConfigVariable, AdminCategory                         |
|    - ManagedRepository, RepoId, GitRef, RepoKind, SyncStatus            |
|    - OrchestrationJob, JobId, StepExecution, ExecutionStatus            |
|    - RollbackPolicy, CompensatingSteps, DriftReport                     |
|    - DomainError (Validation, NotFound, Conflict, ExecutionFailed)      |
|                                                                         |
|  Driven Ports                        v                                  |
|  [ServerStore] [RepoStore] [JobStore] [StackStore] [Registry] [Catalog] |
+-------------------------------------------------------------------------+
        |            |           |           |           |          |
        v            v           v           v           v          v
+---------------+ +--------+ +-------+ +----------+ +--------+ +-----------+
| InMemoryStore | | Mem    | | Mem   | | MemStack | | Multi  | | Standard  |
| / SQLite      | | Repos  | | Jobs  | | Store    | | Source | | Catalog   |
|               | |        | |       | |          | | Reg.   | |           |
+---------------+ +--------+ +-------+ +----------+ +--------+ +-----------+
```

---

## 📦 Réutilisation Maximale de Dépôts d'Administration

Servai capitalise sur l'écosystème open source existant pour éviter toute réinvention de la roue :

### 1. Registre Multi-Sources Intégré
- **Ansible Galaxy** : Rôles et collections de référence ([dev-sec/ansible-collection-hardening](https://github.com/dev-sec/ansible-collection-hardening.git), [geerlingguy/ansible-role-security](https://github.com/geerlingguy/ansible-role-security.git), [geerlingguy/ansible-role-docker](https://github.com/geerlingguy/ansible-role-docker.git), [geerlingguy/ansible-role-nginx](https://github.com/geerlingguy/ansible-role-nginx.git), [geerlingguy/ansible-role-postgresql](https://github.com/geerlingguy/ansible-role-postgresql.git)).
- **LinuxServer.io** : Dépôts de services conteneurisés sécurisés ([linuxserver/docker-wireguard](https://github.com/linuxserver/docker-wireguard.git), [linuxserver/docker-nextcloud](https://github.com/linuxserver/docker-nextcloud.git), [linuxserver/docker-swag](https://github.com/linuxserver/docker-swag.git)).
- **GitHub Community & Cloud-Native** : [traefik/traefik](https://github.com/traefik/traefik.git), [caddyserver/caddy](https://github.com/caddyserver/caddy.git), [prometheus-community/ansible-node-exporter](https://github.com/prometheus-community/ansible-node-exporter.git), [netdata/netdata](https://github.com/netdata/netdata.git), [crowdsecurity/crowdsec](https://github.com/crowdsecurity/crowdsec.git), [borgbackup/borg](https://github.com/borgbackup/borg.git), [restic/restic](https://github.com/restic/restic.git), [k3s-io/k3s-ansible](https://github.com/k3s-io/k3s-ansible.git), [dani-garcia/vaultwarden](https://github.com/dani-garcia/vaultwarden.git), [louislam/uptime-kuma](https://github.com/louislam/uptime-kuma.git), [grafana/grafana](https://github.com/grafana/grafana.git).

### 2. Résolution Automatique de Dépendances Inter-Dépôts
Lorsqu'un dépôt d'administration est sélectionné (ex: `linuxserver-wireguard` ou `traefik-proxy`), Servai résout automatiquement ses dépendances d'infrastructure (ex: `geerlingguy-docker`), injecte les étapes préalables dans le bon ordre d'exécution et gère la compensation de rollback inverse en cas de défaillance.

### 3. Profils de Serveur Prédéfinis (Presets)
- `secure-web-baseline` : Hardening Linux CIS + UFW/Fail2ban + Caddy auto-HTTPS.
- `docker-application-host` : Hardening Linux + Pare-feu + Docker Engine + Traefik v3.
- `observability-node` : Prometheus Node Exporter + Netdata temps réel + Grafana.
- `homelab-private-cloud` : Hardening + Docker + WireGuard VPN + Nextcloud + Vaultwarden.

---

## 🧪 Tests TDD (71/71 Passing - 100% Couverture)

Pour exécuter l'ensemble des suites de tests TDD :
```bash
cargo test
```

### Découpage des 8 suites de tests :
- **`tests/domain_tests.rs` (13 tests)** : Cycle de vie, verrous de concurrence, validation, métriques, rollback, dérive.
- **`tests/catalog_tests.rs` (2 tests)** : Validations du catalogue, compatibilité OS des recettes.
- **`tests/inspector_and_stack_tests.rs` (3 tests)** : Manifestes de détection d'administration, composition multi-dépôts, compilation des étapes.
- **`tests/registry_tests.rs` (3 tests)** : Résolution de dépendances entre dépôts, requêtes de recherche multi-critères, profils prédéfinis.
- **`tests/engine_tests.rs` (8 tests)** : Moteurs d'exécution OpenTofu (plan, apply, state, summary), Ansible (playbooks, task events, module runner, facts gathering), Crossplane (claims, conditions, XRD, managed resources) et WireGuard (interfaces wg-quick, pairs cryptographiques, pair keepalive, device status).
- **`tests/dns_and_registrar_tests.rs` (7 tests)** : Enregistrement de noms de domaine auprès des registrars (Cloudflare, OVH, Gandi, Namecheap, Route53, Porkbun), configuration des serveurs DNS (nameservers), gestion des zones DNS et enregistrements (A, AAAA, CNAME, MX, TXT, SRV, CAA), validation du TTL et DNSSEC, propagation DNS, cycle de vie des certificats TLS/ACME (DNS-01, HTTP-01), et liaison unifiée domaine-serveur avec auto-provisioning SSL (`bind_server_domain`).
- **`tests/application_tests.rs` (22 tests)** : Administration, healthchecks, services systemd, métriques, reboot, rollback automatique, dérive, navigation et import catalogue, application directe de recettes, exécution de stacks, recherche dans le registre, installation avec dépendances automatiques, déploiement de profils complets, orchestration OpenTofu (plan, apply, destroy, state), exécution Ansible (playbook, module ad-hoc, inventaire et faits), orchestration Crossplane (claims, status, providers, managed resources), gestion WireGuard (configuration interface, ajout/suppression de pairs, monitoring du trafic, génération de clés).
- **`tests/api_bioxus_tests.rs` (14 tests)** : Contrats REST complets pour l'UI Bioxus (200, 201, 204, 400, 404, 409, 422, catalogue, recettes, stacks, recherche registre, déploiement de presets, API OpenTofu, Ansible, Crossplane, WireGuard, Registrar, DNS et Server-Domain Binding avec auto-SSL).

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
| `POST` | `/api/v1/servers/:id/bind-domain` | **Lier un domaine au serveur, créer le record DNS A et délivrer le certificat TLS Let's Encrypt (Auto-SSL)** |
| `GET` | `/api/v1/domains/check?domain=:name` | **Vérifier la disponibilité et le prix d'un nom de domaine auprès du Registrar** |
| `POST` | `/api/v1/domains` | **Acheter / Enregistrer un nom de domaine (Cloudflare, OVH, Gandi, Namecheap, Route53, Porkbun)** |
| `GET` | `/api/v1/domains` | **Lister les noms de domaine gérés** |
| `GET` | `/api/v1/domains/:domain` | **Détails d'enregistrement d'un domaine** |
| `PUT` | `/api/v1/domains/:domain/nameservers` | **Mettre à jour les serveurs de noms (Nameservers) du domaine** |
| `PUT` | `/api/v1/domains/:domain/auto-renew` | **Activer ou désactiver le renouvellement automatique du domaine** |
| `POST` | `/api/v1/dns/zones` | **Créer une zone DNS avec ou sans DNSSEC** |
| `GET` | `/api/v1/dns/zones` | **Lister les zones DNS** |
| `GET` | `/api/v1/dns/zones/:domain` | **Inspecter une zone DNS et tous ses enregistrements** |
| `POST` | `/api/v1/dns/zones/:domain/records` | **Créer / Mettre à jour un enregistrement DNS (A, AAAA, CNAME, MX, TXT, etc.)** |
| `DELETE` | `/api/v1/dns/zones/:domain/records/:id` | **Supprimer un enregistrement DNS** |
| `GET` | `/api/v1/dns/zones/:domain/propagate` | **Vérifier en temps réel la propagation mondiale d'un enregistrement DNS** |
| `POST` | `/api/v1/servers/:id/wireguard` | **Configurer une interface WireGuard (`wg0`) et ses pairs** |
| `GET` | `/api/v1/servers/:id/wireguard/:iface/status` | **Statut, pairs actifs et métriques réseau Rx/Tx WireGuard** |
| `GET` | `/api/v1/servers/:id/wireguard/:iface/peers` | **Lister les pairs VPN autorisés sur l'interface** |
| `POST` | `/api/v1/servers/:id/wireguard/:iface/peers` | **Ajouter dynamiquement un pair VPN WireGuard** |
| `DELETE` | `/api/v1/servers/:id/wireguard/:iface/peers/:pubkey` | **Révoquer/supprimer un pair VPN WireGuard** |
| `POST` | `/api/v1/wireguard/keypair` | **Générer une paire de clés privées/publiques WireGuard sécurisées** |
| `POST` | `/api/v1/servers/:id/ansible/playbook` | **Exécuter un playbook Ansible avec variables dynamiques** |
| `POST` | `/api/v1/servers/:id/ansible/module` | **Exécuter une commande/module ad-hoc Ansible** |
| `GET` | `/api/v1/servers/:id/ansible/facts` | **Collecter les faits matériels et système Ansible du serveur** |
| `POST` | `/api/v1/tofu/plan` | **Calculer un plan d'infrastructure OpenTofu** |
| `POST` | `/api/v1/tofu/apply` | **Appliquer une configuration OpenTofu (auto-approve)** |
| `POST` | `/api/v1/tofu/destroy` | **Détruire les ressources gérées par OpenTofu** |
| `GET` | `/api/v1/tofu/state/:repo_id` | **Inspecter l'état (State/Lineage) OpenTofu en JSON** |
| `POST` | `/api/v1/crossplane/claims` | **Déployer un Claim Crossplane (Composite Resource Claim)** |
| `GET` | `/api/v1/crossplane/claims/:namespace/:name` | **Inspecter le statut, conditions et readiness d'un Claim Crossplane** |
| `DELETE` | `/api/v1/crossplane/claims/:namespace/:name` | **Supprimer un Claim Crossplane et libérer les ressources managées** |
| `GET` | `/api/v1/crossplane/resources` | **Lister les ressources managées Crossplane (`Managed Resources`)** |
| `GET` | `/api/v1/crossplane/providers` | **Vérifier l'état et la santé des Providers Crossplane (`provider-aws`, etc.)** |
| `GET` | `/api/v1/registry/search` | **Recherche dans l'écosystème de dépôts (Galaxy, LinuxServer, GitHub)** |
| `GET` | `/api/v1/registry/presets` | **Lister les profils de serveur complets (Presets multi-dépôts)** |
| `POST` | `/api/v1/servers/:id/packages/:package_id/install` | **Installer un dépôt avec résolution automatique de ses dépendances** |
| `POST` | `/api/v1/servers/:id/presets/:preset_id/apply` | **Déployer un profil complet multi-dépôts sur le serveur en un clic** |
| `POST` | `/api/v1/servers/:id/recipes/:recipe_id/apply` | Déployer une recette d'administration standard |
| `POST` | `/api/v1/servers/:id/stacks/:stack_id/apply` | Appliquer un stack multi-dépôts sur le serveur |
| `GET` | `/api/v1/catalog` | Parcourir les dépôts d'administration |
| `POST` | `/api/v1/catalog/:id/import` | Importer un dépôt du catalogue dans Servai |
| `POST` | `/api/v1/stacks` | Créer un stack combinant plusieurs dépôts existants |
| `GET` | `/api/v1/stacks` | Lister les stacks multi-dépôts configurés |
| `POST` | `/api/v1/repos` | Enregistrer un dépôt existant personnalisé (Ansible, Docker Compose, OpenTofu, Crossplane, etc.) |
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
