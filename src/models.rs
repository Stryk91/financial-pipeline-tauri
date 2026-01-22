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

/// Position type (buy or sell/short)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionType {
    Buy,
    Sell,
}

/// Portfolio position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub symbol: String,
    pub quantity: f64,
    pub price: f64,
    pub position_type: PositionType,
    pub date: String,
    pub notes: Option<String>,
}

// ============================================================================
// Signal Generation Types
// ============================================================================

/// Type of trading signal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    // RSI signals
    RsiOverbought,
    RsiOversold,
    // MACD signals
    MacdBullishCross,
    MacdBearishCross,
    // Bollinger Band signals
    BollingerUpperBreak,
    BollingerLowerBreak,
    // Moving Average signals
    MaCrossoverBullish,
    MaCrossoverBearish,
    // ADX signals
    AdxTrendStrong,
    AdxTrendWeak,
    // Stochastic signals
    StochBullishCross,
    StochBearishCross,
    // Williams %R signals
    WillrOverbought,
    WillrOversold,
    // CCI signals
    CciOverbought,
    CciOversold,
    // MFI signals
    MfiOverbought,
    MfiOversold,
}

impl SignalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignalType::RsiOverbought => "RSI_OVERBOUGHT",
            SignalType::RsiOversold => "RSI_OVERSOLD",
            SignalType::MacdBullishCross => "MACD_BULLISH_CROSS",
            SignalType::MacdBearishCross => "MACD_BEARISH_CROSS",
            SignalType::BollingerUpperBreak => "BB_UPPER_BREAK",
            SignalType::BollingerLowerBreak => "BB_LOWER_BREAK",
            SignalType::MaCrossoverBullish => "MA_BULLISH_CROSS",
            SignalType::MaCrossoverBearish => "MA_BEARISH_CROSS",
            SignalType::AdxTrendStrong => "ADX_TREND_STRONG",
            SignalType::AdxTrendWeak => "ADX_TREND_WEAK",
            SignalType::StochBullishCross => "STOCH_BULLISH_CROSS",
            SignalType::StochBearishCross => "STOCH_BEARISH_CROSS",
            SignalType::WillrOverbought => "WILLR_OVERBOUGHT",
            SignalType::WillrOversold => "WILLR_OVERSOLD",
            SignalType::CciOverbought => "CCI_OVERBOUGHT",
            SignalType::CciOversold => "CCI_OVERSOLD",
            SignalType::MfiOverbought => "MFI_OVERBOUGHT",
            SignalType::MfiOversold => "MFI_OVERSOLD",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "RSI_OVERBOUGHT" => Some(SignalType::RsiOverbought),
            "RSI_OVERSOLD" => Some(SignalType::RsiOversold),
            "MACD_BULLISH_CROSS" => Some(SignalType::MacdBullishCross),
            "MACD_BEARISH_CROSS" => Some(SignalType::MacdBearishCross),
            "BB_UPPER_BREAK" => Some(SignalType::BollingerUpperBreak),
            "BB_LOWER_BREAK" => Some(SignalType::BollingerLowerBreak),
            "MA_BULLISH_CROSS" => Some(SignalType::MaCrossoverBullish),
            "MA_BEARISH_CROSS" => Some(SignalType::MaCrossoverBearish),
            "ADX_TREND_STRONG" => Some(SignalType::AdxTrendStrong),
            "ADX_TREND_WEAK" => Some(SignalType::AdxTrendWeak),
            "STOCH_BULLISH_CROSS" => Some(SignalType::StochBullishCross),
            "STOCH_BEARISH_CROSS" => Some(SignalType::StochBearishCross),
            "WILLR_OVERBOUGHT" => Some(SignalType::WillrOverbought),
            "WILLR_OVERSOLD" => Some(SignalType::WillrOversold),
            "CCI_OVERBOUGHT" => Some(SignalType::CciOverbought),
            "CCI_OVERSOLD" => Some(SignalType::CciOversold),
            "MFI_OVERBOUGHT" => Some(SignalType::MfiOverbought),
            "MFI_OVERSOLD" => Some(SignalType::MfiOversold),
            _ => None,
        }
    }
}

