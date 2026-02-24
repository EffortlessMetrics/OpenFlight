// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Async HTTP client for the community cloud profile repository.

use crate::{
    CloudProfile, CloudProfileError, ListFilter, ProfileListing, PublishMeta, Result,
    VoteDirection, VoteResult, DEFAULT_API_BASE_URL, DEFAULT_PAGE_SIZE, DEFAULT_TIMEOUT_SECS,
};
use crate::cache::ProfileCache;
use crate::models::Page;
use flight_profile::Profile;
use serde_json::json;
use std::time::Duration;

/// Configuration for the [`CloudProfileClient`].
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL of the profile repository API.
    ///
    /// Defaults to [`DEFAULT_API_BASE_URL`].
    pub base_url: String,

    /// Request timeout. Defaults to [`DEFAULT_TIMEOUT_SECS`] seconds.
    pub timeout: Duration,

    /// Whether to use the local disk cache for GET requests.
    pub use_cache: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_API_BASE_URL.to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            use_cache: true,
        }
    }
}

/// Async client for the Flight Hub community profile repository.
///
/// All methods are `async` and require a Tokio runtime.
///
/// # Example
///
/// ```no_run
/// use flight_cloud_profiles::{CloudProfileClient, ClientConfig, ListFilter};
///
/// # tokio_test::block_on(async {
/// let client = CloudProfileClient::new(ClientConfig::default())?;
/// let page = client.list_page(ListFilter::default()).await?;
/// println!("{} profiles found", page.total);
/// # Ok::<(), flight_cloud_profiles::CloudProfileError>(())
/// # });
/// ```
pub struct CloudProfileClient {
    config: ClientConfig,
    http: reqwest::Client,
    cache: Option<ProfileCache>,
}

impl CloudProfileClient {
    /// Create a new client from the given configuration.
    ///
    /// Returns an error if the underlying HTTP client cannot be constructed
    /// (e.g. if TLS is unavailable).
    pub fn new(config: ClientConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()?;
        let cache = if config.use_cache {
            ProfileCache::default_dir().ok()
        } else {
            None
        };
        Ok(Self { config, http, cache })
    }

    // ── Read operations ──────────────────────────────────────────────────────

    /// Return the first page of profiles matching the given filter.
    pub async fn list_page(&self, filter: ListFilter) -> Result<Page<ProfileListing>> {
        let mut url = format!("{}/profiles", self.config.base_url);
        let mut params = vec![];
        if let Some(sim) = &filter.sim {
            params.push(format!("sim={}", urlenc(sim)));
        }
        if let Some(icao) = &filter.aircraft_icao {
            params.push(format!("aircraft_icao={}", urlenc(icao)));
        }
        if let Some(q) = &filter.query {
            params.push(format!("q={}", urlenc(q)));
        }
        params.push(format!("sort={}", filter.sort));
        params.push(format!("page={}", filter.page));
        params.push(format!("per_page={}", filter.per_page));
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        let resp = self.http.get(&url).send().await?;
        let page = handle_response::<Page<ProfileListing>>(resp).await?;
        Ok(page)
    }

    /// Return all profiles matching the filter by fetching all pages.
    ///
    /// **Warning**: this may issue many HTTP requests. Prefer
    /// [`list_page`](Self::list_page) for interactive use.
    pub async fn list(&self, filter: ListFilter) -> Result<Vec<ProfileListing>> {
        let mut all = vec![];
        let mut page = 1u32;
        loop {
            let f = ListFilter { page, per_page: DEFAULT_PAGE_SIZE, ..filter.clone() };
            let result = self.list_page(f).await?;
            let has_next = result.has_next_page();
            all.extend(result.items);
            if !has_next {
                break;
            }
            page += 1;
        }
        Ok(all)
    }

    /// Fetch a single profile by its ID.
    ///
    /// If the cache is enabled and the entry is fresh, returns from cache
    /// without a network request.
    pub async fn get(&self, id: &str) -> Result<CloudProfile> {
        if let Some(cache) = &self.cache {
            if let Ok(Some(cached)) = cache.get(id).await {
                tracing::debug!(id, "cloud profile served from cache");
                return Ok(cached);
            }
        }

        let url = format!("{}/profiles/{}", self.config.base_url, urlenc(id));
        let resp = self.http.get(&url).send().await?;
        let profile = handle_response::<CloudProfile>(resp).await?;

        // Store in cache for future calls
        if let Some(cache) = &self.cache {
            if let Err(e) = cache.store(&profile).await {
                tracing::warn!(id, error = %e, "failed to cache cloud profile");
            }
        }

        Ok(profile)
    }

