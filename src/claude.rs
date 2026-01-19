// Claude API integration for AI-powered financial analysis
// Queries Claude with financial context and stores conversations locally

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const MAX_TOKENS: u32 = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: Option<String>,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    id: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResult {
    pub response: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialContext {
    pub symbols: Vec<String>,
    pub recent_prices: Vec<PriceContext>,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceContext {
    pub symbol: String,
    pub price: f64,
    pub change_percent: Option<f64>,
    pub date: String,
}

pub struct ClaudeClient {
    client: Client,
    api_key: String,
    model: String,
}

impl ClaudeClient {
    /// Create a new Claude client
    /// API key is read from ANTHROPIC_API_KEY environment variable
    pub fn new() -> Result<Self> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY environment variable not set"))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        Ok(Self {
            client,
            api_key,
            model: DEFAULT_MODEL.to_string(),
        })
    }

    /// Create client with explicit API key
    pub fn with_api_key(api_key: String) -> Result<Self> {
        if api_key.is_empty() {
            return Err(anyhow!("API key cannot be empty"));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        Ok(Self {
            client,
            api_key,
            model: DEFAULT_MODEL.to_string(),
        })
    }

    /// Set the model to use
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    /// Query Claude with financial context
    pub fn query_with_context(
        &self,
        query: &str,
        context: Option<&FinancialContext>,
        conversation_history: Option<&[ClaudeMessage]>,
    ) -> Result<ChatResult> {
        // Build system prompt with financial context
        let system_prompt = self.build_system_prompt(context);

        // Build messages
        let mut messages: Vec<ClaudeMessage> = Vec::new();

        // Add conversation history if provided
        if let Some(history) = conversation_history {
            messages.extend(history.iter().cloned());
        }

        // Add current query
        messages.push(ClaudeMessage {
            role: "user".to_string(),
            content: query.to_string(),
        });

        let request = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: MAX_TOKENS,
            system: Some(system_prompt),
            messages,
        };

        let response = self.client
            .post(CLAUDE_API_URL)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow!("Claude API error {}: {}", status, error_text));
        }

        let claude_response: ClaudeResponse = response.json()?;

        // Extract text from response
        let response_text = claude_response.content
            .iter()
            .filter_map(|block| block.text.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ChatResult {
            response: response_text,
            model: claude_response.model,
            input_tokens: claude_response.usage.input_tokens,
            output_tokens: claude_response.usage.output_tokens,
            conversation_id: claude_response.id,
        })
    }

    /// Simple query without context
    pub fn query(&self, query: &str) -> Result<ChatResult> {
        self.query_with_context(query, None, None)
    }

    fn build_system_prompt(&self, context: Option<&FinancialContext>) -> String {
        let mut prompt = String::from(
            "You are a financial analysis assistant with expertise in stock markets, \
            technical analysis, and investment strategies. You provide clear, actionable \
            insights based on the data provided. Be concise but thorough.\n\n"
        );

        if let Some(ctx) = context {
            prompt.push_str("CURRENT MARKET CONTEXT:\n");

            if !ctx.symbols.is_empty() {
                prompt.push_str(&format!("Tracking symbols: {}\n", ctx.symbols.join(", ")));
            }

            if !ctx.recent_prices.is_empty() {
                prompt.push_str("\nRecent prices:\n");
                for price in &ctx.recent_prices {
                    let change_str = price.change_percent
                        .map(|c| format!(" ({:+.2}%)", c))
                        .unwrap_or_default();
                    prompt.push_str(&format!(
                        "- {}: ${:.2}{} ({})\n",
                        price.symbol, price.price, change_str, price.date
                    ));
                }
            }
            prompt.push_str("\n");
        }

        prompt.push_str(
            "When analyzing stocks or market conditions:\n\
            1. Consider both technical and fundamental factors\n\
            2. Mention relevant risks and uncertainties\n\
            3. Provide specific price levels when relevant\n\
            4. Note any patterns or trends you observe\n"
        );

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_building() {
        let client = ClaudeClient {
            client: Client::new(),
            api_key: "test".to_string(),
            model: DEFAULT_MODEL.to_string(),
        };

        let context = FinancialContext {
            symbols: vec!["AAPL".to_string(), "MSFT".to_string()],
            recent_prices: vec![
                PriceContext {
                    symbol: "AAPL".to_string(),
                    price: 260.94,
                    change_percent: Some(1.5),
                    date: "2026-01-14".to_string(),
                },
            ],
            query: "Test".to_string(),
        };

        let prompt = client.build_system_prompt(Some(&context));
        assert!(prompt.contains("AAPL"));
        assert!(prompt.contains("260.94"));
    }
}
