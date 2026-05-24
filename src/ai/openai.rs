use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;

use crate::git::diff::{clean_diff, generate_prompt};
use crate::types::{CommitType, L402Config, MessageStyle};

use super::prompts::{
    FALLBACK_MESSAGE, MAX_DIFF_LENGTH, MAX_TOKENS_LONG, MAX_TOKENS_SHORT, SYSTEM_PROMPT_LONG,
    SYSTEM_PROMPT_SHORT,
};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    max_completion_tokens: u32,
}

#[derive(serde::Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(serde::Deserialize)]
struct Choice {
    message: Option<ChoiceMessage>,
}

#[derive(serde::Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

pub async fn generate_commit_message(
    diff: &str,
    commit_type: Option<CommitType>,
    files: &[String],
    branch_name: &str,
    message_style: MessageStyle,
    l402_config: &L402Config,
) -> Result<String> {
    if diff.trim().is_empty() {
        eprintln!("{} No diff found", "✖".red());
        return Ok(FALLBACK_MESSAGE.to_string());
    }

    let cleaned_diff = clean_diff(diff);
    let truncated_diff: String = cleaned_diff.chars().take(MAX_DIFF_LENGTH).collect();

    let system_prompt = match message_style {
        MessageStyle::Long => SYSTEM_PROMPT_LONG,
        MessageStyle::Short => SYSTEM_PROMPT_SHORT,
    };

    let max_tokens = match message_style {
        MessageStyle::Long => MAX_TOKENS_LONG,
        MessageStyle::Short => MAX_TOKENS_SHORT,
    };

    let user_prompt = generate_prompt(&truncated_diff, commit_type, files, branch_name);

    let request = ChatCompletionRequest {
        model: "gpt-5.4-nano".to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        temperature: 0.0,
        max_completion_tokens: max_tokens,
    };

    if l402_config.enabled {
        let db_path = crate::config::l402_token_db_path()?;
        let db_path_str = db_path.to_str().context("invalid L402 token DB path")?;
        let token_store = l402_sqlite::SqliteTokenStore::new(db_path_str)?;

        let budget = l402_core::budget::Budget {
            per_request_max: None,
            hourly_max: None,
            daily_max: Some(150),
            total_max: None,
            domain_budgets: std::collections::HashMap::new(),
        };

        let backend = if let Some(nwc_uri) = &l402_config.nwc_uri {
            let clean_nwc_uri = nwc_uri.trim().replace(['\n', '\r'], "");
            crate::ai::DynamicLnBackend::Nwc(Box::new(
                l402_nwc::NwcBackend::new(&clean_nwc_uri)
                    .await
                    .map_err(|e| anyhow::anyhow!("NWC initialization failed: {}", e))?,
            ))
        } else if let Some(lnd_host) = &l402_config.lnd_host {
            let clean_lnd_host = lnd_host.trim().replace(['\n', '\r'], "");
            let macaroon_hex = l402_config
                .lnd_macaroon
                .clone()
                .or_else(|| std::env::var("LND_MACAROON").ok())
                .or_else(|| std::env::var("LND_MACAROON_HEX").ok())
                .context("LND_MACAROON or LND_MACAROON_HEX environment variable, or config.yaml lnd_macaroon must be set for LND REST backend")?;
            let clean_macaroon = macaroon_hex.trim().replace(['\n', '\r'], "");
            crate::ai::DynamicLnBackend::Lnd(
                l402_lnd::LndRestBackend::new(&clean_lnd_host, &clean_macaroon)
                    .map_err(|e| anyhow::anyhow!("LND REST initialization failed: {}", e))?,
            )
        } else {
            return Err(anyhow::anyhow!(
                "L402 mode requires either LND REST connection parameters (--lnd-host, config.yaml, or LND_REST_HOST) or a Nostr Wallet Connect URI (--nwc-uri, config.yaml, or NWC_CONNECTION_URI)"
            ));
        };

        let client = l402_core::L402Client::builder()
            .ln_backend(backend)
            .token_store(token_store)
            .budget(budget)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build L402 client: {}", e))?;

        let proxy_url = l402_config
            .l402_proxy
            .as_deref()
            .unwrap_or("http://localhost:8081/v1/chat/completions");

        let request_body = serde_json::to_string(&request)?;
        match client.post(proxy_url, Some(&request_body)).await {
            Ok(response) => {
                let completion = response
                    .json::<ChatCompletionResponse>()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to parse L402 response: {}", e))?;

                if let Some(content) = completion
                    .choices
                    .first()
                    .and_then(|c| c.message.as_ref())
                    .and_then(|m| m.content.as_ref())
                    .map(|c| c.trim().to_string())
                    && !content.is_empty()
                {
                    return Ok(content);
                }
                Ok(match commit_type {
                    Some(t) => format!("{}(scope): update files (fallback)", t.as_str()),
                    None => FALLBACK_MESSAGE.to_string(),
                })
            }
            Err(e) => {
                eprintln!("L402 API Error: {}", e);
                Ok(match commit_type {
                    Some(t) => format!("{}(scope): update files (fallback)", t.as_str()),
                    None => FALLBACK_MESSAGE.to_string(),
                })
            }
        }
    } else {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable is not set")?;

        let client = reqwest::Client::new();
        match client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()
            .await
        {
            Ok(response) => {
                if let Ok(completion) = response.json::<ChatCompletionResponse>().await
                    && let Some(content) = completion
                        .choices
                        .first()
                        .and_then(|c| c.message.as_ref())
                        .and_then(|m| m.content.as_ref())
                        .map(|c| c.trim().to_string())
                    && !content.is_empty()
                {
                    return Ok(content);
                }
                Ok(match commit_type {
                    Some(t) => format!("{}(scope): update files (fallback)", t.as_str()),
                    None => FALLBACK_MESSAGE.to_string(),
                })
            }
            Err(e) => {
                eprintln!("OpenAI API Error: {}", e);
                Ok(match commit_type {
                    Some(t) => format!("{}(scope): update files (fallback)", t.as_str()),
                    None => FALLBACK_MESSAGE.to_string(),
                })
            }
        }
    }
}
