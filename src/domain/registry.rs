use serde::{Deserialize, Serialize};

use crate::domain::{
    catalog::AdminCategory,
    errors::DomainError,
    server::ServerSpec,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrySource {
    AnsibleGalaxy,
    LinuxServerIo,
    GitHubCommunity,
    CuratedServai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminPackage {
    id: String,
    name: String,
    description: String,
    source: RegistrySource,
    repo_url: String,
    target_ref: String,
    category: AdminCategory,
    dependencies: Vec<String>,
    supported_os: Vec<String>,
    stars: u32,
}

impl AdminPackage {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        source: RegistrySource,
        repo_url: impl Into<String>,
        target_ref: impl Into<String>,
        category: AdminCategory,
        dependencies: Vec<String>,
        supported_os: Vec<String>,
        stars: u32,
    ) -> Result<Self, DomainError> {
        let id_s = id.into();
        let trimmed_id = id_s.trim();
        if trimmed_id.is_empty() {
            return Err(DomainError::ValidationError(
                "Package id cannot be empty".to_string(),
            ));
        }

        let name_s = name.into();
        let trimmed_name = name_s.trim();
        if trimmed_name.is_empty() {
            return Err(DomainError::ValidationError(
                "Package name cannot be empty".to_string(),
            ));
        }

        let url_s = repo_url.into();
        let trimmed_url = url_s.trim();
        if !trimmed_url.starts_with("git@")
            && !trimmed_url.starts_with("https://")
            && !trimmed_url.starts_with("http://")
            && !trimmed_url.starts_with("ssh://")
            && !trimmed_url.starts_with("file://")
        {
            return Err(DomainError::ValidationError(
                "Repository URL must be a valid git/https/ssh/file URL".to_string(),
            ));
        }

        Ok(Self {
            id: trimmed_id.to_string(),
            name: trimmed_name.to_string(),
            description: description.into(),
            source,
            repo_url: trimmed_url.to_string(),
            target_ref: target_ref.into(),
            category,
            dependencies,
            supported_os,
            stars,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn source(&self) -> RegistrySource {
        self.source
    }

    pub fn repo_url(&self) -> &str {
        &self.repo_url
    }

    pub fn target_ref(&self) -> &str {
        &self.target_ref
    }

    pub fn category(&self) -> AdminCategory {
        self.category
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    pub fn supported_os(&self) -> &[String] {
        &self.supported_os
    }

    pub fn stars(&self) -> u32 {
        self.stars
    }

    pub fn is_compatible_with(&self, spec: &ServerSpec) -> bool {
        if self.supported_os.is_empty() {
            return true;
        }

        let spec_os = spec.os.to_lowercase();
        self.supported_os
            .iter()
            .any(|os| spec_os.contains(&os.to_lowercase()))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryQuery {
    pub keyword: Option<String>,
    pub category: Option<AdminCategory>,
    pub source: Option<RegistrySource>,
    pub min_stars: Option<u32>,
}

impl RegistryQuery {
    pub fn builder() -> RegistryQueryBuilder {
        RegistryQueryBuilder::default()
    }

    pub fn matches(&self, pkg: &AdminPackage) -> bool {
        if let Some(cat) = self.category {
            if pkg.category() != cat {
                return false;
            }
        }

        if let Some(src) = self.source {
            if pkg.source() != src {
                return false;
            }
        }

        if let Some(min) = self.min_stars {
            if pkg.stars() < min {
                return false;
            }
        }

        if let Some(ref kw) = self.keyword {
            let kw_lower = kw.to_lowercase();
            let matches_id = pkg.id().to_lowercase().contains(&kw_lower);
            let matches_name = pkg.name().to_lowercase().contains(&kw_lower);
            let matches_desc = pkg.description().to_lowercase().contains(&kw_lower);

            if !matches_id && !matches_name && !matches_desc {
                return false;
            }
        }

        true
    }
}

#[derive(Default)]
pub struct RegistryQueryBuilder {
    query: RegistryQuery,
}

impl RegistryQueryBuilder {
    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.query.keyword = Some(keyword.into());
        self
    }

    pub fn category(mut self, category: AdminCategory) -> Self {
        self.query.category = Some(category);
        self
    }

    pub fn source(mut self, source: RegistrySource) -> Self {
        self.query.source = Some(source);
        self
    }

    pub fn min_stars(mut self, min: u32) -> Self {
        self.query.min_stars = Some(min);
        self
    }

    pub fn build(self) -> RegistryQuery {
        self.query
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerProfilePreset {
    id: String,
    name: String,
    description: String,
    package_ids: Vec<String>,
}

impl ServerProfilePreset {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        package_ids: Vec<String>,
    ) -> Result<Self, DomainError> {
        let id_s = id.into();
        let trimmed_id = id_s.trim();
        if trimmed_id.is_empty() {
            return Err(DomainError::ValidationError(
                "Preset id cannot be empty".to_string(),
            ));
        }

        if package_ids.is_empty() {
            return Err(DomainError::ValidationError(
                "ServerProfilePreset must have at least one package".to_string(),
            ));
        }

        Ok(Self {
            id: trimmed_id.to_string(),
            name: name.into(),
            description: description.into(),
            package_ids,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn package_ids(&self) -> &[String] {
        &self.package_ids
    }
}
