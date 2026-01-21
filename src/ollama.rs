//! Ollama LLM Integration
//!
//! Local LLM integration via Ollama API (localhost:11434) for AI-powered analysis.
//! Supports sentiment analysis, pattern explanation, price narration, and Q&A.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

// ============================================================================
// Configuration
// ============================================================================

/// Default Ollama API URL
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Fast model for quick queries (sentiment, simple Q&A)
pub const MODEL_FAST: &str = "qwen3:235b";

/// Balanced model for most analysis tasks
pub const MODEL_BALANCED: &str = "gpt-oss:120b-cloud";

/// Heavy model for complex quant analysis (pattern recognition, backtesting advice)
pub const MODEL_HEAVY: &str = "deepseek-v3.2:cloud";

// ============================================================================
// Result Types
// ============================================================================

/// Sentiment classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentimentType {
    Bullish,
    Bearish,
    Neutral,
}

impl SentimentType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bullish" | "positive" | "buy" => SentimentType::Bullish,
            "bearish" | "negative" | "sell" => SentimentType::Bearish,
            _ => SentimentType::Neutral,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SentimentType::Bullish => "bullish",
            SentimentType::Bearish => "bearish",
            SentimentType::Neutral => "neutral",
        }
    }
}

/// Result of sentiment analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentResult {
    pub sentiment: SentimentType,
    pub confidence: f64,
    pub reasoning: String,
}

/// Explanation of a technical pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternExplanation {
    pub pattern_name: String,
    pub explanation: String,
    pub typical_outcome: String,
    pub confidence_level: String,
}

// ============================================================================
// Ollama API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
}

// ============================================================================
// OllamaClient
// ============================================================================

