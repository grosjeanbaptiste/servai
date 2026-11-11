# Servai

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg?logo=rust)](https://www.rust-lang.org)
[![TDD](https://img.shields.io/badge/methodology-100%25%20Pure%20TDD-brightgreen.svg)](https://en.wikipedia.org/wiki/Test-driven_development)
[![Tests](https://img.shields.io/badge/tests-79%2F79%20passing-success.svg)](tests/)
[![Architecture](https://img.shields.io/badge/architecture-Hexagonal%20%28Ports%20%26%20Adapters%29-blue.svg)](#-architecture-hexagonale)
[![UI](https://img.shields.io/badge/UI-Bioxus%20Ready-6f42c1.svg)](#-endpoints-api-bioxus)

**Servai** est un orchestrateur universel en Rust conçu pour l'administration intégrale de serveurs. Il unifie l'ensemble de la chaîne de valeur système et cloud : **noms de domaine (Registrars)**, **zones et enregistrements DNS**, **délivrance de certificats TLS/SSL Let's Encrypt**, **tunnels VPN WireGuard**, moteurs d'automatisation déclarative (**OpenTofu**, **Ansible**, **Crossplane**), et **réutilisation maximale de dépôts d'administration existants** (Ansible Galaxy, LinuxServer.io, collections GitHub Sysadmin & Homelab).

Le projet expose une API REST réactive conçue pour alimenter directement l'interface utilisateur **Bioxus**, et est développé selon une méthodologie **100% pure TDD (Test-Driven Development)** dans le respect strict des principes de l'**Architecture Hexagonale**.

---

## 🏛️ Architecture Hexagonale

```text
                               +----------------------------+
                               |         UI Bioxus          |
                               +--------------+-------------+
                                              |
                                              v (REST JSON)
                         +----------------------------------------+
                         |      Inbound Adapter (Axum HTTP)       |
                         +--------------------+-------------------+
                                              |
      Driving Inbound Ports                   v
+-----------------------------------------------------------------------------------------------+
|  [ServerAdminPort]    [RepoOrchestratorPort]    [DnsAdminPort]    [RegistrarAdminPort]        |
|                                                                                               |
|  =========================== Application Services (Use Cases) ==============================  |
|     ServerService       OrchestrationService       DnsService       RegistrarService          |
|                                                                                               |
|  ================================== Pure Domain Model ======================================  |
|    - Server, ServerId, HostAddress, ServerStatus, ServerMetrics, ServiceAction                |
|    - DomainRegistration, RegistrarProvider, ContactInfo, NameserverConfig, DomainStatus       |
|    - DnsZone, DnsRecord, DnsRecordType, Ttl, DnssecStatus, DnsPropagation                     |
|    - TlsCertificate, ChallengeType, CertStatus (ACME Let's Encrypt)                           |
|    - OpenTofuEngine, AnsibleEngine, CrossplaneEngine, WireGuardEngine                         |
|    - AdminPackage, RegistryQuery, ServerProfilePreset, MultiRepoStack, CatalogItem            |
|    - ManagedRepository, OrchestrationJob, StepExecution, DriftReport, RollbackPolicy         |
|    - DomainError (Validation, NotFound, Conflict, ExecutionFailed)                            |
|                                                                                               |
|  Driven Outbound Ports                      v                                                 |
|  [ServerStore]   [RepoStore]    [JobStore]      [ServerExecutor]    [GitProvider]             |
|  [OpenTofuEngine][AnsibleEngine][CrossplaneEngine][WireGuardEngine] [DnsProvider] [Registrar] |
+-----------------------------------------------------------------------------------------------+
         |              |              |                 |                   |             |
         v              v              v                 v                   v             v
   +-----------+  +-----------+  +------------+   +--------------+    +------------+ +------------+
   | InMemory  |  | InMemory  |  | SSH Exec   |   | CLI Adapters |    | DNS Clis / | | Registrars |
   | / SQLite  |  | Jobs/Repos|  | / Systemd  |   | Tofu/Ansible |    | Cloudflare | | OVH/Gandi/ |
   | Stores    |  | / Stacks  |  | & Metrics  |   | Xplane/WG    |    | Route53    | | Namecheap  |
   +-----------+  +-----------+  +------------+   +--------------+    +------------+ +------------+
```

---

## 🚀 Capacités d'Orchestration Intégrale

### 1. Registrar, DNS & Certificats Auto-TLS
- **Registrar multi-fournisseurs** : Vérification de la disponibilité et des tarifs, commande/enregistrement, gestion des serveurs de noms (Nameservers) et bascule de renouvellement automatique (Cloudflare, OVH, Gandi, Namecheap, Route53, Porkbun).
- **Gestion des Zones & Enregistrements DNS** : Création de zones avec support DNSSEC, gestion granulaire des enregistrements (`A`, `AAAA`, `CNAME`, `MX`, `TXT`, `SRV`, `CAA`, `NS`, `PTR`, `SOA`) et vérification de la propagation DNS mondiale en direct.
- **Certificats TLS / ACME automatiques** : Provisioning de certificats Let's Encrypt via challenge DNS-01 ou HTTP-01 avec suivi du nombre de jours de validité restants.
- **Liaison Serveur-Domaine en un clic (`bind_server_domain`)** : Associe instantanément un serveur à son domaine, crée les enregistrements DNS A correspondants et émet le certificat SSL TLS valide.

### 2. Moteurs d'Automatisation Système & Cloud-Native
- **OpenTofu / Terraform** : `plan`, `apply`, `destroy` et inspection de l'état (`state`) avec détection des changements (ressources ajoutées, modifiées, détruites).
- **Ansible** : Exécution de playbooks complexes, exécution de modules ad-hoc (`shell`, `ping`, `apt`, etc.) et collecte des faits système et matériels (`facts`).
- **Crossplane** : Pilotage de plans de contrôle Kubernetes déclaratifs : déploiement de Claims composites (`XRC`), vérification de la synchronisation et des conditions de disponibilité, inspection des ressources managées (`MR`) et des Providers cloud.
- **WireGuard VPN** : Configuration intégrale d'interfaces sécurisées (`wg-quick`), génération cryptographique de paires de clés (Curve25519), enregistrement/révocation dynamique de pairs (clients nomades, passerelles inter-sites) et monitoring du trafic réseau Rx/Tx.

### 3. Réutilisation Maximale de Dépôts Existants
- **Registre Multi-Sources** : Catalogue indexant nativement [Ansible Galaxy](https://galaxy.ansible.com), [LinuxServer.io](https://fleet.linuxserver.io) et les collections GitHub Sysadmin de référence.
- **Résolution Automatique de Dépendances** : Lorsqu'un service est sélectionné (ex: un conteneur LinuxServer ou un proxy Traefik), Servai installe automatiquement ses prérequis (ex: moteur Docker, configuration du pare-feu UFW) dans le bon ordre.
- **Profils de Serveur Prédéfinis (Presets)** :
  - `secure-web-baseline` : Hardening Linux CIS + UFW/Fail2ban + Caddy auto-HTTPS.
  - `docker-application-host` : Hardening Linux + Pare-feu + Docker Engine + Traefik v3.
  - `observability-node` : Prometheus Node Exporter + Netdata temps réel + Grafana.
  - `homelab-private-cloud` : Hardening + Docker + WireGuard VPN + Nextcloud + Vaultwarden.
- **Multi-Repo Stacks** : Combinaison de dépôts hétérogènes dans un pipeline unifié avec étapes compensatoires de rollback inversé en cas d'erreur.

### 4. Moteur d'Orchestration Complet (Workflows, Flottes & Réconciliation)
- **Pipelines de Workflows Personnalisés** : Définition déclarative de séquences multi-étapes combinant commandes shell distantes, mises à jour DNS temps réel, healthchecks HTTP, actions systemd et installation de paquets.
- **Déploiements en Flotte (Multi-Server Rollouts)** : Ciblage dynamique par tags de serveurs (ex: `web-prod`, `db-cluster`) ou listes explicites d'identifiants avec stratégies d'exécution `Parallel` ou `Rolling` (déploiement progressif avec arrêt en cas d'erreur).
- **Tâches Automatisées Programmées (Cron & Schedules)** : Planification récurrente d'orchestrations avec suivi du cycle de vie et déclenchement manuel immédiat.
- **Réconciliation Déclarative d'État Désiré (Auto-Healing)** : Détection d'écart entre l'état réel du serveur et son état cible (paquets manquants, domaine non lié, interface VPN inactive) avec déclenchement automatique d'un job de remédiation unifié.
- **Traçabilité & Observabilité Granulaire** : Historique complet des jobs avec filtres avancés (`server_id`, `status`) et restitution détaillée des logs d'exécution par étape (durée en millisecondes, stdout, stderr, timestamps).

---

## 🧪 Tests TDD (79/79 Passing - 100% Couverture)

L'intégralité du projet est développée en **Test-Driven Development pur** (Red $\rightarrow$ Green $\rightarrow$ Refactor) :

```bash
cargo test
```

### Découpage des 9 suites de tests :

| Suite de test | Tests | Couverture fonctionnelle |
| :--- | :---: | :--- |
| [`tests/domain_tests.rs`](tests/domain_tests.rs) | **13** | Machine à états du serveur, verrous de concurrence, validation des specs, métriques, rollback automatique, détection de dérive (drift). |
| [`tests/catalog_tests.rs`](tests/catalog_tests.rs) | **2** | Validation des recettes d'administration, métadonnées, compatibilité des distributions Linux. |
| [`tests/inspector_and_stack_tests.rs`](tests/inspector_and_stack_tests.rs) | **3** | Détection automatique des manifestes (Ansible, Compose, Tofu), compilation des étapes multi-dépôts. |
| [`tests/registry_tests.rs`](tests/registry_tests.rs) | **4** | Indexation multi-sources, résolution arborescente de dépendances, composition de profils de serveur. |
| [`tests/engine_tests.rs`](tests/engine_tests.rs) | **8** | Moteurs OpenTofu (plan, apply, destroy, state), Ansible (playbooks, task events, modules, facts), Crossplane (claims, readiness, managed resources) et WireGuard (interfaces, pairs, trafic). |
| [`tests/dns_and_registrar_tests.rs`](tests/dns_and_registrar_tests.rs) | **7** | Enregistrement de domaines, nameservers, zones DNS, types d'enregistrements (A, AAAA, MX, TXT, etc.), TTL, DNSSEC, propagation, certificats TLS et liaison serveur-domaine (`bind_server_domain`). |
| [`tests/complete_orchestration_tests.rs`](tests/complete_orchestration_tests.rs) | **5** | Pipelines de workflows personnalisés, déploiements de flotte (stratégies Parallel / Rolling), automations planifiées (schedules), logs d'exécution détaillés et réconciliation d'état désiré (auto-healing). |
| [`tests/application_tests.rs`](tests/application_tests.rs) | **22** | Cas d'usage applicatifs complets : administration, healthchecks, services systemd, rollback, catalogue, registry, stacks, OpenTofu, Ansible, Crossplane, WireGuard. |
| [`tests/api_bioxus_tests.rs`](tests/api_bioxus_tests.rs) | **15** | Contrats REST complets pour l'UI Bioxus (codes HTTP 200, 201, 204, 400, 404, 409, 422, endpoints serveurs, domaines, DNS, TLS, moteurs, workflows, flottes, schedules et réconciliation). |

---

## 🌐 Endpoints API Bioxus

L'API REST expose tous les services nécessaires à l'interface d'administration **Bioxus** :

### 🖥️ Administration du Serveur & Liaison de Domaine
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/api/v1/health` | Statut et version de Servai |
| `POST` | `/api/v1/servers` | Enregistrer un nouveau serveur physique ou VPS |
| `GET` | `/api/v1/servers` | Lister les serveurs gérés |
| `GET` | `/api/v1/servers/{id}` | Inspecter la fiche détaillée d'un serveur |
| `POST` | `/api/v1/servers/{id}/health` | Ping SSH et auto-détection des caractéristiques système |
| `POST` | `/api/v1/servers/{id}/maintenance` | Activer ou désactiver le mode maintenance |
| `POST` | `/api/v1/servers/{id}/execute` | Exécuter une commande administrative sur le serveur |
| `GET` | `/api/v1/servers/{id}/metrics` | Métriques en temps réel (CPU, RAM, Disque, Uptime) |
| `POST` | `/api/v1/servers/{id}/services/{service}/action` | Contrôle d'un service systemd (`Start`, `Stop`, `Restart`, `Reload`, `Status`) |
| `POST` | `/api/v1/servers/{id}/reboot` | Déclencher le redémarrage ordonné du serveur |
| `POST` | `/api/v1/servers/{id}/bind-domain` | **Lier un nom de domaine au serveur, créer le DNS A et émettre le certificat SSL TLS** |

### 🌐 Registrar de Noms de Domaine
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/api/v1/domains/check?domain={name}` | Vérifier la disponibilité et le prix d'un nom de domaine |
| `POST` | `/api/v1/domains` | Enregistrer un domaine (Cloudflare, OVH, Gandi, Namecheap, Route53, Porkbun) |
| `GET` | `/api/v1/domains` | Lister les noms de domaine gérés |
| `GET` | `/api/v1/domains/{domain}` | Consulter les détails d'enregistrement d'un domaine |
| `PUT` | `/api/v1/domains/{domain}/nameservers` | Mettre à jour les serveurs de noms (Nameservers) |
| `PUT` | `/api/v1/domains/{domain}/auto-renew` | Activer ou désactiver le renouvellement automatique |

### 🧭 Gestion DNS & Propagation
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `POST` | `/api/v1/dns/zones` | Créer une zone DNS avec ou sans DNSSEC |
| `GET` | `/api/v1/dns/zones` | Lister toutes les zones DNS gérées |
| `GET` | `/api/v1/dns/zones/{domain}` | Consulter une zone DNS et tous ses enregistrements |
| `POST` | `/api/v1/dns/zones/{domain}/records` | Créer ou mettre à jour un enregistrement (`A`, `AAAA`, `CNAME`, `MX`, `TXT`, etc.) |
| `DELETE` | `/api/v1/dns/zones/{domain}/records/{id}` | Supprimer un enregistrement DNS |
| `GET` | `/api/v1/dns/zones/{domain}/propagate` | Vérifier la propagation DNS d'un enregistrement en temps réel |

### 🔒 VPN WireGuard
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `POST` | `/api/v1/servers/{id}/wireguard` | Déployer et configurer une interface WireGuard (`wg0`) |
| `GET` | `/api/v1/servers/{id}/wireguard/{iface}/status` | Statut, pairs connectés et statistiques Rx/Tx |
| `GET` | `/api/v1/servers/{id}/wireguard/{iface}/peers` | Lister les pairs VPN autorisés |
| `POST` | `/api/v1/servers/{id}/wireguard/{iface}/peers` | Ajouter dynamiquement un nouveau pair VPN |
| `DELETE` | `/api/v1/servers/{id}/wireguard/{iface}/peers/{pubkey}` | Révoquer un pair VPN |
| `POST` | `/api/v1/wireguard/keypair` | Générer une paire de clés cryptographiques sécurisées |

### ⚙️ Automatisation OpenTofu & Ansible
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `POST` | `/api/v1/tofu/plan` | Calculer le plan d'exécution d'un dépôt OpenTofu |
| `POST` | `/api/v1/tofu/apply` | Appliquer les modifications d'infrastructure OpenTofu |
| `POST` | `/api/v1/tofu/destroy` | Détruire les ressources gérées par OpenTofu |
| `GET` | `/api/v1/tofu/state/{repo_id}` | Récupérer l'état (State/Lineage) OpenTofu au format JSON |
| `POST` | `/api/v1/servers/{id}/ansible/playbook` | Exécuter un playbook Ansible avec variables dynamiques |
| `POST` | `/api/v1/servers/{id}/ansible/module` | Exécuter un module ad-hoc Ansible (`shell`, `apt`, etc.) |
| `GET` | `/api/v1/servers/{id}/ansible/facts` | Collecter les faits matériels et logiciels du serveur |

### ☸️ Plans de Contrôle Crossplane
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `POST` | `/api/v1/crossplane/claims` | Déployer un Claim Crossplane (Composite Resource Claim) |
| `GET` | `/api/v1/crossplane/claims/{namespace}/{name}` | Suivre le statut, les conditions et la synchronisation du Claim |
| `DELETE` | `/api/v1/crossplane/claims/{namespace}/{name}` | Supprimer un Claim et détruire les ressources sous-jacentes |
| `GET` | `/api/v1/crossplane/resources` | Lister les ressources managées Cloud (`Managed Resources`) |
| `GET` | `/api/v1/crossplane/providers` | Inspecter la santé des providers Crossplane (`provider-aws`, etc.) |

### 📦 Registre, Presets, Stacks & Recettes
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/api/v1/registry/search` | Rechercher dans l'écosystème de dépôts (Galaxy, LinuxServer, GitHub) |
| `GET` | `/api/v1/registry/presets` | Lister les profils de serveur complets (Presets multi-dépôts) |
| `POST` | `/api/v1/servers/{id}/packages/{pkg_id}/install` | Installer un paquet avec résolution automatique de dépendances |
| `POST` | `/api/v1/servers/{id}/presets/{preset_id}/apply` | Déployer un profil complet multi-dépôts en un clic |
| `GET` | `/api/v1/catalog` | Explorer les recettes du catalogue standard |
| `POST` | `/api/v1/catalog/{id}/import` | Importer une recette dans Servai |
| `POST` | `/api/v1/servers/{id}/recipes/{recipe_id}/apply` | Déployer une recette standard sur le serveur |
| `POST` | `/api/v1/stacks` | Créer un stack multi-dépôts |
| `GET` | `/api/v1/stacks` | Lister les stacks multi-dépôts |
| `POST` | `/api/v1/servers/{id}/stacks/{stack_id}/apply` | Déployer un stack multi-dépôts sur le serveur |
| `POST` | `/api/v1/repos` | Déclarer un dépôt personnalisé (Git/Ansible/Compose/Tofu) |
| `GET` | `/api/v1/repos` | Lister les dépôts déclarés |
| `POST` | `/api/v1/repos/{id}/sync` | Synchroniser le dépôt Git |

### 🔄 Orchestration Avancée, Workflows, Flottes & Programmation
| Méthode | Route | Description |
| :--- | :--- | :--- |
| `POST` | `/api/v1/orchestrations` | Déclencher un job d'orchestration classique |
| `GET` | `/api/v1/orchestrations` | Lister les jobs avec filtres optionnels (`?server_id=...&status=...`) |
| `GET` | `/api/v1/orchestrations/{id}` | Suivre l'avancement d'un job d'orchestration |
| `GET` | `/api/v1/orchestrations/{id}/logs` | Obtenir les logs détaillés par étape (durée ms, stdout, stderr) |
| `POST` | `/api/v1/orchestrations/{id}/cancel` | Annuler un job en cours avec rollback compensatoire |
| `POST` | `/api/v1/orchestrations/drift` | Détecter les dérives de configuration (Drift) |
| `POST` | `/api/v1/orchestrations/fleet` | Exécuter une commande sur une flotte de serveurs (par tags ou IDs, Parallel / Rolling) |
| `POST` | `/api/v1/orchestrations/reconcile` | Réconcilier déclarativement l'état désiré d'un serveur (auto-healing) |
| `POST` | `/api/v1/workflows` | Créer un pipeline de workflow personnalisé multi-étapes |
| `GET` | `/api/v1/workflows` | Lister les pipelines de workflows enregistrés |
| `GET` | `/api/v1/workflows/{id}` | Consulter la définition d'un pipeline |
| `POST` | `/api/v1/workflows/{id}/execute` | Exécuter un pipeline de workflow sur un serveur cible |
| `POST` | `/api/v1/schedules` | Planifier une tâche d'orchestration récurrente (cron) |
| `GET` | `/api/v1/schedules` | Lister les planifications actives |
| `GET` | `/api/v1/schedules/{id}` | Consulter le statut d'une planification |
| `DELETE` | `/api/v1/schedules/{id}` | Supprimer une planification |
| `POST` | `/api/v1/schedules/{id}/run` | Déclencher immédiatement une tâche planifiée |

---

## 🚀 Démarrage Rapide

### Prérequis
- [Rust 1.80+](https://rustup.rs/)
- `ssh` / `git` installés localement

### Lancement du serveur API
```bash
cargo run
```

Par défaut, Servai écoute sur `http://0.0.0.0:8080` avec les en-têtes CORS permissifs configurés pour les connexions depuis l'UI **Bioxus**.

### Exemples d'appels cURL

**1. Lier un serveur à un domaine avec délivrance automatique du certificat SSL :**
```bash
curl -X POST http://localhost:8080/api/v1/servers/550e8400-e29b-41d4-a716-446655440000/bind-domain \
  -H "Content-Type: application/json" \
  -d '{"domain": "app.bioxus.cloud"}'
```

**2. Enregistrer un nom de domaine via le Registrar :**
```bash
curl -X POST http://localhost:8080/api/v1/domains \
  -H "Content-Type: application/json" \
  -d '{
    "domain": "mon-domaine.org",
    "provider": "Cloudflare",
    "contact_name": "DevOps Lead",
    "contact_email": "ops@bioxus.org",
    "contact_phone": "+33123456789",
    "contact_city": "Paris",
    "contact_country": "France",
    "nameservers": ["ns1.cloudflare.com", "ns2.cloudflare.com"],
    "auto_renew": true
  }'
```

**3. Déployer un profil complet multi-dépôts (Hardening + Docker + WireGuard + Proxy) :**
```bash
curl -X POST http://localhost:8080/api/v1/servers/550e8400-e29b-41d4-a716-446655440000/presets/homelab-private-cloud/apply
```

---

## 📄 Licence
Ce projet est sous licence MIT.
