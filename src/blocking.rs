//! A synchronous client, enabled by the `blocking` feature.
//!
//! [`Client`] wraps the async [`crate::Client`] and drives it on its own
//! single-threaded Tokio runtime. Do not call it from inside an async runtime,
//! because Tokio panics when a runtime starts or drops there. Use the async
//! client in async code.
//!
//! ```no_run
//! use sudhanva::PostsOptions;
//!
//! let client = sudhanva::blocking::Client::new();
//! let posts = client.posts(PostsOptions::new().limit(5))?;
//! println!("{} posts", posts.total);
//! # Ok::<(), sudhanva::Error>(())
//! ```

use std::sync::Arc;

use tokio::runtime::Runtime;

use crate::error::{Error, Result};
use crate::types::{
    AskResponse, BatchOperation, BatchResponse, PostResponse, PostsResponse, ProfileInsightJob,
    ProfileInsightRequest, ProfileResponse,
};
use crate::{AskOptions, PostsOptions, WaitOptions};

/// A blocking client for every stable public API operation.
///
/// Methods match [`crate::Client`] and block until the response arrives.
#[derive(Debug, Clone)]
pub struct Client {
    inner: crate::Client,
    runtime: Arc<Runtime>,
}

impl Client {
    /// Creates a blocking client for the production API.
    ///
    /// # Panics
    ///
    /// Panics when the TLS backend or the Tokio runtime cannot be initialized.
    /// Use [`Client::from_async`] to handle those errors.
    pub fn new() -> Self {
        Self::from_async(crate::Client::new()).expect("the Tokio runtime starts")
    }

    /// Wraps a configured async client.
    ///
    /// ```no_run
    /// let inner = sudhanva::Client::builder()
    ///     .base_url("http://localhost:8787/api/v1")
    ///     .build()?;
    /// let client = sudhanva::blocking::Client::from_async(inner)?;
    /// # Ok::<(), sudhanva::Error>(())
    /// ```
    pub fn from_async(inner: crate::Client) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(Error::Runtime)?;
        Ok(Self {
            inner,
            runtime: Arc::new(runtime),
        })
    }

    /// The wrapped async client.
    pub fn inner(&self) -> &crate::Client {
        &self.inner
    }

    /// See [`crate::Client::profile`].
    pub fn profile(&self) -> Result<ProfileResponse> {
        self.runtime.block_on(self.inner.profile())
    }

    /// See [`crate::Client::profile_with_locale`].
    pub fn profile_with_locale(&self, locale: &str) -> Result<ProfileResponse> {
        self.runtime
            .block_on(self.inner.profile_with_locale(locale))
    }

    /// See [`crate::Client::posts`].
    pub fn posts(&self, options: PostsOptions) -> Result<PostsResponse> {
        self.runtime.block_on(self.inner.posts(options))
    }

    /// See [`crate::Client::post`].
    pub fn post(&self, slug: &str) -> Result<PostResponse> {
        self.runtime.block_on(self.inner.post(slug))
    }

    /// See [`crate::Client::batch`].
    pub fn batch(&self, operations: &[BatchOperation]) -> Result<BatchResponse> {
        self.runtime.block_on(self.inner.batch(operations))
    }

    /// See [`crate::Client::create_profile_insight`].
    pub fn create_profile_insight(
        &self,
        request: &ProfileInsightRequest,
        idempotency_key: &str,
    ) -> Result<ProfileInsightJob> {
        self.runtime
            .block_on(self.inner.create_profile_insight(request, idempotency_key))
    }

    /// See [`crate::Client::profile_insight`].
    pub fn profile_insight(&self, job_id: &str) -> Result<ProfileInsightJob> {
        self.runtime.block_on(self.inner.profile_insight(job_id))
    }

    /// See [`crate::Client::wait_for_profile_insight`].
    pub fn wait_for_profile_insight(
        &self,
        job_id: &str,
        options: WaitOptions,
    ) -> Result<ProfileInsightJob> {
        self.runtime
            .block_on(self.inner.wait_for_profile_insight(job_id, options))
    }

    /// See [`crate::Client::ask`].
    pub fn ask(&self, text: &str, options: AskOptions) -> Result<AskResponse> {
        self.runtime.block_on(self.inner.ask(text, options))
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
