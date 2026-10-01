//! Request and response types.
//!
//! In response types, a missing field takes its default value and an unknown
//! field is skipped, so fields added to an API response do not break
//! deserialization.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The response from `GET /profile`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct ProfileResponse {
    /// The published profile.
    pub profile: Profile,
}

/// Sudhanva Narayana's published professional profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[non_exhaustive]
pub struct Profile {
    /// Full name.
    pub name: String,
    /// Current job title.
    pub job_title: String,
    /// Area of specialization.
    pub specialization: String,
    /// Location.
    pub location: String,
    /// Canonical site URL.
    pub url: String,
    /// Public contact email.
    pub email: String,
    /// Current employer.
    pub works_for: Option<Organization>,
    /// Topics of expertise.
    pub knows_about: Vec<String>,
    /// Canonical public profile links.
    pub same_as: Vec<String>,
}

/// An organization, such as an employer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Organization {
    /// Organization name.
    pub name: String,
    /// Organization URL.
    pub url: String,
}

/// Metadata for one published article.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[non_exhaustive]
pub struct Post {
    /// Canonical URL slug.
    pub slug: String,
    /// Title.
    pub title: String,
    /// Summary.
    pub description: String,
    /// Publication time as an RFC 3339 timestamp.
    pub published_at: String,
    /// Last update time as an RFC 3339 timestamp.
    pub updated_at: String,
    /// Lowercase tags.
    pub tags: Vec<String>,
    /// Category, when the post has one.
    pub category: Option<String>,
    /// Canonical article URL.
    pub url: String,
}

/// One page of results from `GET /posts`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct PostsResponse {
    /// Number of posts in this page.
    pub count: usize,
    /// Number of posts matching the filter across all pages.
    pub total: usize,
    /// Cursor for the next page, or `None` on the last page.
    pub next_cursor: Option<String>,
    /// Posts, newest first.
    pub posts: Vec<Post>,
}

/// The response from `GET /posts/{slug}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct PostResponse {
    /// The requested post.
    pub post: Post,
}

/// One read inside a batch request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchOperation {
    /// Caller-chosen identifier, 1 to 64 characters, echoed in the result.
    pub id: String,
    /// HTTP method. The API accepts only `GET`.
    pub method: String,
    /// Relative API path: `/profile`, `/posts` with optional query, or `/posts/{slug}`.
    pub path: String,
}

impl BatchOperation {
    /// Creates a `GET` operation.
    ///
    /// ```
    /// let operation = sudhanva::BatchOperation::get("latest", "/posts?limit=3");
    /// assert_eq!(operation.method, "GET");
    /// ```
    pub fn get(id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            method: "GET".to_owned(),
            path: path.into(),
        }
    }
}

/// The response from `POST /batch`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct BatchResponse {
    /// Number of results.
    pub count: usize,
    /// One result per operation, in request order.
    pub results: Vec<BatchResult>,
}

/// The outcome of one batch operation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct BatchResult {
    /// The operation identifier.
    pub id: String,
    /// The HTTP status the operation would have returned on its own.
    pub status: u16,
    /// The raw JSON body on success.
    pub body: Option<Value>,
    /// The error on failure.
    pub error: Option<ErrorDetail>,
}

impl BatchResult {
    /// Returns true when the operation succeeded.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Decodes the body into a typed response, such as [`PostResponse`].
    ///
    /// Returns `Ok(None)` when the operation has no body.
    pub fn decode<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.body.clone().map(serde_json::from_value).transpose()
    }
}

/// An error object inside a batch result or a failed job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct ErrorDetail {
    /// Machine-readable code.
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
    /// Suggested fix, when present.
    pub hint: Option<String>,
    /// Documentation link, when present.
    pub docs_url: Option<String>,
}

/// The body of `POST /profile-insights`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileInsightRequest {
    /// The intended reader: `recruiter`, `hiring-manager`, `collaborator`,
    /// `researcher`, or `agent`.
    pub audience: String,
    /// Up to five areas to prioritize. The API defaults to `production-ml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<Vec<String>>,
}

