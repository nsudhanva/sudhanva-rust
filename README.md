# sudhanva for Rust

This is a small async Rust client for the public [sudhanva.me API](https://sudhanva.me/openapi.json).
Use it to get the published profile and article metadata, to read items in batches of a limited
size, and to search the published site. You can also use it to create and poll temporary
profile-insight jobs.

The API is public. It does not need credentials. Do not send private data to it.

## Install

Install it from [crates.io](https://crates.io/crates/sudhanva):

```bash
cargo add sudhanva@0.1.0
```

You can also add it to `Cargo.toml`:

```toml
[dependencies]
sudhanva = "0.1.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The client is async and runs on Tokio. To get a synchronous client, enable the `blocking` feature.

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

Each profile-insight request must have an idempotency key. The caller controls this key, and the
client sends it in the `Idempotency-Key` header. If you send the same key and request again within
24 hours, the API returns the same job.

Methods return typed structs. If a field is missing, the struct uses the default value for that
field. The client ignores unknown fields. Thus, new fields in an API response do not cause decode
errors. Batch result bodies and extra NLWeb result properties stay available as raw
`serde_json::Value`.

If a response is not a success, the client returns `sudhanva::Error::Api`. This error contains an
`ApiError` with the fields `status`, `code`, `message`, `hint`, `docs_url`, and the decoded response
`body`. The client decodes the JSON error envelope. It also decodes the `application/problem+json`
documents that profile-insight requests return. For example:

```rust
match client.post("missing").await {
    Ok(post) => println!("{}", post.post.title),
    Err(sudhanva::Error::Api(error)) if error.status == 404 => println!("{}", error.code),
    Err(error) => return Err(error),
}
```

To read all posts, use `next_cursor` to get the next page:

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

Use `Client::builder()` to set the base URL, the timeout for each request (default 10 seconds), or
your own `reqwest::Client`. Each request sends `User-Agent: sudhanva-rust/0.1.0`.

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

The client uses the stable `/api/v1` contract. For more information, read the
[developer documentation](https://sudhanva.me/developers/sdks/) and the
[versioning policy](https://sudhanva.me/developers/versioning/).

## Dependencies and Rust version

- `reqwest` sends requests over rustls TLS. It uses only the `json` and `rustls` features. Thus, the
  crate does not link OpenSSL.
- `serde` and `serde_json` encode request bodies and decode responses into typed structs.
- `thiserror` derives the `Error` and `ApiError` types.
- `tokio` gives the delay between polls in `wait_for_profile_insight`. It uses only the `time`
  feature. The `blocking` feature adds `rt`.

The minimum supported Rust version is 1.85. This is the first release with the 2024 edition. CI
tests this version and the latest stable release.

The repository contains `Cargo.lock`, so CI builds the same dependency versions on each run. Cargo
ignores `Cargo.lock` in projects that depend on this crate.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

The default test suite uses a local mock server. It does not call production.

A smoke test against production is also available. It does not run by default. To run it, use this
command:

```bash
cargo test --all-features --test live -- --ignored
```

## License

MIT
