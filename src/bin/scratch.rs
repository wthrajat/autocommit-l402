use reqwest::header::{HeaderMap, HeaderValue, HOST};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    // Probe the proxy with Host: api.openai.com
    let mut headers = HeaderMap::new();
    headers.insert(HOST, HeaderValue::from_static("api.openai.com"));
    
    println!("--- Testing unauthenticated request with Host: api.openai.com ---");
    let res = client.post("http://localhost:8081/v1/chat/completions")
        .headers(headers.clone())
        .body(r#"{"model":"gpt-4o-mini","messages":[{"role":"user","content":"ping"}]}"#)
        .send()
        .await?;

    println!("Status: {}", res.status());
    println!("Headers: {:#?}", res.headers());
    println!("Body: {}\n", res.text().await?);

    // Probe the proxy without custom Host header
    println!("--- Testing unauthenticated request WITHOUT custom Host header ---");
    let res2 = client.post("http://localhost:8081/v1/chat/completions")
        .body(r#"{"model":"gpt-4o-mini","messages":[{"role":"user","content":"ping"}]}"#)
        .send()
        .await?;

    println!("Status: {}", res2.status());
    println!("Headers: {:#?}", res2.headers());
    println!("Body: {}\n", res2.text().await?);

    Ok(())
}
