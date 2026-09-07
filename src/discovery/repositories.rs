use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySnapshot {
    pub name: String,
    pub enabled: bool,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryDiscovery {
    pub repositories: Vec<RepositorySnapshot>,
}

impl RepositoryDiscovery {
    pub fn new() -> Self {
        Self {
            repositories: Vec::new(),
        }
    }
}
