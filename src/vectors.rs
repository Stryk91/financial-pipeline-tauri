// Vector database integration for financial data
// Uses SQLite for storage with simple cosine similarity search
// Provides semantic search, pattern matching, and AI chat capabilities

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

const EMBEDDING_DIM: usize = 128; // Smaller dimension for simple hash-based embeddings

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub id: String,
    pub symbol: String,
    pub event_type: String, // news, earnings, split, dividend, pattern
    pub title: String,
    pub content: String,
    pub date: String,
    pub sentiment: Option<f32>, // -1.0 to 1.0
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePattern {
    pub id: String,
    pub symbol: String,
    pub pattern_type: String, // bullish, bearish, reversal, breakout
    pub start_date: String,
    pub end_date: String,
    pub price_change_percent: f32,
    pub volume_change_percent: f32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub content: String,
    pub score: f32,
    pub result_type: String,
    pub symbol: Option<String>,
    pub date: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: String,
    pub sources: Vec<SearchResult>,
}

pub struct VectorStore {
    conn: Connection,
}

impl VectorStore {
    pub fn new(db_path: &str) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = Path::new(db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;

        let store = Self { conn };
        store.init_tables()?;

        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        // Market events table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS market_events (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                event_type TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                date TEXT NOT NULL,
                sentiment REAL,
                metadata TEXT,
                embedding BLOB NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Price patterns table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS price_patterns (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                pattern_type TEXT NOT NULL,
                start_date TEXT NOT NULL,
                end_date TEXT NOT NULL,
                price_change_percent REAL NOT NULL,
                volume_change_percent REAL NOT NULL,
                description TEXT NOT NULL,
                embedding BLOB NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create indexes
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_symbol ON market_events(symbol)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_patterns_symbol ON price_patterns(symbol)",
            [],
        )?;

        Ok(())
    }

    /// Generate a simple embedding from text using hash-based approach
    /// In production, replace with actual embedding model (local Ollama or API)
    fn generate_embedding(text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0f32; EMBEDDING_DIM];
        let text_lower = text.to_lowercase();
        let words: Vec<&str> = text_lower.split_whitespace().collect();