/// Direction of the signal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalDirection {
    Bullish,
    Bearish,
    Neutral,
}

impl SignalDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignalDirection::Bullish => "bullish",
            SignalDirection::Bearish => "bearish",
            SignalDirection::Neutral => "neutral",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bullish" => SignalDirection::Bullish,
            "bearish" => SignalDirection::Bearish,
            _ => SignalDirection::Neutral,
        }
    }
}

/// A generated trading signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: i64,
    pub symbol: String,
    pub signal_type: SignalType,
    pub direction: SignalDirection,
    pub strength: f64,
    pub price_at_signal: f64,
    pub triggered_by: String,
    pub trigger_value: f64,
    pub timestamp: NaiveDate,
    pub created_at: String,
    pub acknowledged: bool,
}

// ============================================================================
// Indicator Alert Types
// ============================================================================

/// Type of indicator alert
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndicatorAlertType {
    Threshold,    // RSI crosses 30, ADX crosses 25, etc.
    Crossover,    // MACD crosses signal, SMA20 crosses SMA50
    BandTouch,    // Price touches Bollinger bands
}

impl IndicatorAlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IndicatorAlertType::Threshold => "threshold",
            IndicatorAlertType::Crossover => "crossover",
            IndicatorAlertType::BandTouch => "band_touch",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "threshold" => Some(IndicatorAlertType::Threshold),
            "crossover" => Some(IndicatorAlertType::Crossover),
            "band_touch" => Some(IndicatorAlertType::BandTouch),
            _ => None,
        }
    }
}

/// Condition for indicator alerts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndicatorAlertCondition {
    CrossesAbove,
    CrossesBelow,
    BullishCrossover,
    BearishCrossover,
}

impl IndicatorAlertCondition {
    pub fn as_str(&self) -> &'static str {
        match self {
            IndicatorAlertCondition::CrossesAbove => "crosses_above",
            IndicatorAlertCondition::CrossesBelow => "crosses_below",
            IndicatorAlertCondition::BullishCrossover => "bullish_crossover",
            IndicatorAlertCondition::BearishCrossover => "bearish_crossover",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "crosses_above" => Some(IndicatorAlertCondition::CrossesAbove),
            "crosses_below" => Some(IndicatorAlertCondition::CrossesBelow),
            "bullish_crossover" => Some(IndicatorAlertCondition::BullishCrossover),
            "bearish_crossover" => Some(IndicatorAlertCondition::BearishCrossover),
            _ => None,
        }
    }
}

/// An indicator-based alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorAlert {
    pub id: i64,
    pub symbol: String,
    pub alert_type: IndicatorAlertType,
    pub indicator_name: String,
    pub secondary_indicator: Option<String>,
    pub condition: IndicatorAlertCondition,
    pub threshold: Option<f64>,
    pub triggered: bool,
    pub last_value: Option<f64>,
    pub created_at: String,
    pub message: Option<String>,
}

// ============================================================================
// Backtesting Types
// ============================================================================

/// Strategy entry/exit condition type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyConditionType {
    RsiOversold,      // RSI < threshold (buy signal)
    RsiOverbought,    // RSI > threshold (sell signal)
    MacdCrossUp,      // MACD crosses above signal line
    MacdCrossDown,    // MACD crosses below signal line
    PriceAboveSma,    // Price > SMA
    PriceBelowSma,    // Price < SMA
    SmaCrossUp,       // Fast SMA crosses above slow SMA
    SmaCrossDown,     // Fast SMA crosses below slow SMA
    StopLoss,         // Price falls below entry - threshold%
    TakeProfit,       // Price rises above entry + threshold%
}

