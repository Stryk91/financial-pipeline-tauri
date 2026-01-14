//! Data models for Financial Pipeline

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Stock symbol metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub symbol: String,
    pub name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub market_cap: Option<f64>,
    pub country: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
    pub isin: Option<String>,
    pub asset_class: Option<String>,
}

/// Daily price data (OHLCV)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyPrice {
    pub symbol: String,
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub source: String,
}

/// Macro economic indicator data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroData {
    pub indicator: String,
    pub date: NaiveDate,
    pub value: f64,
    pub source: String,
}

/// Watchlist definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Watchlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
}

/// API call log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCall {
    pub id: i64,
    pub source: String,
    pub endpoint: String,
    pub symbol: String,
    pub timestamp: String,
}

/// Technical indicator value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicator {
    pub symbol: String,
    pub date: NaiveDate,
    pub indicator_name: String,
    pub value: f64,
}

/// Price alert condition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertCondition {
    Above,
    Below,
}

/// Price alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceAlert {
    pub id: i64,
    pub symbol: String,
    pub target_price: f64,
    pub condition: AlertCondition,
    pub triggered: bool,
    pub created_at: String,
}

/// Yahoo Finance chart response structures
pub mod yahoo {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct ChartResponse {
        pub chart: Chart,
    }

    #[derive(Debug, Deserialize)]
    pub struct Chart {
        pub result: Option<Vec<ChartResult>>,
        pub error: Option<ChartError>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ChartError {
        pub code: String,
        pub description: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ChartResult {
        pub meta: ChartMeta,
        pub timestamp: Option<Vec<i64>>,
        pub indicators: Indicators,
    }

    #[derive(Debug, Deserialize)]
    pub struct ChartMeta {
        pub symbol: String,
        pub currency: Option<String>,
        #[serde(rename = "exchangeName")]
        pub exchange_name: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Indicators {
        pub quote: Vec<Quote>,
        pub adjclose: Option<Vec<AdjClose>>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Quote {
        pub open: Vec<Option<f64>>,
        pub high: Vec<Option<f64>>,
        pub low: Vec<Option<f64>>,
        pub close: Vec<Option<f64>>,
        pub volume: Vec<Option<i64>>,
    }

    #[derive(Debug, Deserialize)]
    pub struct AdjClose {
        pub adjclose: Vec<Option<f64>>,
    }
}
