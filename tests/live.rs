//! Smoke tests against production. They are ignored by default:
//!
//! ```bash
//! cargo test --all-features --test live -- --ignored
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sudhanva::{
    AskOptions, BatchOperation, Client, Error, JobStatus, PostsOptions, ProfileInsightRequest,
    WaitOptions,
};

#[tokio::test]
#[ignore = "calls https://sudhanva.me"]
async fn every_method_works_against_production() {
    let client = Client::new();

    let profile = client.profile().await.unwrap().profile;
    assert_eq!(profile.name, "Sudhanva Narayana");
    assert!(!profile.knows_about.is_empty());

    let first = client.posts(PostsOptions::new().limit(2)).await.unwrap();
    assert_eq!(first.count, first.posts.len());
    assert!(first.total >= first.count);
    let cursor = first
        .next_cursor
        .clone()
        .expect("more than two posts are published");
    let second = client
        .posts(PostsOptions::new().limit(2).cursor(cursor))
        .await
        .unwrap();
    assert_ne!(first.posts[0].slug, second.posts[0].slug);

    let tagged = client
        .posts(PostsOptions::new().limit(5).tag("kubernetes"))
        .await
        .unwrap();
    assert!(
        tagged
            .posts
            .iter()
            .all(|post| post.tags.iter().any(|tag| tag == "kubernetes"))
    );

    let slug = &first.posts[0].slug;
    let post = client.post(slug).await.unwrap().post;
    assert_eq!(&post.slug, slug);

    let batch = client
        .batch(&[
            BatchOperation::get("profile", "/profile"),
            BatchOperation::get("post", format!("/posts/{slug}")),
            BatchOperation::get("missing", "/posts/sdk-smoke-test-missing"),
        ])
        .await
        .unwrap();
    assert_eq!(batch.count, 3);
    assert!(batch.results[0].is_success());
    assert_eq!(batch.results[2].status, 404);

    let answer = client
        .ask("Kubernetes inference", AskOptions::new().limit(3))
        .await
        .unwrap();
    assert_eq!(answer.meta.response_type, "answer");
    assert!(!answer.results.is_empty());

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let key = format!("sudhanva-rust-smoke-{nanos}");
    let request = ProfileInsightRequest::new("agent").focus(["inference"]);
    let job = client.create_profile_insight(&request, &key).await.unwrap();
    assert!(job.job_id.starts_with("pi_"));
    let replay = client.create_profile_insight(&request, &key).await.unwrap();
    assert_eq!(replay.job_id, job.job_id);

    let done = client
        .wait_for_profile_insight(
            &job.job_id,
            WaitOptions::new()
                .interval(Duration::from_millis(500))
                .timeout(Duration::from_secs(30)),
        )
        .await
        .unwrap();
    assert_eq!(done.status, JobStatus::Succeeded);
    let result = done.result.expect("a succeeded job has a result");
    assert_eq!(result.audience, "agent");
    assert!(!result.summary.is_empty());
}

#[tokio::test]
#[ignore = "calls https://sudhanva.me"]
async fn production_errors_decode() {
    let client = Client::new();

    let error = client.post("sdk-smoke-test-missing").await.unwrap_err();
    let api = error.api_error().expect("an API error");
    assert_eq!(api.status, 404);
    assert_eq!(api.code, "POST_NOT_FOUND");
    assert!(api.hint.is_some());
    assert!(api.docs_url.is_some());

    let error = client
        .profile_insight("pi_00000000000000000000000000000000")
        .await
        .unwrap_err();
    assert_eq!(error.api_error().unwrap().code, "PROFILE_INSIGHT_NOT_FOUND");

    let error = client
        .create_profile_insight(
            &ProfileInsightRequest::new("nobody"),
            "sudhanva-rust-invalid",
        )
        .await
        .unwrap_err();
    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.status, 422);
    assert_eq!(api.code, "INVALID_PROFILE_INSIGHT");
    assert!(
        api.content_type
            .unwrap()
            .starts_with("application/problem+json")
    );
}

#[cfg(feature = "blocking")]
#[test]
#[ignore = "calls https://sudhanva.me"]
fn blocking_client_works_against_production() {
    let client = sudhanva::blocking::Client::new();
    assert_eq!(client.profile().unwrap().profile.name, "Sudhanva Narayana");
    assert_eq!(
        client.post("sdk-smoke-test-missing").unwrap_err().status(),
        Some(404)
    );
}