    // ── Write operations ─────────────────────────────────────────────────────

    /// Publish a sanitized local profile to the community repository.
    ///
    /// The caller must have already called [`sanitize_for_upload`] before
    /// passing `profile` here.
    ///
    /// [`sanitize_for_upload`]: crate::sanitize_for_upload
    pub async fn publish(&self, profile: &Profile, meta: PublishMeta) -> Result<CloudProfile> {
        let url = format!("{}/profiles", self.config.base_url);
        let body = json!({
            "title": meta.title,
            "description": meta.description,
            "profile": profile,
        });
        let resp = self.http.post(&url).json(&body).send().await?;
        handle_response::<CloudProfile>(resp).await
    }

    /// Submit a vote on a community profile.
    ///
    /// Voting the same direction twice is idempotent. To remove a vote,
    /// use [`remove_vote`](Self::remove_vote).
    pub async fn vote(&self, id: &str, direction: VoteDirection) -> Result<VoteResult> {
        let url = format!("{}/profiles/{}/vote", self.config.base_url, urlenc(id));
        let body = json!({ "direction": direction });
        let resp = self.http.post(&url).json(&body).send().await?;
        handle_response::<VoteResult>(resp).await
    }

    /// Remove a previously cast vote on a community profile.
    pub async fn remove_vote(&self, id: &str) -> Result<()> {
        let url = format!("{}/profiles/{}/vote", self.config.base_url, urlenc(id));
        let resp = self.http.delete(&url).send().await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status().as_u16();
            let message = resp.text().await.unwrap_or_default();
            Err(CloudProfileError::ApiError { status, message })
        }
    }
}

/// Deserialize a successful response or map an error status to an API error.
async fn handle_response<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T> {
    if resp.status().is_success() {
        Ok(resp.json::<T>().await?)
    } else {
        let status = resp.status().as_u16();
        let message = resp.text().await.unwrap_or_default();
        Err(CloudProfileError::ApiError { status, message })
    }
}

/// Percent-encode a URL path segment (minimal — encodes space and special chars).
fn urlenc(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                vec![c]
            }
            other => {
                let mut buf = [0u8; 4];
                let bytes = other.encode_utf8(&mut buf).as_bytes().to_vec();
                bytes.iter().flat_map(|b| format!("%{b:02X}").chars().collect::<Vec<_>>()).collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlenc_passthrough_safe_chars() {
        assert_eq!(urlenc("C172"), "C172");
        assert_eq!(urlenc("msfs"), "msfs");
        assert_eq!(urlenc("abc-def_gh.ij~"), "abc-def_gh.ij~");
    }

    #[test]
    fn test_urlenc_encodes_spaces() {
        let encoded = urlenc("hello world");
        assert!(encoded.contains("%20"), "space should be percent-encoded");
    }

    #[test]
    fn test_urlenc_encodes_slash() {
        let encoded = urlenc("a/b");
        assert!(encoded.contains("%2F") || encoded.contains("%2f"));
    }

    #[test]
    fn test_client_config_defaults() {
        let cfg = ClientConfig::default();
        assert_eq!(cfg.base_url, DEFAULT_API_BASE_URL);
        assert_eq!(cfg.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        assert!(cfg.use_cache);
    }

    #[test]
    fn test_client_constructs_without_error() {
        let cfg = ClientConfig { use_cache: false, ..ClientConfig::default() };
        let result = CloudProfileClient::new(cfg);
        assert!(result.is_ok());
    }

    // Live API tests — only run when FLIGHT_CLOUD_API_URL env var is set
    #[tokio::test]
    #[ignore = "requires live cloud API (set FLIGHT_CLOUD_API_URL)"]
    async fn test_list_profiles_live() {
        let url = std::env::var("FLIGHT_CLOUD_API_URL").unwrap();
        let cfg = ClientConfig { base_url: url, use_cache: false, ..ClientConfig::default() };
        let client = CloudProfileClient::new(cfg).unwrap();
        let page = client.list_page(ListFilter::default()).await.unwrap();
        println!("Total profiles: {}", page.total);
    }
}
