use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BookConfig {
    pub(crate) api_base_url: Option<String>,
    pub(crate) currency: Option<String>,
    pub(crate) github_release: GithubReleaseConfig,
    pub(crate) item: BookItemConfig,
}

impl BookConfig {
    pub(crate) fn resolve_relative_files(mut self, base_dir: &Path) -> Self {
        for file in &mut self.item.files {
            if let Some(path) = &file.path {
                let resolved = if path.is_absolute() {
                    path.clone()
                } else {
                    base_dir.join(path)
                };
                file.path = Some(resolved);
            }
        }
        self
    }

    pub(crate) fn validate_submit_prerequisites(&self) -> Result<()> {
        if self.item.files.is_empty() {
            bail!("submit requires at least one production file URL in the config");
        }
        for file in &self.item.files {
            if file.url.is_empty() {
                bail!("file entry {:?} is missing a URL", file.file_type);
            }
            if file.md5sum.is_empty() {
                bail!("file entry {:?} is missing an md5sum", file.file_type);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GithubReleaseConfig {
    pub(crate) owner: String,
    pub(crate) repo: String,
    pub(crate) cover_asset_name: String,
    pub(crate) book_asset_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BookItemConfig {
    pub(crate) product: String,
    pub(crate) title: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) currency: Option<String>,
    pub(crate) harmonized_code: Option<String>,
    #[serde(default)]
    pub(crate) options: Vec<ItemOption>,
    #[serde(default)]
    pub(crate) files: Vec<OrderFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Address {
    pub(crate) company: Option<String>,
    pub(crate) firstname: String,
    pub(crate) lastname: String,
    pub(crate) street1: String,
    #[serde(default)]
    pub(crate) street2: Option<String>,
    pub(crate) zip: String,
    pub(crate) city: String,
    pub(crate) country: String,
    #[serde(default)]
    pub(crate) state: Option<String>,
    #[serde(default)]
    pub(crate) order_email: Option<String>,
    #[serde(default)]
    pub(crate) delivery_email: Option<String>,
    #[serde(default)]
    pub(crate) phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ItemOption {
    #[serde(rename = "type")]
    pub(crate) option_type: String,
    pub(crate) count: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OrderFile {
    #[serde(rename = "type")]
    pub(crate) file_type: String,
    pub(crate) url: String,
    pub(crate) md5sum: String,
    #[serde(skip_serializing, default)]
    pub(crate) path: Option<PathBuf>,
}

pub(crate) fn load_toml<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)?;
    Ok(toml::from_str(&raw)?)
}

pub(crate) fn absolutize(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}