impl StrategyConditionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StrategyConditionType::RsiOversold => "rsi_oversold",
            StrategyConditionType::RsiOverbought => "rsi_overbought",
            StrategyConditionType::MacdCrossUp => "macd_cross_up",
            StrategyConditionType::MacdCrossDown => "macd_cross_down",
            StrategyConditionType::PriceAboveSma => "price_above_sma",
            StrategyConditionType::PriceBelowSma => "price_below_sma",
            StrategyConditionType::SmaCrossUp => "sma_cross_up",
            StrategyConditionType::SmaCrossDown => "sma_cross_down",
            StrategyConditionType::StopLoss => "stop_loss",
            StrategyConditionType::TakeProfit => "take_profit",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rsi_oversold" => Some(StrategyConditionType::RsiOversold),
            "rsi_overbought" => Some(StrategyConditionType::RsiOverbought),
            "macd_cross_up" => Some(StrategyConditionType::MacdCrossUp),
            "macd_cross_down" => Some(StrategyConditionType::MacdCrossDown),
            "price_above_sma" => Some(StrategyConditionType::PriceAboveSma),
            "price_below_sma" => Some(StrategyConditionType::PriceBelowSma),
            "sma_cross_up" => Some(StrategyConditionType::SmaCrossUp),
            "sma_cross_down" => Some(StrategyConditionType::SmaCrossDown),
            "stop_loss" => Some(StrategyConditionType::StopLoss),
            "take_profit" => Some(StrategyConditionType::TakeProfit),
            _ => None,
        }
    }
}

/// A trading strategy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub entry_condition: StrategyConditionType,
    pub entry_threshold: f64,
    pub exit_condition: StrategyConditionType,
    pub exit_threshold: f64,
    pub stop_loss_percent: Option<f64>,
    pub take_profit_percent: Option<f64>,
    pub position_size_percent: f64, // % of capital per trade
    pub created_at: String,
}

/// Trade direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeDirection {
    Long,
    Short,
}

impl TradeDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            TradeDirection::Long => "long",
            TradeDirection::Short => "short",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "short" => TradeDirection::Short,
            _ => TradeDirection::Long,
        }
    }
}

/// A single trade from backtesting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub id: i64,
    pub backtest_id: i64,
    pub symbol: String,
    pub direction: TradeDirection,
    pub entry_date: NaiveDate,
    pub entry_price: f64,
    pub exit_date: Option<NaiveDate>,
    pub exit_price: Option<f64>,
    pub shares: f64,
    pub entry_reason: String,
    pub exit_reason: Option<String>,
    pub profit_loss: Option<f64>,
    pub profit_loss_percent: Option<f64>,
}

/// Performance metrics from backtesting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_return: f64,
    pub total_return_dollars: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub avg_win_percent: f64,
    pub avg_loss_percent: f64,
    pub profit_factor: f64,
    pub avg_trade_duration_days: f64,
}

/// Complete backtest result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub id: i64,
    pub strategy_id: i64,
    pub strategy_name: String,
    pub symbol: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub initial_capital: f64,
    pub final_capital: f64,
    pub metrics: PerformanceMetrics,
    pub trades: Vec<BacktestTrade>,
    pub created_at: String,
}

// ============================================================================
// Paper Trading Types
// ============================================================================

/// Paper trading wallet (tracks cash balance)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperWallet {
    pub id: i64,
    pub cash: f64,
    pub created_at: String,
    pub updated_at: String,
}

/// Paper trading position (open position)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperPosition {
    pub id: i64,
    pub symbol: String,
    pub quantity: f64,
    pub entry_price: f64,
    pub entry_date: String,
    pub linked_event_id: Option<i64>,
}

/// Paper trading action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperTradeAction {
    Buy,
    Sell,
}

impl PaperTradeAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaperTradeAction::Buy => "BUY",
            PaperTradeAction::Sell => "SELL",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "SELL" => PaperTradeAction::Sell,
            _ => PaperTradeAction::Buy,
        }
    }
}

/// Paper trade history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTrade {
    pub id: i64,
    pub symbol: String,
    pub action: PaperTradeAction,
    pub quantity: f64,
    pub price: f64,
    pub pnl: Option<f64>,           // Calculated on SELL
    pub timestamp: String,
    pub linked_event_id: Option<i64>,
    pub notes: Option<String>,
}

