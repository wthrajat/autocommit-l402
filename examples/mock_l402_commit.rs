use anyhow::Result;
use l402_mock::{EndpointConfig, MockL402Server};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Mock L402 Integration Test...");

    // 1. Spawn a MockL402Server pointing to a deterministic path and response body
    let response_body = r#"{"choices": [{"message": {"role": "assistant", "content": "feat(core): add l402 pay-per-commit support"}}]}"#;
    let endpoint_config = EndpointConfig::new(10).with_body(response_body);

    let server = MockL402Server::builder()
        .endpoint("/v1/chat/completions", endpoint_config)
        .build()
        .await?;

    println!("Mock L402 server running at {}", server.url());

    // 2. Bind the client to server.mock_backend()
    let backend = server.mock_backend();

    // Create a temporary database path for SqliteTokenStore
    let tmp_dir = tempfile::tempdir()?;
    let db_path = tmp_dir.path().join("tokens.db");
    let token_store = l402_sqlite::SqliteTokenStore::new(db_path.to_str().unwrap())?;

    // Setup client-side circuit breaker budget of 150 sats daily
    let budget = l402_core::budget::Budget {
        per_request_max: None,
        hourly_max: None,
        daily_max: Some(150),
        total_max: None,
        domain_budgets: std::collections::HashMap::new(),
    };

    // Instantiate L402Client
    let client = l402_core::L402Client::builder()
        .ln_backend(backend)
        .token_store(token_store)
        .budget(budget)
        .build()?;

    let target_url = format!("{}{}", server.url(), "/v1/chat/completions");
    let request_body = r#"{"model": "gpt-5.4-nano", "messages": [{"role": "user", "content": "generate commit message"}]}"#;

    // 3. Validate end-to-end payment flow

    // First request: triggers payment and caches the token
    println!("Sending first request (requires payment)...");
    let response1 = client.post(&target_url, Some(request_body)).await?;

    assert!(
        response1.paid(),
        "First response should have triggered a payment"
    );
    assert!(
        !response1.cached_token(),
        "First response should not have used a cached token"
    );
    println!(
        "✔ First request successful! paid={}, cached_token={}",
        response1.paid(),
        response1.cached_token()
    );

    let body1 = response1.text().await?;
    println!("Response 1 body: {}", body1);

    // Second request: reuses the cached token, no new payment
    println!("Sending second request (re-uses cached token)...");
    let response2 = client.post(&target_url, Some(request_body)).await?;

    assert!(
        !response2.paid(),
        "Second response should not trigger a new payment"
    );
    assert!(
        response2.cached_token(),
        "Second response should use the cached token"
    );
    println!(
        "✔ Second request successful! paid={}, cached_token={}",
        response2.paid(),
        response2.cached_token()
    );

    let body2 = response2.text().await?;
    println!("Response 2 body: {}", body2);

    assert_eq!(
        body1, body2,
        "Response bodies from cached and paid requests must match"
    );

    println!("All L402 mock integration tests passed successfully!");
    Ok(())
}
