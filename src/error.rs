use std::time::Duration;

use serde_json::Value;

/// A `Result` whose error type is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Every failure this crate can return.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The API returned a non-success status.
    #[error(transparent)]
    Api(Box<ApiError>),

    /// The request could not be sent or the response could not be read.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// A success response did not match the expected JSON shape.
    #[error("could not decode the {status} response: {source}")]
    Decode {
        /// The HTTP status of the response.
        status: u16,
        /// The JSON error.
        #[source]
        source: serde_json::Error,
        /// The response body, decoded as UTF-8 with replacement characters.
        body: String,
    },

    /// The response body exceeded the size this client reads.
    #[error("the response body is larger than {limit} bytes")]
    ResponseTooLarge {
        /// The maximum number of bytes read.
        limit: usize,
    },

    /// An argument failed validation before any request was sent.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// A profile-insight job did not finish before the wait timeout.
    #[error("profile insight {job_id} did not finish within {timeout:?}")]
    Timeout {
        /// The job that was still pending.
        job_id: String,
        /// The configured wait timeout.
        timeout: Duration,
    },

    /// The blocking client could not start its Tokio runtime.
    #[cfg(feature = "blocking")]
    #[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
    #[error("could not start the Tokio runtime: {0}")]
    Runtime(#[source] std::io::Error),
}

impl Error {
    /// Returns the API error when this is [`Error::Api`].
    pub fn api_error(&self) -> Option<&ApiError> {
        match self {
            Error::Api(error) => Some(error),
            _ => None,
        }
    }

    /// Returns the HTTP status when the server sent a response.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api(error) => Some(error.status),
            Error::Decode { status, .. } => Some(*status),
            Error::Http(error) => error.status().map(|status| status.as_u16()),
            _ => None,
        }
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Error::InvalidArgument(message.into())
    }
}

impl From<ApiError> for Error {
    fn from(error: ApiError) -> Self {
        Error::Api(Box::new(error))
    }
}

/// A structured non-success response from the API.
///
/// The API uses two error formats. Most endpoints return a JSON envelope:
///
/// ```json
/// {"error": {"code": "POST_NOT_FOUND", "message": "...", "hint": "...", "docs_url": "..."}}
/// ```
///
/// Profile-insight request errors use RFC 9457 `application/problem+json`:
///
/// ```json
/// {"type": "https://sudhanva.me/docs/profile-insights/#idempotency-key", "title": "...",
///  "status": 400, "detail": "...", "instance": "/api/v1/profile-insights"}
/// ```
///
/// For a problem document, `message` is the `detail` (or `title`), `docs_url`
/// is the `type` URL, and `code` is derived from the `type` URL fragment, so
/// `#idempotency-key` becomes `IDEMPOTENCY_KEY`. When the body carries no code,
/// `code` is `api_error`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{status} {code}: {message}")]
#[non_exhaustive]
pub struct ApiError {
    /// The HTTP status code.
    pub status: u16,
    /// The machine-readable error code, such as `POST_NOT_FOUND`.
    pub code: String,
    /// The human-readable explanation.
    pub message: String,
    /// A suggestion for fixing the request, when the API sends one.
    pub hint: Option<String>,
    /// A link to documentation about the error.
    pub docs_url: Option<String>,
    /// The problem `title`, for `application/problem+json` responses.
    pub title: Option<String>,
    /// The problem `instance`, for `application/problem+json` responses.
    pub instance: Option<String>,
    /// The response `Content-Type` header.
    pub content_type: Option<String>,
    /// The decoded JSON body. When the body is not JSON, this is the raw text as
    /// a JSON string, and it is `Null` when the body is empty.
    pub body: Value,
}

