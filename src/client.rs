use eyre::{Context, Result, bail};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use reqwest::{Client, Method, StatusCode};
use serde_json::Value;
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, instrument, warn};

/// Typed PagerDuty API error. Lets callers (like `try_get`) pattern-match on
/// HTTP status via `eyre::Report::downcast_ref::<ApiError>()`, while still
/// printing the same human-readable message the `bail!` path produced.
#[derive(Debug, Error)]
#[error("{formatted}")]
pub struct ApiError {
    pub status: StatusCode,
    pub body: String,
    pub formatted: String,
}

/// Characters that must be percent-encoded in query parameter values.
/// This is the set from RFC 3986 §3.4 plus `+` (which has special meaning in some parsers).
const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'&')
    .add(b'+')
    .add(b'=');

/// Percent-encode a string for safe use as a URL query parameter value.
pub fn encode_query(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ENCODE_SET).to_string()
}

const BASE_URL: &str = "https://api.pagerduty.com";
const PAGINATION_LIMIT: u32 = 25;
// Some endpoints (e.g. /incident_workflows/triggers, /incident_workflows/actions)
// accept `?limit=N` but reject `?offset=N`. For these we request a single large
// page and warn if the response indicates more results exist. The PagerDuty
// API caps limit at 100 on these endpoints (larger values return 400).
const LARGE_PAGE_LIMIT: u32 = 100;
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

    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    #[instrument(skip(self, body), fields(%method, %path))]
    async fn send(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        self.send_with_from(method, path, body, None).await
    }

    #[instrument(skip(self, body), fields(%method, %path, from = ?from_email))]
    async fn send_with_from(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        from_email: Option<&str>,
    ) -> Result<Value> {
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

            if let Some(email) = from_email {
                req = req.header("From", email);
            }

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
                warn!(delay, attempts, max = MAX_RETRY_ATTEMPTS, "rate limited, retrying");
                sleep(Duration::from_secs(delay)).await;
                continue;
            }

            // 204 No Content (e.g. DELETE /incident_workflows/triggers/{id}) has an empty body.
            if status == StatusCode::NO_CONTENT {
                debug!("204 No Content");
                return Ok(Value::Null);
            }

            if !status.is_success() {
                let error_body = resp.text().await.unwrap_or_default();
                return Err(ApiError {
                    status,
                    formatted: format_api_error(status, &error_body),
                    body: error_body,
                }
                .into());
            }

            let json: Value = resp.json().await.context("Failed to parse response JSON")?;
            debug!(%status, "request succeeded");
            return Ok(json);
        }
    }

    #[instrument(skip(self))]
    pub async fn get(&self, path: &str) -> Result<Value> {
        self.send(Method::GET, path, None).await
    }

    /// GET a resource, returning `Ok(None)` on HTTP 404. All other errors propagate.
    /// Useful for fallback lookups where a missing resource is a legitimate signal.
    #[instrument(skip(self))]
    pub async fn try_get(&self, path: &str) -> Result<Option<Value>> {
        match self.send(Method::GET, path, None).await {
            Ok(v) => Ok(Some(v)),
            Err(e) => match e.downcast_ref::<ApiError>() {
                Some(api_err) if api_err.status == StatusCode::NOT_FOUND => Ok(None),
                _ => Err(e),
            },
        }
    }

    #[instrument(skip(self, body))]
    pub async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.send(Method::POST, path, Some(body)).await
    }

    /// POST with a `From: <email>` header. Required by PagerDuty for mutating
    /// operations like `POST /incidents` and `POST /incidents/{id}/notes`.
    #[instrument(skip(self, body))]
    pub async fn post_with_from(&self, path: &str, body: Value, from_email: &str) -> Result<Value> {
        self.send_with_from(Method::POST, path, Some(body), Some(from_email))
            .await
    }

    /// PUT with a `From: <email>` header. Used by `PUT /incidents/{id}` when
    /// changing status or priority.
    #[instrument(skip(self, body))]
    pub async fn put_with_from(&self, path: &str, body: Value, from_email: &str) -> Result<Value> {
        self.send_with_from(Method::PUT, path, Some(body), Some(from_email))
            .await
    }

    #[instrument(skip(self, body))]
    pub async fn put(&self, path: &str, body: Value) -> Result<Value> {
        self.send(Method::PUT, path, Some(body)).await
    }

    #[instrument(skip(self))]
    pub async fn delete(&self, path: &str) -> Result<Value> {
        self.send(Method::DELETE, path, None).await
    }

    #[instrument(skip(self, body))]
    pub async fn raw(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        let m = Method::from_str(&method.to_uppercase()).map_err(|_| eyre::eyre!("Invalid HTTP method: {}", method))?;
        self.send(m, path, body).await
    }

    /// Fetch a list endpoint that rejects the `offset` query parameter.
    /// Used for `/incident_workflows/triggers` and `/incident_workflows/actions`,
    /// which return 400 on `?offset=0` but accept `?limit=N`. Returns all items
    /// in a single page and warns if the server reports `more=true`.
    #[instrument(skip(self))]
    pub async fn get_all_no_offset(&self, path: &str, key: &str) -> Result<Vec<Value>> {
        let sep = if path.contains('?') { '&' } else { '?' };
        let paginated = format!("{}{}limit={}", path, sep, LARGE_PAGE_LIMIT);
        let resp = self.get(&paginated).await?;

        if resp.get("more").and_then(|v| v.as_bool()).unwrap_or(false) {
            warn!(
                path = %path,
                limit = LARGE_PAGE_LIMIT,
                "endpoint reports more=true but does not support offset pagination; results are truncated"
            );
        }

        Ok(resp.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default())
    }

    /// Paginate through all results for a list endpoint using PagerDuty's
    /// cursor convention: the response carries an `after` field holding the
    /// cursor for the next page, or `null` when the caller has reached the
    /// final page. Request carries the cursor back as `?after=<cursor>`.
    /// Used by newer endpoints like `/alert_grouping_settings` that reject
    /// `?offset=` in favor of `?after=`.
    ///
    /// `key` is the JSON array key in the response body (e.g.
    /// `"alert_grouping_settings"`). `path` may already contain other query
    /// parameters.
    #[instrument(skip(self))]
    pub async fn get_all_cursor(&self, path: &str, key: &str) -> Result<Vec<Value>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        let sep = if path.contains('?') { '&' } else { '?' };

        loop {
            let paginated = match &cursor {
                Some(c) => format!("{}{}limit={}&after={}", path, sep, PAGINATION_LIMIT, encode_query(c)),
                None => format!("{}{}limit={}", path, sep, PAGINATION_LIMIT),
            };
            let resp = self.get(&paginated).await?;

            if let Some(items) = resp.get(key).and_then(|v| v.as_array()) {
                all.extend(items.clone());
            }

            let next = resp
                .get("after")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);

            match next {
                Some(n) => cursor = Some(n),
                None => break,
            }
        }

        debug!(total = all.len(), "cursor pagination complete");
        Ok(all)
    }

    /// Paginate through all results for a list endpoint.
    /// `key` is the JSON array key in the response (e.g., "incident_types").
    /// `path` may contain existing query parameters (e.g., "/incident_workflows?query=foo").
    #[instrument(skip(self))]
    pub async fn get_all(&self, path: &str, key: &str) -> Result<Vec<Value>> {
        let mut all = Vec::new();
        let mut offset = 0u32;
        let sep = if path.contains('?') { '&' } else { '?' };

        loop {
            let paginated = format!("{}{}limit={}&offset={}", path, sep, PAGINATION_LIMIT, offset);
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

        debug!(total = all.len(), "pagination complete");
        Ok(all)
    }
}

