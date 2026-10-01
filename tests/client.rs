//! Offline tests against a local wiremock server. They never call production.

use std::time::Duration;

use serde_json::{Value, json};
use sudhanva::{
    AskOptions, BatchOperation, Client, Error, JobStatus, PostResponse, PostsOptions,
    ProfileInsightRequest, USER_AGENT, WaitOptions,
};
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

async fn client(server: &MockServer) -> Client {
    Client::builder()
        .base_url(format!("{}/api/v1", server.uri()))
        .build()
        .unwrap()
}

fn json_response(status: u16, body: Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

fn post_json(slug: &str) -> Value {
    json!({
        "slug": slug,
        "title": "Title",
        "description": "Description",
        "publishedAt": "2026-09-27T00:00:00.000Z",
        "updatedAt": "2026-09-28T00:00:00.000Z",
        "tags": ["kubernetes"],
        "category": null,
        "url": format!("https://sudhanva.me/blog/posts/{slug}/"),
        "futureField": true
    })
}

fn job_json(status: &str) -> Value {
    json!({
        "job_id": "pi_0123456789abcdef0123456789abcdef",
        "status": status,
        "status_url": "https://sudhanva.me/api/v1/profile-insights/pi_0123456789abcdef0123456789abcdef",
        "created_at": "2026-10-01T00:00:00.000Z",
        "updated_at": "2026-10-01T00:00:00.000Z",
        "expires_at": "2026-10-02T00:00:00.000Z"
    })
}

#[tokio::test]
async fn profile_sends_locale_and_sdk_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/profile"))
        .and(query_param("locale", "en"))
        .and(header("accept", "application/json"))
        .and(header("user-agent", USER_AGENT))
        .respond_with(json_response(
            200,
            json!({"profile": {
                "name": "Sudhanva Narayana",
                "jobTitle": "Senior Machine Learning Engineer",
                "worksFor": {"name": "Montai Therapeutics", "url": "https://montai.com"},
                "knowsAbout": ["Kubernetes"],
                "sameAs": ["https://github.com/nsudhanva"]
            }}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let profile = client(&server).await.profile().await.unwrap().profile;
    assert_eq!(profile.name, "Sudhanva Narayana");
    assert_eq!(profile.job_title, "Senior Machine Learning Engineer");
    assert_eq!(profile.works_for.unwrap().name, "Montai Therapeutics");
    assert_eq!(profile.knows_about, ["Kubernetes"]);
    // Missing fields take their defaults.
    assert_eq!(profile.location, "");
}

#[tokio::test]
async fn profile_with_locale_passes_the_locale() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/profile"))
        .and(query_param("locale", "fr"))
        .respond_with(json_response(200, json!({"profile": {"name": "N"}})))
        .expect(1)
        .mount(&server)
        .await;
    client(&server)
        .await
        .profile_with_locale("fr")
        .await
        .unwrap();
}

#[tokio::test]
async fn posts_encode_filters_and_follow_cursors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/posts"))
        .and(query_param("limit", "1"))
        .and(query_param("tag", "machine-learning"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json_response(
            200,
            json!({"count": 1, "total": 2, "next_cursor": "first", "posts": [post_json("first")]}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/posts"))
        .and(query_param("cursor", "first"))
        .respond_with(json_response(
            200,
            json!({"count": 1, "total": 2, "next_cursor": null, "posts": [post_json("second")]}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server).await;
    let mut options = PostsOptions::new().limit(1).tag("machine-learning");
    let mut slugs = Vec::new();
    loop {
        let page = client.posts(options.clone()).await.unwrap();
        assert_eq!(page.total, 2);
        slugs.extend(page.posts.into_iter().map(|post| post.slug));
        match page.next_cursor {
            Some(cursor) => options = options.cursor(cursor),
            None => break,
        }
    }
    assert_eq!(slugs, ["first", "second"]);
}

#[tokio::test]
async fn posts_default_to_twenty() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/posts"))
        .and(query_param("limit", "20"))
        .and(query_param_is_missing("tag"))
        .respond_with(json_response(
            200,
            json!({"count": 0, "total": 0, "next_cursor": null, "posts": []}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let page = client(&server)
        .await
        .posts(PostsOptions::new())
        .await
        .unwrap();
    assert!(page.posts.is_empty());
    assert_eq!(page.next_cursor, None);
}

#[tokio::test]
async fn post_escapes_the_slug_and_decodes_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/posts/making-your-site-agent-friendly"))
        .respond_with(json_response(
            200,
            json!({"post": post_json("making-your-site-agent-friendly")}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/posts/a%2Fb"))
        .respond_with(json_response(200, json!({"post": post_json("a/b")})))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server).await;
    let post = client
        .post("making-your-site-agent-friendly")
        .await
        .unwrap()
        .post;
    assert_eq!(post.published_at, "2026-09-27T00:00:00.000Z");
    assert_eq!(post.category, None);
    assert_eq!(post.tags, ["kubernetes"]);
    assert_eq!(client.post("a/b").await.unwrap().post.slug, "a/b");
}

#[tokio::test]
async fn batch_sends_operations_and_preserves_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/batch"))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({"operations": [
            {"id": "post", "method": "GET", "path": "/posts/hello"},
            {"id": "missing", "method": "GET", "path": "/posts/missing"}
        ]})))
        .respond_with(json_response(
            200,
            json!({"count": 2, "results": [
                {"id": "post", "status": 200, "body": {"post": post_json("hello")}},
                {"id": "missing", "status": 404, "error": {"code": "POST_NOT_FOUND", "message": "Missing"}}
            ]}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let batch = client(&server)
        .await
        .batch(&[
            BatchOperation::get("post", "/posts/hello"),
            BatchOperation::get("missing", "/posts/missing"),
        ])
        .await
        .unwrap();
    assert_eq!(batch.count, 2);
    assert!(batch.results[0].is_success());
    let post: PostResponse = batch.results[0].decode().unwrap().unwrap();
    assert_eq!(post.post.slug, "hello");
    assert!(!batch.results[1].is_success());
    assert_eq!(
        batch.results[1].error.as_ref().unwrap().code,
        "POST_NOT_FOUND"
    );
    assert_eq!(batch.results[1].decode::<PostResponse>().unwrap(), None);
}

#[tokio::test]
async fn profile_insight_sends_idempotency_key_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/profile-insights"))
        .and(header("idempotency-key", "rust-test-123"))
        .and(header("user-agent", USER_AGENT))
        .and(header("content-type", "application/json"))
        .and(body_json(
            json!({"audience": "agent", "focus": ["production-ml"]}),
        ))
        .respond_with(json_response(202, job_json("queued")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/profile-insights"))
        .and(header("idempotency-key", "rust-test-456"))
        .and(body_json(json!({"audience": "recruiter"})))
        .respond_with(json_response(202, job_json("queued")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server).await;
    let request = ProfileInsightRequest::new("agent").focus(["production-ml"]);
    let job = client
        .create_profile_insight(&request, "rust-test-123")
        .await
        .unwrap();
    assert_eq!(job.status, JobStatus::Queued);
    assert_eq!(job.job_id, "pi_0123456789abcdef0123456789abcdef");

    // Without focus, the field is omitted so the API applies its default.
    client
        .create_profile_insight(&ProfileInsightRequest::new("recruiter"), "rust-test-456")
        .await
        .unwrap();
}

#[tokio::test]
async fn profile_insight_decodes_the_result() {
    let server = MockServer::start().await;
    let mut body = job_json("succeeded");
    body["result"] = json!({
        "title": "Insight",
        "audience": "agent",
        "focus": ["inference"],
        "summary": "Summary",
        "highlights": ["ML Inference"],
        "case_studies": [{"title": "Case", "summary": "S", "proof_points": ["10B+ rows"], "url": "https://sudhanva.me/work/"}],
        "suggested_pages": [{"title": "About", "url": "https://sudhanva.me/about/"}]
    });
    Mock::given(method("GET"))
        .and(path(
            "/api/v1/profile-insights/pi_0123456789abcdef0123456789abcdef",
        ))
        .respond_with(json_response(200, body))
        .expect(1)
        .mount(&server)
        .await;

    let job = client(&server)
        .await
        .profile_insight("pi_0123456789abcdef0123456789abcdef")
        .await
        .unwrap();
    assert!(job.status.is_terminal());
    let result = job.result.unwrap();
    assert_eq!(result.case_studies[0].proof_points, ["10B+ rows"]);
    assert_eq!(result.suggested_pages[0].title, "About");
}

/// Answers `running` for the first `pending` polls, then `succeeded`.
struct Progress {
    pending: usize,
    calls: std::sync::atomic::AtomicUsize,
}

impl Respond for Progress {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let status = if call < self.pending {
            "running"
        } else {
            "succeeded"
        };
        json_response(200, job_json(status))
    }
}

#[tokio::test]
async fn wait_polls_until_a_terminal_state() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/profile-insights/pi_wait"))
        .respond_with(Progress {
            pending: 2,
            calls: 0.into(),
        })
        .expect(3)
        .mount(&server)
        .await;

    let job = client(&server)
        .await
        .wait_for_profile_insight(
            "pi_wait",
            WaitOptions::new().interval(Duration::from_millis(5)),
        )
        .await
        .unwrap();
    assert_eq!(job.status, JobStatus::Succeeded);
}

#[tokio::test]
async fn wait_returns_failed_jobs() {
    let server = MockServer::start().await;
    let mut body = job_json("failed");
    body["error"] = json!({"code": "INSIGHT_FAILED", "message": "Failed", "hint": "Retry", "docs_url": "https://sudhanva.me/docs/"});
    Mock::given(path("/api/v1/profile-insights/pi_failed"))
        .respond_with(json_response(200, body))
        .mount(&server)
        .await;

    let job = client(&server)
        .await
        .wait_for_profile_insight("pi_failed", WaitOptions::new())
        .await
        .unwrap();
    assert_eq!(job.status, JobStatus::Failed);
    assert_eq!(job.error.unwrap().code, "INSIGHT_FAILED");
}

#[tokio::test]
async fn wait_times_out() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/profile-insights/pi_slow"))
        .respond_with(json_response(200, job_json("running")))
        .mount(&server)
        .await;

    let error = client(&server)
        .await
        .wait_for_profile_insight(
            "pi_slow",
            WaitOptions::new()
                .interval(Duration::from_millis(10))
                .timeout(Duration::from_millis(50)),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Timeout { ref job_id, .. } if job_id == "pi_slow"));
}

#[tokio::test]
async fn unknown_job_statuses_do_not_break_decoding() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/profile-insights/pi_new"))
        .respond_with(json_response(200, job_json("paused")))
        .mount(&server)
        .await;
    let job = client(&server)
        .await
        .profile_insight("pi_new")
        .await
        .unwrap();
    assert_eq!(job.status, JobStatus::Unknown);
}

#[tokio::test]
async fn ask_posts_nlweb_to_the_site_root() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ask"))
        .and(header("accept", "application/json"))
        .and(body_json(json!({
            "query": {"text": "Kubernetes", "site": server.uri(), "limit": 3},
            "prefer": {"streaming": false, "response_format": "conversational_search", "mode": "summarize"},
            "meta": {"version": "0.55"}
        })))
        .respond_with(json_response(
            200,
            json!({
                "_meta": {"response_type": "answer", "response_format": "conversational_search", "version": "0.55", "request_id": "id"},
                "results": [{
                    "@type": "CreativeWork",
                    "name": "Case",
                    "url": "https://sudhanva.me/work/",
                    "keywords": ["Kubernetes"],
                    "grounding": {"url": "https://sudhanva.me/work/"}
                }]
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let answer = client(&server)
        .await
        .ask("Kubernetes", AskOptions::new().limit(3).mode("summarize"))
        .await
        .unwrap();
    assert_eq!(answer.meta.response_type, "answer");
    let result = &answer.results[0];
    assert_eq!(result.kind, "CreativeWork");
    assert_eq!(result.name.as_deref(), Some("Case"));
    assert_eq!(result.extra["keywords"], json!(["Kubernetes"]));
    assert_eq!(
        result.grounding.as_ref().unwrap().url.as_deref(),
        Some("https://sudhanva.me/work/")
    );
}

#[tokio::test]
async fn ask_defaults_to_ten_results_in_list_mode() {
    let server = MockServer::start().await;
    Mock::given(path("/ask"))
        .and(body_json(json!({
            "query": {"text": "inference", "site": server.uri(), "limit": 10},
            "prefer": {"streaming": false, "response_format": "conversational_search", "mode": "list"},
            "meta": {"version": "0.55"}
        })))
        .respond_with(json_response(200, json!({"_meta": {"response_type": "answer", "version": "0.55"}, "results": []})))
        .expect(1)
        .mount(&server)
        .await;
    client(&server)
        .await
        .ask("inference", AskOptions::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn json_error_envelopes_become_api_errors() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/posts/missing"))
        .respond_with(json_response(
            404,
            json!({"error": {
                "code": "POST_NOT_FOUND",
                "message": "No published post exists with slug \"missing\".",
                "hint": "List published posts with GET /api/v1/posts.",
                "docs_url": "https://sudhanva.me/developers/"
            }}),
        ))
        .mount(&server)
        .await;

    let error = client(&server).await.post("missing").await.unwrap_err();
    assert_eq!(error.status(), Some(404));
    let api = error.api_error().unwrap();
    assert_eq!(api.code, "POST_NOT_FOUND");
    assert_eq!(
        api.hint.as_deref(),
        Some("List published posts with GET /api/v1/posts.")
    );
    assert_eq!(
        api.docs_url.as_deref(),
        Some("https://sudhanva.me/developers/")
    );
    assert_eq!(api.body["error"]["code"], "POST_NOT_FOUND");
    assert!(error.to_string().starts_with("404 POST_NOT_FOUND: "));
}

#[tokio::test]
async fn problem_json_errors_become_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/profile-insights"))
        .respond_with(
            ResponseTemplate::new(422).set_body_raw(
                json!({
                    "type": "https://sudhanva.me/docs/profile-insights/#invalid-profile-insight",
                    "title": "Invalid profile insight request",
                    "status": 422,
                    "detail": "audience must be recruiter, hiring-manager, collaborator, researcher, or agent.",
                    "instance": "/api/v1/profile-insights"
                })
                .to_string(),
                "application/problem+json; charset=utf-8",
            ),
        )
        .mount(&server)
        .await;

    let error = client(&server)
        .await
        .create_profile_insight(&ProfileInsightRequest::new("nobody"), "rust-test-789")
        .await
        .unwrap_err();
    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.status, 422);
    assert_eq!(api.code, "INVALID_PROFILE_INSIGHT");
    assert!(api.message.starts_with("audience must be"));
    assert_eq!(
        api.title.as_deref(),
        Some("Invalid profile insight request")
    );
    assert_eq!(api.instance.as_deref(), Some("/api/v1/profile-insights"));
    assert!(
        api.content_type
            .as_deref()
            .unwrap()
            .starts_with("application/problem+json")
    );
}

#[tokio::test]
async fn nlweb_failures_become_api_errors() {
    let server = MockServer::start().await;
    Mock::given(path("/ask"))
        .respond_with(json_response(
            400,
            json!({"_meta": {"response_type": "failure", "version": "0.55"}, "error": {"code": "UNSUPPORTED_MODE", "message": "Bad mode"}}),
        ))
        .mount(&server)
        .await;
    let error = client(&server)
        .await
        .ask("x", AskOptions::new().mode("poem"))
        .await
        .unwrap_err();
    assert_eq!(error.api_error().unwrap().code, "UNSUPPORTED_MODE");
}

#[tokio::test]
async fn invalid_success_bodies_are_decode_errors() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/profile"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;
    let error = client(&server).await.profile().await.unwrap_err();
    assert!(matches!(error, Error::Decode { status: 200, ref body, .. } if body == "not json"));
}

#[tokio::test]
async fn arguments_are_validated_before_sending() {
    // No server: every call must fail before a request is made.
    let client = Client::builder()
        .base_url("http://127.0.0.1:9/api/v1")
        .build()
        .unwrap();
    let invalid = |result: sudhanva::Result<()>| matches!(result, Err(Error::InvalidArgument(_)));

    assert!(invalid(
        client.posts(PostsOptions::new().limit(0)).await.map(drop)
    ));
    assert!(invalid(
        client.posts(PostsOptions::new().limit(101)).await.map(drop)
    ));
    assert!(invalid(client.post("").await.map(drop)));
    assert!(invalid(client.batch(&[]).await.map(drop)));
    let many: Vec<_> = (0..21)
        .map(|i| BatchOperation::get(i.to_string(), "/profile"))
        .collect();
    assert!(invalid(client.batch(&many).await.map(drop)));
    let request = ProfileInsightRequest::new("agent");
    assert!(invalid(
        client.create_profile_insight(&request, "").await.map(drop)
    ));
    assert!(invalid(
        client
            .create_profile_insight(&request, "bad\nkey")
            .await
            .map(drop)
    ));
    assert!(invalid(client.profile_insight("").await.map(drop)));
    assert!(invalid(
        client
            .wait_for_profile_insight("pi_x", WaitOptions::new().timeout(Duration::ZERO))
            .await
            .map(drop)
    ));
    assert!(invalid(client.ask("", AskOptions::new()).await.map(drop)));
    assert!(invalid(
        client.ask("x", AskOptions::new().limit(21)).await.map(drop)
    ));
}

#[test]
fn builder_validates_configuration() {
    assert!(Client::builder().base_url("not a url").build().is_err());
    assert!(
        Client::builder()
            .base_url("ftp://example.com/api")
            .build()
            .is_err()
    );
    assert!(Client::builder().timeout(Duration::ZERO).build().is_err());

    let client = Client::builder()
        .base_url("https://example.test/api/v1/")
        .http_client(reqwest::Client::new())
        .build()
        .unwrap();
    assert_eq!(client.base_url().as_str(), "https://example.test/api/v1");
    assert_eq!(
        Client::new().base_url().as_str(),
        sudhanva::DEFAULT_BASE_URL
    );
}

#[tokio::test]
async fn custom_http_clients_still_identify_the_sdk() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/profile"))
        .and(header("user-agent", USER_AGENT))
        .respond_with(json_response(200, json!({"profile": {}})))
        .expect(1)
        .mount(&server)
        .await;
    let http = reqwest::Client::builder()
        .user_agent("custom/1.0")
        .build()
        .unwrap();
    let client = Client::builder()
        .base_url(format!("{}/api/v1", server.uri()))
        .http_client(http)
        .build()
        .unwrap();
    client.profile().await.unwrap();
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_client_matches_the_async_client() {
    // The mock server runs on its own runtime; the blocking client must be
    // called from a thread outside any runtime.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(async {
        let server = MockServer::start().await;
        Mock::given(path("/api/v1/posts/hello"))
            .and(header("user-agent", USER_AGENT))
            .respond_with(json_response(200, json!({"post": post_json("hello")})))
            .mount(&server)
            .await;
        Mock::given(path("/api/v1/profile-insights/pi_wait"))
            .respond_with(Progress {
                pending: 1,
                calls: 0.into(),
            })
            .mount(&server)
            .await;
        server
    });

    let inner = Client::builder()
        .base_url(format!("{}/api/v1", server.uri()))
        .build()
        .unwrap();
    let client = sudhanva::blocking::Client::from_async(inner).unwrap();
    assert_eq!(client.post("hello").unwrap().post.slug, "hello");
    let job = client
        .wait_for_profile_insight("pi_wait", WaitOptions::new().interval(Duration::ZERO))
        .unwrap();
    assert_eq!(job.status, JobStatus::Succeeded);
    assert_eq!(client.post("nope").unwrap_err().status(), Some(404));
}
