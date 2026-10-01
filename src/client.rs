use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue, USER_AGENT as USER_AGENT_HEADER};
use reqwest::{Method, RequestBuilder, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::error::{ApiError, Error, Result};
use crate::types::{
    AskResponse, BatchOperation, BatchResponse, PostResponse, PostsResponse, ProfileInsightJob,
    ProfileInsightRequest, ProfileResponse,
};
use crate::{DEFAULT_BASE_URL, USER_AGENT};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 2 << 20;

/// An async client for every stable public API operation.
///
/// Cloning a `Client` is cheap because clones share one connection pool.
///
/// ```no_run
/// # async fn run() -> Result<(), sudhanva::Error> {
/// let client = sudhanva::Client::new();
/// let profile = client.profile().await?;
/// println!("{} works on {}", profile.profile.name, profile.profile.specialization);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: Url,
    site_url: Url,
    timeout: Duration,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Creates a client for the production API with a 10 second timeout.
    ///
    /// # Panics
    ///
    /// Panics when the TLS backend cannot be initialized, like
    /// [`reqwest::Client::new`]. Use [`Client::builder`] to handle that error.
    pub fn new() -> Self {
        Self::builder()
            .build()
            .expect("the default client configuration is valid")
    }

    /// Starts configuring a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// The API base URL, such as `https://sudhanva.me/api/v1`.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns the published professional profile in English.
    pub async fn profile(&self) -> Result<ProfileResponse> {
        self.profile_with_locale("en").await
    }

    /// Returns the published professional profile in a locale. English (`en`)
    /// is the only published locale.
    pub async fn profile_with_locale(&self, locale: &str) -> Result<ProfileResponse> {
        if locale.is_empty() {
            return Err(Error::invalid("locale is required"));
        }
        let mut url = self.api_url(&["profile"]);
        url.query_pairs_mut().append_pair("locale", locale);
        self.send(self.http.get(url)).await
    }

    /// Returns a page of published articles, newest first.
    ///
    /// Follow [`PostsResponse::next_cursor`] to read later pages:
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), sudhanva::Error> {
    /// use sudhanva::PostsOptions;
    ///
    /// let client = sudhanva::Client::new();
    /// let mut options = PostsOptions::new().limit(50);
    /// loop {
    ///     let page = client.posts(options.clone()).await?;
    ///     for post in &page.posts {
    ///         println!("{}", post.title);
    ///     }
    ///     match page.next_cursor {
    ///         Some(cursor) => options = options.cursor(cursor),
    ///         None => break,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn posts(&self, options: PostsOptions) -> Result<PostsResponse> {
        let limit = options.limit.unwrap_or(20);
        if !(1..=100).contains(&limit) {
            return Err(Error::invalid("limit must be between 1 and 100"));
        }
        let mut url = self.api_url(&["posts"]);
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("limit", &limit.to_string());
            if let Some(tag) = options.tag.as_deref().filter(|tag| !tag.is_empty()) {
                query.append_pair("tag", tag);
            }
            if let Some(cursor) = options
                .cursor
                .as_deref()
                .filter(|cursor| !cursor.is_empty())
            {
                query.append_pair("cursor", cursor);
            }
        }
        self.send(self.http.get(url)).await
    }

    /// Returns metadata for one article by its canonical slug.
    pub async fn post(&self, slug: &str) -> Result<PostResponse> {
        if slug.is_empty() {
            return Err(Error::invalid("slug is required"));
        }
        self.send(self.http.get(self.api_url(&["posts", slug])))
            .await
    }

    /// Runs between 1 and 20 public reads in one request.
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), sudhanva::Error> {
    /// use sudhanva::{BatchOperation, PostsResponse};
    ///
    /// let client = sudhanva::Client::new();
    /// let batch = client
    ///     .batch(&[
    ///         BatchOperation::get("profile", "/profile"),
    ///         BatchOperation::get("latest", "/posts?limit=3"),
    ///     ])
    ///     .await?;
    /// let latest: Option<PostsResponse> = batch.results[1].decode().unwrap();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn batch(&self, operations: &[BatchOperation]) -> Result<BatchResponse> {
        if operations.is_empty() || operations.len() > 20 {
            return Err(Error::invalid(
                "operations must contain between 1 and 20 items",
            ));
        }
        let body = json!({ "operations": operations });
        self.send(self.json(Method::POST, self.api_url(&["batch"]), &body))
            .await
    }

    /// Creates a short-lived profile-insight job.
    ///
    /// The idempotency key is sent as the `Idempotency-Key` header. It must be
    /// 8 to 128 letters, numbers, periods, underscores, colons, or hyphens.
    /// Replaying the same key and request within 24 hours returns the same job;
    /// reusing the key with a different request returns a 422 error.
    pub async fn create_profile_insight(
        &self,
        request: &ProfileInsightRequest,
        idempotency_key: &str,
    ) -> Result<ProfileInsightJob> {
        if idempotency_key.is_empty() {
            return Err(Error::invalid("idempotency key is required"));
        }
        let key = HeaderValue::from_str(idempotency_key)
            .map_err(|_| Error::invalid("idempotency key must be a valid header value"))?;
        let builder = self
            .json(Method::POST, self.api_url(&["profile-insights"]), request)
            .header("Idempotency-Key", key);
        self.send(builder).await
    }

    /// Returns the current state of a profile-insight job.
    pub async fn profile_insight(&self, job_id: &str) -> Result<ProfileInsightJob> {
        if job_id.is_empty() {
            return Err(Error::invalid("job ID is required"));
        }
        self.send(self.http.get(self.api_url(&["profile-insights", job_id])))
            .await
    }

    /// Polls a profile-insight job until it succeeds or fails.
    ///
    /// Returns the job in its final state, including a `failed` job. Returns
    /// [`Error::Timeout`] when the job is still pending after the timeout.
    pub async fn wait_for_profile_insight(
        &self,
        job_id: &str,
        options: WaitOptions,
    ) -> Result<ProfileInsightJob> {
        if options.timeout.is_zero() {
            return Err(Error::invalid("timeout must be positive"));
        }
        let started = Instant::now();
        loop {
            let job = self.profile_insight(job_id).await?;
            if job.status.is_terminal() {
                return Ok(job);
            }
            let elapsed = started.elapsed();
            if elapsed >= options.timeout {
                return Err(Error::Timeout {
                    job_id: job_id.to_owned(),
                    timeout: options.timeout,
                });
            }
            tokio::time::sleep(options.interval.min(options.timeout - elapsed)).await;
        }
    }

    /// Searches the published site with NLWeb 0.55 conversational search.
    ///
    /// The request goes to `/ask` on the site root, not under the API base path.
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), sudhanva::Error> {
    /// use sudhanva::AskOptions;
    ///
    /// let client = sudhanva::Client::new();
    /// let answer = client.ask("Kubernetes inference", AskOptions::new().limit(3)).await?;
    /// for result in &answer.results {
    ///     println!("{} {:?}", result.kind, result.name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ask(&self, text: &str, options: AskOptions) -> Result<AskResponse> {
        if text.is_empty() {
            return Err(Error::invalid("text is required"));
        }
        let limit = options.limit.unwrap_or(10);
        if !(1..=20).contains(&limit) {
            return Err(Error::invalid("limit must be between 1 and 20"));
        }
        let mode = options.mode.as_deref().unwrap_or("list");
        let site = self.site_url.origin().ascii_serialization();
        let body = json!({
            "query": { "text": text, "site": site, "limit": limit },
            "prefer": {
                "streaming": false,
                "response_format": "conversational_search",
                "mode": mode,
            },
            "meta": { "version": "0.55" },
        });
        let mut url = self.site_url.clone();
        url.set_path("/ask");
        self.send(self.json(Method::POST, url, &body)).await
    }

    fn api_url(&self, segments: &[&str]) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("base URL is validated as HTTP(S)")
            .pop_if_empty()
            .extend(segments);
        url
    }

    fn json<T: Serialize + ?Sized>(&self, method: Method, url: Url, body: &T) -> RequestBuilder {
        self.http.request(method, url).json(body)
    }

    async fn send<T: DeserializeOwned>(&self, builder: RequestBuilder) -> Result<T> {
        let mut response = builder
            .header(ACCEPT, "application/json")
            .header(USER_AGENT_HEADER, USER_AGENT)
            .timeout(self.timeout)
            .send()
            .await?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(Error::ResponseTooLarge {
                    limit: MAX_RESPONSE_BYTES,
                });
            }
            body.extend_from_slice(&chunk);
        }

        if !(200..300).contains(&status) {
            return Err(ApiError::from_response(status, content_type, &body).into());
        }
        serde_json::from_slice(&body).map_err(|source| Error::Decode {
            status,
            source,
            body: String::from_utf8_lossy(&body).into_owned(),
        })
    }
}