/// Format a PagerDuty API error response into a human-readable message.
/// PD returns structured JSON errors: `{"error": {"message": "...", "errors": [...]}}`.
/// For condition/PCL errors, appends a reference to the PCL documentation.
fn format_api_error(status: StatusCode, body: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<Value>(body) {
        let message = parsed
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");

        let details: Vec<&str> = parsed
            .get("error")
            .and_then(|e| e.get("errors"))
            .and_then(|e| e.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let mut msg = format!("API error {}: {}", status, message);
        if !details.is_empty() {
            msg.push_str(&format!("\nDetails: {}", details.join("; ")));
        }

        // Hint for PCL condition errors
        let is_condition_error = body.contains("condition") || body.contains("PCL") || body.contains("pcl");
        if is_condition_error {
            msg.push_str("\nPCL reference: ~/pd/docs/developer/pcl-overview.md");
        }

        return msg;
    }

    format!("API error {}: {}", status, body)
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

    #[test]
    fn test_format_api_error_structured() {
        let body = r#"{"error":{"message":"Invalid Input","code":2001,"errors":["name is required"]}}"#;
        let msg = format_api_error(StatusCode::BAD_REQUEST, body);
        assert!(msg.contains("400"));
        assert!(msg.contains("Invalid Input"));
        assert!(msg.contains("name is required"));
    }

    #[test]
    fn test_format_api_error_pcl_hint() {
        let body = r#"{"error":{"message":"Invalid condition syntax","errors":["PCL parse error"]}}"#;
        let msg = format_api_error(StatusCode::BAD_REQUEST, body);
        assert!(msg.contains("PCL reference"));
    }

    #[test]
    fn test_format_api_error_plain() {
        let msg = format_api_error(StatusCode::NOT_FOUND, "not found");
        assert!(msg.contains("404"));
        assert!(msg.contains("not found"));
    }
}