impl ProfileInsightRequest {
    /// Creates a request for an audience.
    pub fn new(audience: impl Into<String>) -> Self {
        Self {
            audience: audience.into(),
            focus: None,
        }
    }

    /// Sets the focus areas, such as `production-ml`, `ml-infrastructure`,
    /// `inference`, `kubernetes`, `distributed-systems`, `technical-writing`, or
    /// `career`.
    pub fn focus<I, S>(mut self, focus: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.focus = Some(focus.into_iter().map(Into::into).collect());
        self
    }
}

/// The lifecycle state of a profile-insight job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JobStatus {
    /// Accepted and waiting to run.
    #[default]
    Queued,
    /// Running.
    Running,
    /// Finished with a result.
    Succeeded,
    /// Finished with an error.
    Failed,
    /// A status this version of the SDK does not know.
    #[serde(other)]
    Unknown,
}

impl JobStatus {
    /// Returns true for `succeeded` and `failed`.
    pub fn is_terminal(self) -> bool {
        matches!(self, JobStatus::Succeeded | JobStatus::Failed)
    }
}

/// A short-lived profile-insight job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct ProfileInsightJob {
    /// Job identifier, such as `pi_0123456789abcdef0123456789abcdef`.
    pub job_id: String,
    /// Current state.
    pub status: JobStatus,
    /// Absolute URL for polling the job.
    pub status_url: String,
    /// Creation time as an RFC 3339 timestamp.
    pub created_at: String,
    /// Last update time as an RFC 3339 timestamp.
    pub updated_at: String,
    /// Expiry time as an RFC 3339 timestamp, 24 hours after creation.
    pub expires_at: String,
    /// The result once the job has succeeded.
    pub result: Option<ProfileInsightResult>,
    /// The error once the job has failed.
    pub error: Option<ErrorDetail>,
}

/// Guidance composed from the published profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct ProfileInsightResult {
    /// Title.
    pub title: String,
    /// The audience the guidance was written for.
    pub audience: String,
    /// The focus areas used.
    pub focus: Vec<String>,
    /// One-paragraph summary.
    pub summary: String,
    /// Highlighted skills.
    pub highlights: Vec<String>,
    /// Relevant case studies.
    pub case_studies: Vec<CaseStudy>,
    /// Pages worth reading next.
    pub suggested_pages: Vec<SuggestedPage>,
}

/// A case study in a profile insight.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct CaseStudy {
    /// Title.
    pub title: String,
    /// Summary.
    pub summary: String,
    /// Short measurable outcomes.
    pub proof_points: Vec<String>,
    /// Canonical URL.
    pub url: String,
}

/// A page suggested by a profile insight.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct SuggestedPage {
    /// Page title.
    pub title: String,
    /// Page URL.
    pub url: String,
}

/// An NLWeb 0.55 conversational-search answer from `POST /ask`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct AskResponse {
    /// Protocol metadata.
    #[serde(rename = "_meta")]
    pub meta: AskMeta,
    /// Matching schema.org items.
    pub results: Vec<AskResult>,
}

/// NLWeb response metadata.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct AskMeta {
    /// `answer` or `failure`.
    pub response_type: String,
    /// `conversational_search`.
    pub response_format: Option<String>,
    /// NLWeb protocol version.
    pub version: String,
    /// Request identifier.
    pub request_id: Option<String>,
}

/// One NLWeb result: a schema.org item such as a `Person` or `CreativeWork`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct AskResult {
    /// The schema.org `@type`.
    #[serde(rename = "@type")]
    pub kind: String,
    /// Item name.
    pub name: Option<String>,
    /// Item description.
    pub description: Option<String>,
    /// Item URL.
    pub url: Option<String>,
    /// The page that grounds this result.
    pub grounding: Option<Grounding>,
    /// Every other schema.org property, as raw JSON.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The source of an NLWeb result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct Grounding {
    /// The grounding page URL.
    pub url: Option<String>,
    /// Every other property, as raw JSON.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
