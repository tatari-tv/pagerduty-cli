use eyre::{Context, Result, bail};
use log::{debug, warn};
use reqwest::{Client, Method, StatusCode};
use serde_json::Value;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;

const BASE_URL: &str = "https://api.pagerduty.com";
const PAGINATION_LIMIT: u32 = 25;
const MAX_RETRY_ATTEMPTS: u32 = 3;
const DEFAULT_RETRY_DELAY_SECS: u64 = 5;
const REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct PdClient {
    http: Client,
    base_url: String,
    token: String,
}

impl PdClient {
    pub fn new(token: String) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            base_url: BASE_URL.to_string(),
            token,
        })
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    async fn send(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        debug!("send: method={} path={}", method, path);

        let url = format!("{}{}", self.base_url, path);
        let mut attempts = 0u32;

        loop {
            attempts += 1;

            let mut req = self
                .http
                .request(method.clone(), &url)
                .header("Authorization", format!("Token token={}", self.token))
                .header("Content-Type", "application/json")
                .header("Accept", "application/vnd.pagerduty+json;version=2");

            if let Some(ref b) = body {
                req = req.json(b);
            }

            let resp = req.send().await.context("HTTP request failed")?;
            let status = resp.status();

            if status == StatusCode::TOO_MANY_REQUESTS {
                if attempts > MAX_RETRY_ATTEMPTS {
                    bail!("Rate limited after {} attempts", MAX_RETRY_ATTEMPTS);
                }
                let delay = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(DEFAULT_RETRY_DELAY_SECS);
                warn!(
                    "Rate limited, retrying after {}s (attempt {}/{})",
                    delay, attempts, MAX_RETRY_ATTEMPTS
                );
                sleep(Duration::from_secs(delay)).await;
                continue;
            }

            if !status.is_success() {
                let error_body = resp.text().await.unwrap_or_default();
                // PagerDuty returns structured errors; include body for context
                bail!("API error {}: {}", status, error_body);
            }

            let json: Value = resp.json().await.context("Failed to parse response JSON")?;
            debug!("send: success status={}", status);
            return Ok(json);
        }
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.send(Method::GET, path, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.send(Method::POST, path, Some(body)).await
    }

    pub async fn put(&self, path: &str, body: Value) -> Result<Value> {
        self.send(Method::PUT, path, Some(body)).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value> {
        self.send(Method::DELETE, path, None).await
    }

    pub async fn raw(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        debug!("raw: method={} path={}", method, path);
        let m = Method::from_str(&method.to_uppercase()).map_err(|_| eyre::eyre!("Invalid HTTP method: {}", method))?;
        self.send(m, path, body).await
    }

    /// Paginate through all results for a list endpoint.
    /// `key` is the JSON array key in the response (e.g., "incident_types").
    pub async fn get_all(&self, path: &str, key: &str) -> Result<Vec<Value>> {
        debug!("get_all: path={} key={}", path, key);
        let mut all = Vec::new();
        let mut offset = 0u32;

        loop {
            let paginated = format!("{}?limit={}&offset={}", path, PAGINATION_LIMIT, offset);
            let resp = self.get(&paginated).await?;

            if let Some(items) = resp.get(key).and_then(|v| v.as_array()) {
                all.extend(items.clone());
            }

            let more = resp.get("more").and_then(|v| v.as_bool()).unwrap_or(false);

            if !more {
                break;
            }

            offset += PAGINATION_LIMIT;
        }

        debug!("get_all: fetched {} total items", all.len());
        Ok(all)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_succeeds() {
        let client = PdClient::new("test-token".to_string());
        assert!(client.is_ok());
    }

    #[test]
    fn test_with_base_url() {
        let client = PdClient::new("test-token".to_string())
            .unwrap()
            .with_base_url("https://custom.example.com".to_string());
        assert_eq!(client.base_url, "https://custom.example.com");
    }
}