// ============================================================================
// DC Trader Types (Separate from KALIC AI paper trading)
// ============================================================================

/// DC trader wallet (tracks cash balance with starting capital)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcWallet {
    pub id: i64,
    pub cash: f64,
    pub starting_capital: f64,
    pub created_at: String,
    pub updated_at: String,
}

/// DC trading position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcPosition {
    pub id: i64,
    pub symbol: String,
    pub quantity: f64,
    pub entry_price: f64,
    pub entry_date: String,
}

/// DC trade history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcTrade {
    pub id: i64,
    pub symbol: String,
    pub action: String,
    pub quantity: f64,
    pub price: f64,
    pub pnl: Option<f64>,
    pub timestamp: String,
    pub notes: Option<String>,
}

/// Portfolio snapshot for performance charting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSnapshot {
    pub id: i64,
    pub team: String,
    pub date: String,
    pub total_value: f64,
    pub cash: f64,
    pub positions_value: f64,
}

/// Team configuration preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub kalic_starting_capital: f64,
    pub dc_starting_capital: f64,
    pub created_at: String,
}

/// Result of batch import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub success_count: i32,
    pub error_count: i32,
    pub errors: Vec<String>,
}

/// Competition statistics comparing KALIC and DC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionStats {
    pub kalic_total: f64,
    pub kalic_cash: f64,
    pub kalic_positions: f64,
    pub kalic_pnl_pct: f64,
    pub kalic_trades: i32,
    pub dc_total: f64,
    pub dc_cash: f64,
    pub dc_positions: f64,
    pub dc_pnl_pct: f64,
    pub dc_trades: i32,
    pub leader: String,
    pub lead_amount: f64,
}

// ============================================================================
// Confluence Signal Types
// ============================================================================

/// Individual indicator vote in a confluence signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorVote {
    pub indicator_name: String,
    pub direction: SignalDirection,
    pub strength: f64,
    pub value: f64,
}

/// Configuration for confluence signal detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfluenceConfig {
    pub min_agreeing_indicators: usize,
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub stoch_oversold: f64,
    pub stoch_overbought: f64,
    pub cci_oversold: f64,
    pub cci_overbought: f64,
    pub adx_strong_trend: f64,
}

impl Default for ConfluenceConfig {
    fn default() -> Self {
        Self {
            min_agreeing_indicators: 3,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stoch_oversold: 20.0,
            stoch_overbought: 80.0,
            cci_oversold: -100.0,
            cci_overbought: 100.0,
            adx_strong_trend: 25.0,
        }
    }
}

/// A confluence signal that fires when 3+ indicators agree on direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfluenceSignal {
    pub id: i64,
    pub symbol: String,
    pub date: NaiveDate,
    pub direction: SignalDirection,
    pub strength: f64,
    pub contributing_indicators: Vec<IndicatorVote>,
    pub bullish_count: usize,
    pub bearish_count: usize,
    pub adx_confidence: Option<f64>,
    pub price_at_signal: f64,
    pub created_at: String,
}

// ============================================================================
// AI Trading Simulator Types
// ============================================================================

/// AI Trader Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTraderConfig {
    pub starting_capital: f64,
    pub max_position_size_percent: f64,
    pub stop_loss_percent: f64,
    pub take_profit_percent: f64,
    pub session_duration_minutes: u32,
    pub benchmark_symbol: String,
    pub model_priority: Vec<String>,
    // Trading mode: aggressive, normal, conservative, paused
    pub trading_mode: String,
    // Circuit breaker settings
    pub daily_loss_threshold: f64,
    pub consecutive_loss_limit: i32,
    pub auto_conservative_on_trigger: bool,
    // Guardrail settings
    pub max_daily_trades: i32,
    pub max_single_trade_value: f64,
    pub require_confluence: bool,
    pub blocked_hours: String,
}

