//! Client for the public [sudhanva.me API](https://sudhanva.me/openapi.json).
//!
//! It retrieves published profile and article metadata, performs bounded batch
//! reads, searches the published site, and creates or polls temporary
//! profile-insight jobs. It follows the stable `/api/v1` contract.
//!
//! The API is public and requires no credentials. Do not send private data.
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
//! Creating a profile-insight job requires a caller-controlled idempotency key.
//! Replaying the same key with the same request within 24 hours returns the
//! same job.
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
//! Every method returns [`Error`]. A non-success HTTP response becomes
//! [`Error::Api`], which carries an [`ApiError`] with the status, error code,
//! message, hint, documentation URL, and the decoded response body. The client
//! decodes both the JSON error envelope and the `application/problem+json`
//! documents that profile-insight requests return.
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