/// Configures a [`Client`].
///
/// ```
/// use std::time::Duration;
///
/// let client = sudhanva::Client::builder()
///     .base_url("http://localhost:8787/api/v1")
///     .timeout(Duration::from_secs(30))
///     .build()?;
/// assert_eq!(client.base_url().as_str(), "http://localhost:8787/api/v1");
/// # Ok::<(), sudhanva::Error>(())
/// ```
#[derive(Debug, Default)]
#[must_use]
pub struct ClientBuilder {
    base_url: Option<String>,
    http: Option<reqwest::Client>,
    timeout: Option<Duration>,
}

impl ClientBuilder {
    /// Replaces the production API base URL. Useful for tests and proxies.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Uses your own `reqwest::Client`, for example to configure a proxy or
    /// share a connection pool. The SDK still sets `Accept`, `User-Agent`, and
    /// the per-request timeout on every request.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Sets the per-request timeout. The default is 10 seconds.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Builds the client.
    ///
    /// Fails when the base URL is not an absolute HTTP(S) URL, the timeout is
    /// zero, or the default HTTP client cannot be created.
    pub fn build(self) -> Result<Client> {
        let raw = self.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL);
        let mut base_url = Url::parse(raw.trim_end_matches('/'))
            .map_err(|_| Error::invalid("base URL must be an absolute HTTP(S) URL"))?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host_str().is_none() {
            return Err(Error::invalid("base URL must use HTTP or HTTPS"));
        }
        base_url.set_query(None);
        base_url.set_fragment(None);

