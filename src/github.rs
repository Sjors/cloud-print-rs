use anyhow::{Context, Result, bail};
use md5::Context as Md5Context;
use reqwest::Method;
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::config::{GithubReleaseConfig, OrderFile};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSubmitFiles {
    pub(crate) release_tag: Option<String>,
    pub(crate) files: Vec<OrderFile>,
}

pub(crate) struct GithubClient {
    http: Client,
}

impl GithubClient {
    pub(crate) fn new(http: Client) -> Self {
        Self { http }
    }

    pub(crate) fn resolve_release_files(
        &self,
        config: &GithubReleaseConfig,
        request: ReleaseRequest<'_>,
    ) -> Result<ResolvedSubmitFiles> {
        let release = self.fetch_release(config, request)?;
        let cover = release
            .assets
            .iter()
            .find(|asset| asset.name == config.cover_asset_name)
            .with_context(|| {
                format!(
                    "release {} does not contain asset {}",
                    release.tag_name, config.cover_asset_name
                )
            })?;
        let book = release
            .assets
            .iter()
            .find(|asset| asset.name == config.book_asset_name)
            .with_context(|| {
                format!(
                    "release {} does not contain asset {}",
                    release.tag_name, config.book_asset_name
                )
            })?;

        Ok(ResolvedSubmitFiles {
            release_tag: Some(release.tag_name),
            files: vec![
                OrderFile {
                    file_type: "cover".to_string(),
                    url: cover.browser_download_url.clone(),
                    md5sum: self.md5_for_url(&cover.browser_download_url)?,
                    path: None,
                },
                OrderFile {
                    file_type: "book".to_string(),
                    url: book.browser_download_url.clone(),
                    md5sum: self.md5_for_url(&book.browser_download_url)?,
                    path: None,
                },
            ],
        })
    }

    fn fetch_release(
        &self,
        config: &GithubReleaseConfig,
        request: ReleaseRequest<'_>,
    ) -> Result<GithubRelease> {
        let url = match request {
            ReleaseRequest::Latest => format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                config.owner, config.repo
            ),
            ReleaseRequest::Tag(tag) => format!(
                "https://api.github.com/repos/{}/{}/releases/tags/{}",
                config.owner, config.repo, tag
            ),
        };

        let response = self
            .http
            .request(Method::GET, &url)
            .header("User-Agent", "cloud-print-rs")
            .send()
            .with_context(|| format!("request to {url} failed"))?;
        let status = response.status();
        let body_text = response.text()?;

        if !status.is_success() {
            bail!(
                "GitHub release lookup failed with HTTP {}: {}",
                status,
                body_text
            );
        }

        serde_json::from_str(&body_text).with_context(|| "failed to decode GitHub release response")
    }

    fn md5_for_url(&self, url: &str) -> Result<String> {
        let mut response = self
            .http
            .request(Method::GET, url)
            .header("User-Agent", "cloud-print-rs")
            .send()
            .with_context(|| format!("failed to download {url}"))?;
        let status = response.status();
        if !status.is_success() {
            bail!("downloading {url} failed with HTTP {}", status);
        }

        let mut ctx = Md5Context::new();
        std::io::copy(&mut response, &mut ctx)
            .with_context(|| format!("failed to read {url} for md5"))?;

        Ok(format!("{:x}", ctx.finalize()))
    }
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

pub(crate) enum ReleaseRequest<'a> {
    Latest,
    Tag(&'a str),
}
