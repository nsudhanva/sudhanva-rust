//! Client for the public [sudhanva.me API](https://sudhanva.me/openapi.json).
//!
//! Use it to get the published profile and article metadata, to read items in
//! batches of a limited size, and to search the published site. You can also
//! use it to create and poll temporary profile-insight jobs. The client uses
//! the stable `/api/v1` contract.
//!
//! The API is public. It does not need credentials. Do not send private data
//! to it.
//!
//! # Example
//!
//! ```no_run
//! use sudhanva::{Client, PostsOptions};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), sudhanva::Error> {
//!     let client = Client::new();
//!
//!     let profile = client.profile().await?;
//!     println!("{}", profile.profile.name);
//!
//!     let posts = client.posts(PostsOptions::new().limit(5).tag("kubernetes")).await?;
//!     for post in &posts.posts {
//!         println!("{} {}", post.slug, post.title);
//!     }
//!
//!     let article = client.post("making-your-site-agent-friendly").await?;
//!     println!("{}", article.post.url);
//!     Ok(())
//! }
//! ```
//!
//! # Profile insights
//!
//! Each profile-insight job must have an idempotency key. The caller controls
//! this key. If you send the same key with the same request again within 24
//! hours, the API returns the same job.
//!
//! ```no_run
//! use sudhanva::{Client, ProfileInsightRequest, WaitOptions};
//!
//! # async fn run() -> Result<(), sudhanva::Error> {
//! let client = Client::new();
//! let request = ProfileInsightRequest::new("hiring-manager").focus(["production-ml", "inference"]);
//! let job = client.create_profile_insight(&request, "my-workflow-2026-08-23").await?;
//! let job = client.wait_for_profile_insight(&job.job_id, WaitOptions::new()).await?;
//! if let Some(result) = job.result {
//!     println!("{}", result.summary);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Errors
//!
//! Each method returns [`Error`]. If an HTTP response is not a success, the
//! client returns [`Error::Api`]. This error contains an [`ApiError`] with the
//! status, error code, message, hint, documentation URL, and the decoded
//! response body. The client decodes the JSON error envelope. It also decodes
//! the `application/problem+json` documents that profile-insight requests
//! return.
//!
//! ```no_run
//! # async fn run() {
//! let client = sudhanva::Client::new();
//! match client.post("missing").await {
//!     Ok(post) => println!("{}", post.post.title),
//!     Err(sudhanva::Error::Api(error)) if error.status == 404 => {
//!         println!("{}: {}", error.code, error.message);
//!     }
//!     Err(error) => eprintln!("{error}"),
//! }
//! # }
//! ```
//!
//! # Features
//!
//! - `blocking`: adds `blocking::Client`, a synchronous client that runs the
//!   async client on its own single-threaded Tokio runtime.
//!
//! TLS uses rustls, so the crate does not link OpenSSL.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub mod blocking;
mod client;
mod error;
mod types;

pub use client::{AskOptions, Client, ClientBuilder, PostsOptions, WaitOptions};
pub use error::{ApiError, Error, Result};
pub use types::*;

/// The `reqwest` version this crate uses, for building a custom
/// [`ClientBuilder::http_client`].
pub use reqwest;

/// The version of this SDK, taken from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `User-Agent` header sent with every request.
pub const USER_AGENT: &str = concat!("sudhanva-rust/", env!("CARGO_PKG_VERSION"));

/// The production API base URL.
pub const DEFAULT_BASE_URL: &str = "https://sudhanva.me/api/v1";