        let mut site_url = base_url.clone();
        site_url.set_path("/");

        let timeout = self.timeout.unwrap_or(DEFAULT_TIMEOUT);
        if timeout.is_zero() {
            return Err(Error::invalid("timeout must be positive"));
        }

        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder().build()?,
        };

        Ok(Client {
            http,
            base_url,
            site_url,
            timeout,
        })
    }
}

/// Filters and pagination for [`Client::posts`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use]
pub struct PostsOptions {
    limit: Option<u32>,
    tag: Option<String>,
    cursor: Option<String>,
}

impl PostsOptions {
    /// Returns the defaults: 20 posts, no tag, first page.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the page size, from 1 to 100.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Filters by an exact lowercase tag, such as `kubernetes`.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Continues from a previous page's `next_cursor`.
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }
}

/// Options for [`Client::ask`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use]
pub struct AskOptions {
    limit: Option<u32>,
    mode: Option<String>,
}

impl AskOptions {
    /// Returns the defaults: 10 results in `list` mode.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of results, from 1 to 20.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the NLWeb mode: `list`, `summarize`, or `list, summarize`.
    pub fn mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = Some(mode.into());
        self
    }
}

/// Polling options for [`Client::wait_for_profile_insight`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct WaitOptions {
    interval: Duration,
    timeout: Duration,
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
            timeout: Duration::from_secs(30),
        }
    }
}

impl WaitOptions {
    /// Returns the defaults: poll every second for up to 30 seconds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the delay between polls. Zero polls again immediately.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Sets the total time to wait before returning [`Error::Timeout`].
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