        // Simple bag-of-words style embedding with positional encoding
        for (word_idx, word) in words.iter().enumerate() {
            let bytes = word.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                // Hash-like distribution across embedding dimensions
                let idx = ((b as usize) * 31 + i * 17 + word_idx * 7) % EMBEDDING_DIM;
                embedding[idx] += 1.0;

                // Add character n-gram features
                if i + 1 < bytes.len() {
                    let bigram_idx = ((b as usize * 256 + bytes[i + 1] as usize) * 13) % EMBEDDING_DIM;
                    embedding[bigram_idx] += 0.5;
                }
            }
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }

    /// Convert embedding to bytes for storage
    fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect()
    }

    /// Convert bytes back to embedding
    fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks(4)
            .map(|chunk| {
                let arr: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                f32::from_le_bytes(arr)
            })
            .collect()
    }

    /// Calculate cosine similarity between two embeddings
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Add a market event to the vector store
    pub fn add_market_event(&self, event: &MarketEvent) -> Result<()> {
        let text = format!("{} {} {}", event.title, event.content, event.event_type);
        let embedding = Self::generate_embedding(&text);
        let embedding_bytes = Self::embedding_to_bytes(&embedding);

        self.conn.execute(
            "INSERT OR REPLACE INTO market_events
             (id, symbol, event_type, title, content, date, sentiment, metadata, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.id,
                event.symbol,
                event.event_type,
                event.title,
                event.content,
                event.date,
                event.sentiment,
                event.metadata,
                embedding_bytes,
            ],
        )?;

        Ok(())
    }

    /// Search for similar market events
    pub fn search_events(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = Self::generate_embedding(query);

        let mut stmt = self.conn.prepare(
            "SELECT id, symbol, event_type, title, content, date, metadata, embedding
             FROM market_events"
        )?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let symbol: String = row.get(1)?;
            let event_type: String = row.get(2)?;
            let title: String = row.get(3)?;
            let content: String = row.get(4)?;
            let date: String = row.get(5)?;
            let metadata: Option<String> = row.get(6)?;
            let embedding_bytes: Vec<u8> = row.get(7)?;
            Ok((id, symbol, event_type, title, content, date, metadata, embedding_bytes))
        })?;

        let mut results: Vec<SearchResult> = Vec::new();

        for row in rows {
            let (id, symbol, _event_type, title, content, date, metadata, embedding_bytes) = row?;
            let embedding = Self::bytes_to_embedding(&embedding_bytes);
            let score = Self::cosine_similarity(&query_embedding, &embedding);

            results.push(SearchResult {
                id,
                content: format!("{}: {}", title, content),
                score,
                result_type: "market_event".to_string(),
                symbol: Some(symbol),
                date: Some(date),
                metadata,
            });
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Add a price pattern to the vector store
    pub fn add_price_pattern(&self, pattern: &PricePattern) -> Result<()> {
        let text = format!(
            "{} {} price change {}% volume change {}%",
            pattern.pattern_type, pattern.description,
            pattern.price_change_percent, pattern.volume_change_percent
        );
        let embedding = Self::generate_embedding(&text);
        let embedding_bytes = Self::embedding_to_bytes(&embedding);

        self.conn.execute(
            "INSERT OR REPLACE INTO price_patterns
             (id, symbol, pattern_type, start_date, end_date, price_change_percent,
              volume_change_percent, description, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                pattern.id,
                pattern.symbol,
                pattern.pattern_type,
                pattern.start_date,
                pattern.end_date,
                pattern.price_change_percent,
                pattern.volume_change_percent,
                pattern.description,
                embedding_bytes,
            ],
        )?;

        Ok(())
    }

    /// Search for similar price patterns
    pub fn search_patterns(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = Self::generate_embedding(query);

        let mut stmt = self.conn.prepare(
            "SELECT id, symbol, pattern_type, start_date, end_date, description, embedding
             FROM price_patterns"
        )?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let symbol: String = row.get(1)?;
            let pattern_type: String = row.get(2)?;
            let start_date: String = row.get(3)?;
            let _end_date: String = row.get(4)?;
            let description: String = row.get(5)?;
            let embedding_bytes: Vec<u8> = row.get(6)?;
            Ok((id, symbol, pattern_type, start_date, description, embedding_bytes))
        })?;

        let mut results: Vec<SearchResult> = Vec::new();

        for row in rows {
            let (id, symbol, pattern_type, start_date, description, embedding_bytes) = row?;
            let embedding = Self::bytes_to_embedding(&embedding_bytes);
            let score = Self::cosine_similarity(&query_embedding, &embedding);

            results.push(SearchResult {
                id,
                content: format!("{}: {}", pattern_type, description),
                score,
                result_type: "price_pattern".to_string(),
                symbol: Some(symbol),
                date: Some(start_date),
                metadata: None,
            });
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Combined search across all tables
    pub fn search_all(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut all_results = Vec::new();

        // Search events
        if let Ok(events) = self.search_events(query, limit) {
            all_results.extend(events);
        }

        // Search patterns
        if let Ok(patterns) = self.search_patterns(query, limit) {
            all_results.extend(patterns);
        }

        // Sort by score descending
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Limit total results
        all_results.truncate(limit);

        Ok(all_results)
    }

    /// Get table statistics
    pub fn get_stats(&self) -> Result<(usize, usize)> {
        let events_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM market_events",
            [],
            |row| row.get(0),
        )?;

        let patterns_count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM price_patterns",
            [],
            |row| row.get(0),
        )?;

        Ok((events_count, patterns_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_store() {
        let store = VectorStore::new("./test_vectors.db").unwrap();

        let event = MarketEvent {
            id: "test1".to_string(),
            symbol: "AAPL".to_string(),
            event_type: "earnings".to_string(),
            title: "Apple beats earnings expectations".to_string(),
            content: "Apple reported Q4 earnings above analyst expectations with strong iPhone sales".to_string(),
            date: "2024-01-15".to_string(),
            sentiment: Some(0.8),
            metadata: None,
        };

        store.add_market_event(&event).unwrap();

        let results = store.search_events("Apple iPhone sales", 5).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);
    }
}