impl ApiError {
    pub(crate) fn from_response(status: u16, content_type: Option<String>, bytes: &[u8]) -> Self {
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into_owned()))
        };

        let text = |value: &Value, key: &str| -> Option<String> {
            value.get(key).and_then(Value::as_str).map(str::to_owned)
        };

        let problem_type = content_type
            .as_deref()
            .is_some_and(|value| value.to_ascii_lowercase().contains("problem+json"))
            || (body.get("error").is_none() && body.get("title").is_some());

        let mut error = ApiError {
            status,
            code: String::new(),
            message: String::new(),
            hint: None,
            docs_url: None,
            title: None,
            instance: None,
            content_type,
            body: Value::Null,
        };

        if problem_type {
            let type_url = text(&body, "type");
            error.title = text(&body, "title");
            error.instance = text(&body, "instance");
            error.message = text(&body, "detail")
                .or_else(|| error.title.clone())
                .unwrap_or_default();
            error.code = text(&body, "code")
                .or_else(|| type_url.as_deref().and_then(code_from_type))
                .unwrap_or_default();
            error.docs_url = type_url.filter(|url| url != "about:blank");
        } else {
            let envelope = body
                .get("error")
                .filter(|value| value.is_object())
                .unwrap_or(&body);
            error.code = text(envelope, "code").unwrap_or_default();
            error.message = text(envelope, "message").unwrap_or_default();
            error.hint = text(envelope, "hint");
            error.docs_url = text(envelope, "docs_url");
        }

        if error.code.is_empty() {
            error.code = "api_error".to_owned();
        }
        if error.message.is_empty() {
            error.message = reqwest::StatusCode::from_u16(status)
                .ok()
                .and_then(|status| status.canonical_reason())
                .unwrap_or("request failed")
                .to_owned();
        }
        error.body = body;
        error
    }
}

/// Turns `https://.../#invalid-profile-insight` into `INVALID_PROFILE_INSIGHT`.
fn code_from_type(type_url: &str) -> Option<String> {
    let (_, fragment) = type_url.split_once('#')?;
    if fragment.is_empty() {
        return None;
    }
    Some(
        fragment
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_json_envelope() {
        let body = br#"{"error":{"code":"POST_NOT_FOUND","message":"Missing","hint":"List posts","docs_url":"https://sudhanva.me/developers/"}}"#;
        let error = ApiError::from_response(404, Some("application/json".into()), body);
        assert_eq!(error.status, 404);
        assert_eq!(error.code, "POST_NOT_FOUND");
        assert_eq!(error.message, "Missing");
        assert_eq!(error.hint.as_deref(), Some("List posts"));
        assert_eq!(
            error.docs_url.as_deref(),
            Some("https://sudhanva.me/developers/")
        );
        assert_eq!(error.body["error"]["code"], "POST_NOT_FOUND");
        assert_eq!(error.to_string(), "404 POST_NOT_FOUND: Missing");
    }

    #[test]
    fn decodes_problem_json() {
        let body = br#"{"type":"https://sudhanva.me/docs/profile-insights/#invalid-profile-insight","title":"Invalid profile insight request","status":422,"detail":"audience must be agent.","instance":"/api/v1/profile-insights"}"#;
        let error = ApiError::from_response(
            422,
            Some("application/problem+json; charset=utf-8".into()),
            body,
        );
        assert_eq!(error.code, "INVALID_PROFILE_INSIGHT");
        assert_eq!(error.message, "audience must be agent.");
        assert_eq!(
            error.title.as_deref(),
            Some("Invalid profile insight request")
        );
        assert_eq!(error.instance.as_deref(), Some("/api/v1/profile-insights"));
        assert_eq!(
            error.docs_url.as_deref(),
            Some("https://sudhanva.me/docs/profile-insights/#invalid-profile-insight")
        );
        assert_eq!(error.hint, None);
    }

    #[test]
    fn falls_back_for_non_json_bodies() {
        let error =
            ApiError::from_response(502, Some("text/html".into()), b"<html>bad gateway</html>");
        assert_eq!(error.code, "api_error");
        assert_eq!(error.message, "Bad Gateway");
        assert_eq!(error.body, Value::String("<html>bad gateway</html>".into()));

        let empty = ApiError::from_response(599, None, b"");
        assert_eq!(empty.message, "request failed");
        assert_eq!(empty.body, Value::Null);
    }

    #[test]
    fn error_stays_small() {
        assert!(std::mem::size_of::<Error>() <= 64);
    }
}