impl Default for AiTraderConfig {
    fn default() -> Self {
        Self {
            starting_capital: 1_000_000.0,
            max_position_size_percent: 10.0,
            stop_loss_percent: 5.0,
            take_profit_percent: 15.0,
            session_duration_minutes: 60,
            benchmark_symbol: "SPY".to_string(),
            model_priority: vec![
                "deepseek-v3.2:cloud".to_string(),
                "gpt-oss:120b-cloud".to_string(),
                "qwen3:235b".to_string(),
            ],
            trading_mode: "normal".to_string(),
            daily_loss_threshold: -10.0,
            consecutive_loss_limit: 5,
            auto_conservative_on_trigger: true,
            max_daily_trades: 10,
            max_single_trade_value: 50_000.0,
            require_confluence: true,
            blocked_hours: "09:30-09:45,15:45-16:00".to_string(),
        }
    }
}

/// AI Trading Session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTradingSession {
    pub id: i64,
    pub start_time: String,
    pub end_time: Option<String>,
    pub starting_portfolio_value: f64,
    pub ending_portfolio_value: Option<f64>,
    pub decisions_count: i32,
    pub trades_count: i32,
    pub session_notes: Option<String>,
    pub status: String,
}

/// AI Trade Decision with reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTradeDecision {
    pub id: i64,
    pub session_id: Option<i64>,
    pub timestamp: String,
    pub action: String,
    pub symbol: String,
    pub quantity: Option<f64>,
    pub price_at_decision: Option<f64>,
    pub confidence: f64,
    pub reasoning: String,
    pub model_used: String,
    pub predicted_direction: Option<String>,
    pub predicted_price_target: Option<f64>,
    pub predicted_timeframe_days: Option<i32>,
    pub actual_outcome: Option<String>,
    pub actual_price_at_timeframe: Option<f64>,
    pub prediction_accurate: Option<bool>,
    pub paper_trade_id: Option<i64>,
}

/// AI Performance Snapshot for charting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPerformanceSnapshot {
    pub id: i64,
    pub timestamp: String,
    pub portfolio_value: f64,
    pub cash: f64,
    pub positions_value: f64,
    pub benchmark_value: f64,
    pub benchmark_symbol: String,
    pub total_pnl: f64,
    pub total_pnl_percent: f64,
    pub benchmark_pnl_percent: f64,
    pub prediction_accuracy: Option<f64>,
    pub trades_to_date: i32,
    pub winning_trades: i32,
    pub losing_trades: i32,
    pub win_rate: Option<f64>,
}

/// AI Prediction Accuracy Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPredictionAccuracy {
    pub total_predictions: u32,
    pub accurate_predictions: u32,
    pub accuracy_percent: f64,
}

/// AI Trader Status Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTraderStatus {
    pub is_running: bool,
    pub current_session: Option<AiTradingSession>,
    pub portfolio_value: f64,
    pub cash: f64,
    pub positions_value: f64,
    pub is_bankrupt: bool,
    pub sessions_completed: u32,
    pub total_decisions: u32,
    pub total_trades: u32,
}

/// Benchmark Comparison Data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub portfolio_return_percent: f64,
    pub benchmark_return_percent: f64,
    pub alpha: f64,
    pub tracking_data: Vec<(String, f64, f64)>, // (timestamp, portfolio_value, benchmark_value)
}

impl Default for BenchmarkComparison {
    fn default() -> Self {
        Self {
            portfolio_return_percent: 0.0,
            benchmark_return_percent: 0.0,
            alpha: 0.0,
            tracking_data: Vec::new(),
        }
    }
}

/// Compounding Forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundingForecast {
    pub current_daily_return: f64,
    pub current_win_rate: f64,
    pub projected_30_days: f64,
    pub projected_90_days: f64,
    pub projected_365_days: f64,
    pub time_to_double: Option<u32>,
    pub time_to_bankruptcy: Option<u32>,
}

impl CompoundingForecast {
    pub fn insufficient_data() -> Self {
        Self {
            current_daily_return: 0.0,
            current_win_rate: 0.0,
            projected_30_days: 0.0,
            projected_90_days: 0.0,
            projected_365_days: 0.0,
            time_to_double: None,
            time_to_bankruptcy: None,
        }
    }
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
