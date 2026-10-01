# sudhanva for Rust

Minimal async Rust client for the public [sudhanva.me API](https://sudhanva.me/openapi.json). It
retrieves published profile and article metadata, performs bounded batch reads, searches the
published site, and creates or polls temporary profile-insight jobs.

The API is public and requires no credentials. Do not send private data.

## Install

Install it from [crates.io](https://crates.io/crates/sudhanva):

```bash
cargo add sudhanva@0.1.0
```

Or add it to `Cargo.toml`:

```toml
[dependencies]
sudhanva = "0.1.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The client is async and runs on Tokio. Enable the `blocking` feature for a synchronous client.

## Use

```rust
use sudhanva::{Client, PostsOptions, ProfileInsightRequest, WaitOptions};

#[tokio::main]
async fn main() -> Result<(), sudhanva::Error> {
    let client = Client::new();

    let profile = client.profile().await?;
    let posts = client.posts(PostsOptions::new().limit(5).tag("kubernetes")).await?;
    let article = client.post("making-your-site-agent-friendly").await?;
    println!("{} has {} posts", profile.profile.name, posts.total);
    println!("{}", article.post.title);

    let request = ProfileInsightRequest::new("hiring-manager").focus(["production-ml", "inference"]);
    let job = client.create_profile_insight(&request, "my-workflow-2026-08-23").await?;
    let job = client.wait_for_profile_insight(&job.job_id, WaitOptions::new()).await?;
    if let Some(result) = job.result {
        println!("{}", result.summary);
    }
    Ok(())
}
```

Profile-insight requests require a caller-controlled idempotency key, sent as the
`Idempotency-Key` header. Replaying the same key and request within 24 hours returns the same job.

Methods return typed structs. A missing field takes its default value and the client skips unknown
fields, so a new field in an API response does not break decoding. Batch result bodies and extra
NLWeb result properties stay available as raw `serde_json::Value`.

Non-success responses return `sudhanva::Error::Api` with an `ApiError` holding `status`, `code`,
`message`, `hint`, `docs_url`, and the decoded response `body`. The client decodes both the JSON
error envelope and the `application/problem+json` documents that profile-insight requests return:

```rust
match client.post("missing").await {
    Ok(post) => println!("{}", post.post.title),
    Err(sudhanva::Error::Api(error)) if error.status == 404 => println!("{}", error.code),
    Err(error) => return Err(error),
}
```

To read every post, follow `next_cursor`:

```rust
let mut options = PostsOptions::new().limit(50);
loop {
    let page = client.posts(options.clone()).await?;
    for post in &page.posts {
        println!("{}", post.title);
    }
    match page.next_cursor {
        Some(cursor) => options = options.cursor(cursor),
        None => break,
    }
}
```

Use `Client::builder()` to set the base URL, the per-request timeout (10 seconds by default), or
your own `reqwest::Client`. Every request sends `User-Agent: sudhanva-rust/0.1.0`.

## API coverage

| Operation                         | Method                                                              |
| --------------------------------- | ------------------------------------------------------------------- |
| `GET /profile`                    | `profile()`, `profile_with_locale()`                                |
| `GET /posts`                      | `posts()` with `limit`, `tag`, and `cursor`                         |
| `GET /posts/{slug}`               | `post()`                                                            |
| `POST /batch`                     | `batch()` with 1 to 20 operations                                   |
| `POST /profile-insights`          | `create_profile_insight()`                                          |
| `GET /profile-insights/{job_id}`  | `profile_insight()` and `wait_for_profile_insight()`                |
| `POST /ask`                       | `ask()` for NLWeb conversational search                             |

The client follows the stable `/api/v1` contract. See the
[developer documentation](https://sudhanva.me/developers/sdks/) and
[versioning policy](https://sudhanva.me/developers/versioning/).

## Dependencies and Rust version

- `reqwest`, with only the `json` and `rustls` features, sends requests over rustls TLS, so the crate
  does not link OpenSSL.
- `serde` and `serde_json` encode request bodies and decode responses into typed structs.
- `thiserror` derives the `Error` and `ApiError` types.
- `tokio`, with only the `time` feature, provides the polling delay in `wait_for_profile_insight`.
  The `blocking` feature adds `rt`.

The minimum supported Rust version is 1.85, the first release with the 2024 edition. CI tests it
alongside the latest stable release. The repository commits `Cargo.lock` so CI builds the same
dependency versions on every run. Cargo ignores it in projects that depend on this crate.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

The default test suite uses a local mock server and never calls production. A smoke test against
production is ignored by default:

```bash
cargo test --all-features --test live -- --ignored
```

## License

MIT