/// Client for interacting with Ollama API
#[derive(Debug, Clone)]
pub struct OllamaClient {
    client: Client,
    base_url: String,
    default_model: String,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaClient {
    /// Create a new client with default settings
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url: DEFAULT_OLLAMA_URL.to_string(),
            default_model: MODEL_BALANCED.to_string(),
        }
    }

    /// Create client with custom URL
    pub fn with_url(url: &str) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url: url.to_string(),
            default_model: MODEL_BALANCED.to_string(),
        }
    }

    /// Set the default model
    pub fn with_model(mut self, model: &str) -> Self {
        self.default_model = model.to_string();
        self
    }

    /// Check if Ollama is available (2-second timeout)
    pub async fn is_available(&self) -> bool {
        let check = async {
            self.client
                .get(format!("{}/api/tags", self.base_url))
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        };

        tokio::time::timeout(Duration::from_secs(2), check)
            .await
            .unwrap_or(false)
    }

    /// Raw LLM query with optional system prompt
    pub async fn query(
        &self,
        prompt: &str,
        system: Option<&str>,
        model: Option<&str>,
    ) -> Result<String> {
        let model = model.unwrap_or(&self.default_model);

        let request = GenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            system: system.map(|s| s.to_string()),
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error: {} - {}", status, body);
        }

        let gen_response: GenerateResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(gen_response.response)
    }

    /// Analyze sentiment of text (news, social media, etc.)
    pub async fn analyze_sentiment(&self, text: &str) -> Result<SentimentResult> {
        let system = "You are a financial sentiment analyzer. Analyze text and respond ONLY with valid JSON.";

        let prompt = format!(
            r#"Analyze the market sentiment of the following text. Respond ONLY with valid JSON in this exact format:
{{
  "sentiment": "bullish|bearish|neutral",
  "confidence": 0.0-1.0,
  "reasoning": "brief explanation"
}}

Text to analyze:
{}"#,
            text
        );

        let response = self.query(&prompt, Some(system), None).await?;

        // Try to parse as JSON
        self.parse_sentiment_response(&response)
    }

    /// Parse sentiment from LLM response (handles both JSON and plain text)
    fn parse_sentiment_response(&self, response: &str) -> Result<SentimentResult> {
        // Try JSON first
        if let Ok(parsed) = serde_json::from_str::<Value>(response) {
            let sentiment = parsed["sentiment"]
                .as_str()
                .map(SentimentType::from_str)
                .unwrap_or(SentimentType::Neutral);
            let confidence = parsed["confidence"].as_f64().unwrap_or(0.5);
            let reasoning = parsed["reasoning"]
                .as_str()
                .unwrap_or("No reasoning provided")
                .to_string();

            return Ok(SentimentResult {
                sentiment,
                confidence: confidence.clamp(0.0, 1.0),
                reasoning,
            });
        }

        // Fallback: parse plain text
        let lower = response.to_lowercase();
        let sentiment = if lower.contains("bullish") || lower.contains("positive") {
            SentimentType::Bullish
        } else if lower.contains("bearish") || lower.contains("negative") {
            SentimentType::Bearish
        } else {
            SentimentType::Neutral
        };

        Ok(SentimentResult {
            sentiment,
            confidence: 0.5,
            reasoning: response.chars().take(200).collect(),
        })
    }

    /// Explain a technical pattern (e.g., "Head and Shoulders", "MACD Bullish Cross")
    pub async fn explain_pattern(&self, pattern_name: &str, context: &str) -> Result<PatternExplanation> {
        let system = "You are a technical analysis educator. Explain patterns clearly for traders.";

        let prompt = format!(
            r#"Explain the technical pattern "{}" in the context of trading. Additional context: {}

Respond ONLY with valid JSON in this exact format:
{{
  "pattern_name": "{}",
  "explanation": "clear explanation of what this pattern is",
  "typical_outcome": "what usually happens after this pattern",
  "confidence_level": "high|medium|low based on pattern reliability"
}}"#,
            pattern_name, context, pattern_name
        );

        let response = self.query(&prompt, Some(system), None).await?;
        self.parse_pattern_response(&response, pattern_name)
    }

    /// Parse pattern explanation from LLM response
    fn parse_pattern_response(&self, response: &str, pattern_name: &str) -> Result<PatternExplanation> {
        // Try JSON first
        if let Ok(parsed) = serde_json::from_str::<Value>(response) {
            return Ok(PatternExplanation {
                pattern_name: parsed["pattern_name"]
                    .as_str()
                    .unwrap_or(pattern_name)
                    .to_string(),
                explanation: parsed["explanation"]
                    .as_str()
                    .unwrap_or("No explanation available")
                    .to_string(),
                typical_outcome: parsed["typical_outcome"]
                    .as_str()
                    .unwrap_or("Outcome varies")
                    .to_string(),
                confidence_level: parsed["confidence_level"]
                    .as_str()
                    .unwrap_or("medium")
                    .to_string(),
            });
        }

        // Fallback: use response as explanation
        Ok(PatternExplanation {
            pattern_name: pattern_name.to_string(),
            explanation: response.chars().take(500).collect(),
            typical_outcome: "See explanation above".to_string(),
            confidence_level: "medium".to_string(),
        })
    }

    /// Generate a natural language summary of price action
    pub async fn narrate_price_action(
        &self,
        symbol: &str,
        prices: &[(String, f64, f64, f64, f64, i64)], // (date, open, high, low, close, volume)
    ) -> Result<String> {
        if prices.is_empty() {
            anyhow::bail!("No price data provided for {}", symbol);
        }

        let system = "You are a market analyst providing clear, concise price action summaries.";

        // Format price data
        let price_summary: Vec<String> = prices
            .iter()
            .take(10) // Limit to recent 10 days
            .map(|(date, open, high, low, close, volume)| {
                format!(
                    "{}: O={:.2} H={:.2} L={:.2} C={:.2} V={}",
                    date, open, high, low, close, volume
                )
            })
            .collect();

        let prompt = format!(
            r#"Summarize the recent price action for {} based on this data:

{}

Provide a concise 2-3 sentence summary covering:
1. Overall trend direction
2. Key price levels
3. Notable volume patterns"#,
            symbol,
            price_summary.join("\n")
        );

        self.query(&prompt, Some(system), None).await
    }

    /// Natural language Q&A about financial data
    pub async fn answer_query(&self, question: &str, context: &str) -> Result<String> {
        let system = "You are a helpful financial assistant. Answer questions based on the provided context. Be concise and accurate.";

        let prompt = format!(
            r#"Context (financial data):
{}

Question: {}

Answer the question based only on the provided context. If the context doesn't contain relevant information, say so."#,
            context, question
        );

        self.query(&prompt, Some(system), None).await
    }

    // ========================================================================
    // Web Search & Thinking Mode (requires OLLAMA_API_KEY)
    // ========================================================================

    /// Query with web search capability for real-time market data
    /// Uses the chat API with tools parameter
    pub async fn query_with_web_search(
        &self,
        prompt: &str,
        system: Option<&str>,
        model: Option<&str>,
    ) -> Result<String> {
        let model = model.unwrap_or("deepseek-v3.1:671b-cloud");

        let mut messages = vec![];
        if let Some(sys) = system {
            messages.push(json!({"role": "system", "content": sys}));
        }
        messages.push(json!({"role": "user", "content": prompt}));

        let request = json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "tools": ["web_search", "web_fetch"]
        });

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send web search request to Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error: {} - {}", status, body);
        }

        let json_response: Value = response.json().await?;
        Ok(json_response["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    /// Query with extended thinking mode for complex analysis
    pub async fn query_with_thinking(
        &self,
        prompt: &str,
        system: Option<&str>,
        model: Option<&str>,
    ) -> Result<(String, Option<String>)> {
        let model = model.unwrap_or("deepseek-v3.1:671b-cloud");

        let mut messages = vec![];
        if let Some(sys) = system {
            messages.push(json!({"role": "system", "content": sys}));
        }
        messages.push(json!({"role": "user", "content": prompt}));

        let request = json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "think": true
        });

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send thinking request to Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error: {} - {}", status, body);
        }

        let json_response: Value = response.json().await?;
        let content = json_response["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let thinking = json_response["message"]["thinking"]
            .as_str()
            .map(|s| s.to_string());

        Ok((content, thinking))
    }

    /// Full-powered query: web search + thinking for market analysis
    pub async fn analyze_market_live(
        &self,
        symbols: &[&str],
        question: &str,
    ) -> Result<String> {
        let symbols_str = symbols.join(", ");
        let prompt = format!(
            "You have access to web search. Search for the latest market data and news about: {}\n\n\
            Question: {}\n\n\
            Provide accurate, current information with sources.",
            symbols_str, question
        );

        let system = "You are a financial analyst with web search access. \
            Always search for current data before answering. \
            Cite your sources. Be specific about dates and prices.";

        self.query_with_web_search(&prompt, Some(system), None).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_type_from_str() {
        assert_eq!(SentimentType::from_str("bullish"), SentimentType::Bullish);
        assert_eq!(SentimentType::from_str("BEARISH"), SentimentType::Bearish);
        assert_eq!(SentimentType::from_str("positive"), SentimentType::Bullish);
        assert_eq!(SentimentType::from_str("negative"), SentimentType::Bearish);
        assert_eq!(SentimentType::from_str("unknown"), SentimentType::Neutral);
    }

    #[test]
    fn test_parse_sentiment_json() {
        let client = OllamaClient::new();
        let json_response = r#"{"sentiment": "bullish", "confidence": 0.85, "reasoning": "Strong buy signals"}"#;
        let result = client.parse_sentiment_response(json_response).unwrap();

        assert_eq!(result.sentiment, SentimentType::Bullish);
        assert!((result.confidence - 0.85).abs() < 0.01);
        assert_eq!(result.reasoning, "Strong buy signals");
    }

    #[test]
    fn test_parse_sentiment_fallback() {
        let client = OllamaClient::new();
        let plain_response = "The market sentiment appears bullish based on recent momentum.";
        let result = client.parse_sentiment_response(plain_response).unwrap();

        assert_eq!(result.sentiment, SentimentType::Bullish);
        assert_eq!(result.confidence, 0.5); // Default for fallback
    }

    #[test]
    fn test_parse_pattern_json() {
        let client = OllamaClient::new();
        let json_response = r#"{"pattern_name": "MACD Cross", "explanation": "Bullish signal", "typical_outcome": "Price increase", "confidence_level": "high"}"#;
        let result = client.parse_pattern_response(json_response, "MACD Cross").unwrap();

        assert_eq!(result.pattern_name, "MACD Cross");
        assert_eq!(result.explanation, "Bullish signal");
        assert_eq!(result.confidence_level, "high");
    }

    #[tokio::test]
    async fn test_client_creation() {
        let client = OllamaClient::new();
        assert_eq!(client.base_url, DEFAULT_OLLAMA_URL);
        assert_eq!(client.default_model, MODEL_BALANCED);  // Default is balanced model

        let custom = OllamaClient::with_url("http://custom:11434").with_model(MODEL_HEAVY);
        assert_eq!(custom.base_url, "http://custom:11434");
        assert_eq!(custom.default_model, MODEL_HEAVY);
    }
}
