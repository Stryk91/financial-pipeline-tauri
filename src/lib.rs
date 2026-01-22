//! Financial Data Pipeline - Rust Edition
//!
//! A high-performance financial data pipeline for:
//! - Fetching stock prices from Yahoo Finance (FREE, unlimited)
//! - Fetching macro economic data from FRED
//! - Storing data in SQLite database
//!
//! # Example
//!
//! ```no_run
//! use financial_pipeline::{Database, YahooFinance, Fred};
//!
//! // Open database
//! let mut db = Database::open("data/finance.db").unwrap();
//! db.init_schema().unwrap();
//!
//! // Fetch stock prices
//! let yahoo = YahooFinance::new();
//! yahoo.fetch_and_store(&mut db, "AAPL", "1y").unwrap();
//!
//! // Fetch macro data
//! let fred = Fred::new();
//! fred.fetch_and_store(&mut db, "DFF").unwrap();
//!
//! // Query latest price
//! let price = db.get_latest_price("AAPL").unwrap();
//! println!("AAPL: ${:.2}", price.unwrap_or(0.0));
//! ```

pub mod db;
pub mod error;
pub mod fred;
pub mod indicators;
pub mod models;
pub mod backtest;
pub mod signals;
pub mod trends;
pub mod vectors;
pub mod yahoo;
pub mod claude;
pub mod finnhub;
pub mod ollama;
pub mod ai_trader;

// Re-exports for convenience
pub use db::Database;
pub use error::{PipelineError, Result};
pub use fred::Fred;
pub use indicators::{
    calculate_adx, calculate_all, calculate_atr, calculate_bollinger_bands, calculate_cci,
    calculate_ema, calculate_macd, calculate_mfi, calculate_obv, calculate_roc, calculate_rsi,
    calculate_sma, calculate_stochastic, calculate_williams_r,
};
pub use models::{
    AlertCondition, BacktestResult, BacktestTrade, DailyPrice, IndicatorAlert,
    IndicatorAlertCondition, IndicatorAlertType, MacroData, PerformanceMetrics, Position,
    PositionType, PriceAlert, Signal, SignalDirection, SignalType, Strategy,
    StrategyConditionType, Symbol, TechnicalIndicator, TradeDirection, Watchlist,
    PaperWallet, PaperPosition, PaperTrade, PaperTradeAction,
};
pub use backtest::{BacktestConfig, BacktestEngine};
pub use signals::{SignalConfig, SignalEngine};
pub use trends::{GoogleTrends, TrendData};
pub use vectors::{VectorStore, MarketEvent, PricePattern, SearchResult as VectorSearchResult, ChatMessage, ChatResponse};
pub use yahoo::YahooFinance;
pub use claude::{ClaudeClient, ClaudeMessage, ChatResult, FinancialContext, PriceContext};
pub use finnhub::{FinnhubClient, NewsItem, SimpleNewsItem, Quote, Candles, PriceReaction};
pub use ai_trader::AiTrader;
pub use models::{
    AiTraderConfig, AiTradingSession, AiTradeDecision, AiPerformanceSnapshot,
    AiPredictionAccuracy, AiTraderStatus, BenchmarkComparison, CompoundingForecast,
    // DC Trader types
    DcWallet, DcPosition, DcTrade, PortfolioSnapshot, TeamConfig, ImportResult, CompetitionStats,
};
