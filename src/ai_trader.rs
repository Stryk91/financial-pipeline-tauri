//! AI Trading Simulator
//!
//! Autonomous AI-powered trading engine that uses Ollama LLMs to make trading decisions.
//! Features:
//! - Full autonomy - no human confirmation required
//! - $1M starting capital
//! - DeepSeek v3.2 primary model with fallbacks
//! - Detailed decision logging with AI reasoning
//! - Performance tracking vs SPY benchmark
//! - Compounding forecast projections

use anyhow::{Context, Result};
use chrono::{Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::db::Database;
use crate::models::{
    AiPerformanceSnapshot, AiTradeDecision, AiTraderConfig, AiTraderStatus, AiTradingSession,
    BenchmarkComparison, CompoundingForecast,
};
use crate::ollama::OllamaClient;
use crate::signals::SignalEngine;

// ============================================================================
// Constants
// ============================================================================

/// Bankruptcy threshold - below this we're done
pub const BANKRUPTCY_THRESHOLD: f64 = 1000.0;

/// System prompt for the AI trader
pub const AI_TRADER_SYSTEM_PROMPT: &str = r#"
You are an autonomous AI trading agent managing a virtual portfolio. Your goal is to MAXIMIZE RETURNS through aggressive trading decisions based on technical analysis.

CRITICAL RULES:
1. You make ALL decisions autonomously - no human confirmation needed
2. You must make at least 3-4 trades per trading session - DEPLOY CAPITAL AGGRESSIVELY
3. You operate until the portfolio reaches bankruptcy (<$1,000)
4. CASH EARNS NOTHING - idle cash is losing to inflation. Deploy it!
5. Mental stop-loss: 5%, take-profit: 15%

CAPITAL DEPLOYMENT PHILOSOPHY:
- Target 80-100% capital deployment at all times
- Cash is a DRAG on returns - minimize idle cash
- Better to have 4-5 positions than 1-2 large ones (diversification)
- If you see confluence signals, BUY. Don't overthink it.

DECISION FRAMEWORK:
1. Prioritize confluence signals (3+ indicators agreeing) - these are your GREEN LIGHTS
2. Even moderate confluence (2 indicators) warrants a position
3. Cut losses early on positions without supporting signals
4. Let winners run when confluence remains strong

RESPONSE FORMAT:
You MUST respond with ONLY valid JSON, no other text. Format:
{
  "decisions": [
    {
      "action": "BUY" | "SELL" | "HOLD",
      "symbol": "TICKER",
      "quantity_percent": 0-100 (percent of available cash for BUY, percent of position for SELL),
      "confidence": 0.0-1.0,
      "reasoning": "Detailed explanation of why this decision was made",
      "prediction": {
        "direction": "bullish" | "bearish" | "neutral",
        "price_target": 123.45,
        "timeframe_days": 5
      }
    }
  ],
  "market_outlook": "Brief overall market assessment",
  "session_notes": "Any relevant notes about this trading session"
}

RISK MANAGEMENT:
- Position sizing based on conviction (higher confidence = larger position)
- Close positions that violate stop-loss even if still bullish
- NO CASH HOARDING - deploy capital or explain why not
"#;

/// Default logs directory path
const LOGS_DIR: &str = "logs/ai_decisions";

// ============================================================================
// Logging Infrastructure - FILE-BASED FAILSAFE
// ============================================================================

/// Log entry for AI trading decisions
#[derive(Debug, Clone, Serialize)]
pub struct AiTradeLog {
    pub timestamp: String,
    pub model: String,
    pub prompt: String,
    pub raw_response: String,
    pub parsed_decisions: Option<AiDecisionResponse>,
    pub error: Option<String>,
}

impl AiTradeLog {
    /// Create a new log entry
    pub fn new(model: &str, prompt: &str) -> Self {
        Self {
            timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            model: model.to_string(),
            prompt: prompt.to_string(),
            raw_response: String::new(),
            parsed_decisions: None,
            error: None,
        }
    }

    /// Save log to file (JSON format)
    pub fn save(&self, logs_dir: Option<&str>) -> Result<PathBuf> {
        let dir = logs_dir.unwrap_or(LOGS_DIR);
        fs::create_dir_all(dir)?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}/ai_decision_{}.json", dir, timestamp);
        let path = PathBuf::from(&filename);

        let json = serde_json::to_string_pretty(self)?;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        file.write_all(json.as_bytes())?;

        println!("[AI Trader] Decision logged to: {}", filename);
        Ok(path)
    }

    /// Append to daily log file (JSONL format - one JSON object per line)
    pub fn append_to_daily_log(&self, logs_dir: Option<&str>) -> Result<()> {
        let dir = logs_dir.unwrap_or(LOGS_DIR);
        fs::create_dir_all(dir)?;

        let date = Utc::now().format("%Y%m%d");
        let filename = format!("{}/decisions_{}.jsonl", dir, date);

        let json = serde_json::to_string(self)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filename)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }
}

/// Log raw DeepSeek/LLM response to separate file for debugging
pub fn log_raw_response(model: &str, prompt: &str, response: &str, logs_dir: Option<&str>) -> Result<PathBuf> {
    let dir = format!("{}/raw", logs_dir.unwrap_or(LOGS_DIR));
    fs::create_dir_all(&dir)?;

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}/raw_{}_{}.txt", dir, model.replace(":", "_"), timestamp);
    let path = PathBuf::from(&filename);

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;

    writeln!(file, "=== AI TRADER RAW LOG ===")?;
    writeln!(file, "Timestamp: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"))?;
    writeln!(file, "Model: {}", model)?;
    writeln!(file, "")?;
    writeln!(file, "=== PROMPT SENT ===")?;
    writeln!(file, "{}", prompt)?;
    writeln!(file, "")?;
    writeln!(file, "=== RAW RESPONSE ===")?;
    writeln!(file, "{}", response)?;

    Ok(path)
}

/// Decision index entry for manifest tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionIndexEntry {
    pub id: String,
    pub timestamp: String,
    pub model: String,
    pub symbol: String,
    pub action: String,
    pub quantity_percent: f64,
    pub confidence: f64,
    pub predicted_direction: String,
    pub predicted_price_target: f64,
    pub log_file: String,
    // Outcome tracking (filled in later)
    pub outcome_recorded: bool,
    pub actual_pnl: Option<f64>,
    pub prediction_accurate: Option<bool>,
}

/// Decision index manifest
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionIndex {
    pub last_updated: String,
    pub total_decisions: usize,
    pub decisions_with_outcomes: usize,
    pub accuracy_rate: Option<f64>,
    pub entries: Vec<DecisionIndexEntry>,
}

impl DecisionIndex {
    /// Load existing index or create new
    pub fn load(logs_dir: Option<&str>) -> Self {
        let path = format!("{}/index.json", logs_dir.unwrap_or(LOGS_DIR));
        if let Ok(content) = fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Add a decision entry
    pub fn add_decision(&mut self, entry: DecisionIndexEntry) {
        self.entries.push(entry);
        self.total_decisions = self.entries.len();
        self.last_updated = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    }

    /// Update outcome for a decision by ID
    pub fn update_outcome(&mut self, id: &str, pnl: f64, accurate: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.outcome_recorded = true;
            entry.actual_pnl = Some(pnl);
            entry.prediction_accurate = Some(accurate);
        }
        self.recalculate_stats();
    }

    /// Recalculate aggregate stats
    fn recalculate_stats(&mut self) {
        let with_outcomes: Vec<_> = self.entries.iter().filter(|e| e.outcome_recorded).collect();
        self.decisions_with_outcomes = with_outcomes.len();

        if !with_outcomes.is_empty() {
            let accurate_count = with_outcomes.iter().filter(|e| e.prediction_accurate == Some(true)).count();
            self.accuracy_rate = Some(accurate_count as f64 / with_outcomes.len() as f64 * 100.0);
        }
        self.last_updated = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    }

    /// Save index to file
    pub fn save(&self, logs_dir: Option<&str>) -> Result<()> {
        let dir = logs_dir.unwrap_or(LOGS_DIR);
        fs::create_dir_all(dir)?;
        let path = format!("{}/index.json", dir);
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }
}

/// Add decision to index manifest
pub fn index_decision(
    model: &str,
    decision: &ParsedDecision,
    log_file: &str,
    logs_dir: Option<&str>,
) -> Result<String> {
    let mut index = DecisionIndex::load(logs_dir);

    let id = format!("{}_{}",
        Utc::now().format("%Y%m%d%H%M%S"),
        &decision.symbol
    );

    let (direction, target) = match &decision.prediction {
        Some(p) => (p.direction.clone(), p.price_target),
        None => ("unknown".to_string(), 0.0),
    };

    let entry = DecisionIndexEntry {
        id: id.clone(),
        timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        model: model.to_string(),
        symbol: decision.symbol.clone(),
        action: decision.action.clone(),
        quantity_percent: decision.quantity_percent,
        confidence: decision.confidence,
        predicted_direction: direction,
        predicted_price_target: target,
        log_file: log_file.to_string(),
        outcome_recorded: false,
        actual_pnl: None,
        prediction_accurate: None,
    };

    index.add_decision(entry);
    index.save(logs_dir)?;

    Ok(id)
}

// ============================================================================
// Market Context Types
// ============================================================================

/// Complete market context sent to AI for decision making
#[derive(Debug, Clone, Serialize)]
pub struct MarketContext {
    pub timestamp: String,
    pub portfolio: PortfolioSnapshot,
    pub symbols_data: Vec<SymbolContext>,
    pub recent_trades: Vec<RecentTrade>,
    pub prediction_accuracy: f64,
    pub constraints: TradingConstraints,
}

/// Current portfolio state
#[derive(Debug, Clone, Serialize)]
pub struct PortfolioSnapshot {
    pub cash: f64,
    pub positions: Vec<PositionInfo>,
    pub total_value: f64,
    pub total_pnl: f64,
    pub total_pnl_percent: f64,
}

/// Position info for context
#[derive(Debug, Clone, Serialize)]
pub struct PositionInfo {
    pub symbol: String,
    pub quantity: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub unrealized_pnl_percent: f64,
}

/// Symbol analysis context
#[derive(Debug, Clone, Serialize)]
pub struct SymbolContext {
    pub symbol: String,
    pub current_price: f64,
    pub price_change_percent: f64,
    pub indicators: HashMap<String, f64>,
    pub signals: Vec<SignalSummary>,
    pub confluence: Option<ConfluenceSummary>,
}

/// Signal summary for context
#[derive(Debug, Clone, Serialize)]
pub struct SignalSummary {
    pub signal_type: String,
    pub direction: String,
    pub strength: f64,
}

/// Confluence summary for context
#[derive(Debug, Clone, Serialize)]
pub struct ConfluenceSummary {
    pub direction: String,
    pub strength: f64,
    pub agreeing_indicators: usize,
}

/// Recent trade for context
#[derive(Debug, Clone, Serialize)]
pub struct RecentTrade {
    pub symbol: String,
    pub action: String,
    pub quantity: f64,
    pub price: f64,
    pub pnl: Option<f64>,
    pub timestamp: String,
}

/// Trading constraints
#[derive(Debug, Clone, Serialize)]
pub struct TradingConstraints {
    pub max_position_size_percent: f64,
    pub stop_loss_percent: f64,
    pub take_profit_percent: f64,
    pub min_cash_reserve_percent: f64,
}

// ============================================================================
// AI Response Types
// ============================================================================

/// Parsed AI decision response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDecisionResponse {
    pub decisions: Vec<ParsedDecision>,
    pub market_outlook: Option<String>,
    pub session_notes: Option<String>,
}

/// Single parsed decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDecision {
    pub action: String,
    pub symbol: String,
    pub quantity_percent: f64,
    pub confidence: f64,
    pub reasoning: String,
    pub prediction: Option<PredictionInfo>,
}

/// Prediction info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionInfo {
    pub direction: String,
    pub price_target: f64,
    pub timeframe_days: i32,
}

// ============================================================================
// Trade Guardrails & Circuit Breaker
// ============================================================================

/// Trading mode determines aggressiveness of AI decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingMode {
    /// Maximum position sizes, lowest barriers - STRYK override only
    Aggressive,
    /// Default operation - balanced risk
    Normal,
    /// Reduced position sizes, higher confluence required - triggered by circuit breaker
    Conservative,
    /// No new trades, only manage existing positions
    Paused,
}

impl Default for TradingMode {
    fn default() -> Self {
        TradingMode::Normal
    }
}

impl std::fmt::Display for TradingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradingMode::Aggressive => write!(f, "aggressive"),
            TradingMode::Normal => write!(f, "normal"),
            TradingMode::Conservative => write!(f, "conservative"),
            TradingMode::Paused => write!(f, "paused"),
        }
    }
}

impl TradingMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "aggressive" => TradingMode::Aggressive,
            "normal" => TradingMode::Normal,
            "conservative" => TradingMode::Conservative,
            "paused" => TradingMode::Paused,
            _ => TradingMode::Normal,
        }
    }
}

/// Trade guardrails with tiered limits based on mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeGuardrails {
    pub mode: TradingMode,
    /// Maximum position size as percent of portfolio (varies by mode)
    pub max_position_pct: f64,
    /// Maximum trades per day
    pub max_daily_trades: u32,
    /// Maximum single trade value in dollars
    pub max_single_trade_value: f64,
    /// Require confluence signal support for trades
    pub require_confluence: bool,
    /// Hours to block trading (open/close volatility)
    pub blocked_hours: Vec<(u8, u8)>,
}

impl TradeGuardrails {
    /// Get guardrails for a specific trading mode
    pub fn for_mode(mode: TradingMode) -> Self {
        match mode {
            TradingMode::Aggressive => Self {
                mode,
                max_position_pct: 33.0,  // 33% max (via STRYK override only)
                max_daily_trades: 20,
                max_single_trade_value: 100_000.0,
                require_confluence: false,
                blocked_hours: vec![],  // No restrictions
            },
            TradingMode::Normal => Self {
                mode,
                max_position_pct: 10.0,  // 10% default
                max_daily_trades: 10,
                max_single_trade_value: 50_000.0,
                require_confluence: true,
                blocked_hours: vec![(9, 9), (15, 16)],  // 9:00-9:45, 15:45-16:00
            },
            TradingMode::Conservative => Self {
                mode,
                max_position_pct: 5.0,  // 5% max
                max_daily_trades: 5,
                max_single_trade_value: 25_000.0,
                require_confluence: true,
                blocked_hours: vec![(9, 10), (15, 16)],  // Extended blocked
            },
            TradingMode::Paused => Self {
                mode,
                max_position_pct: 0.0,  // No new positions
                max_daily_trades: 0,
                max_single_trade_value: 0.0,
                require_confluence: true,
                blocked_hours: vec![(0, 24)],  // All hours blocked
            },
        }
    }
}

impl Default for TradeGuardrails {
    fn default() -> Self {
        Self::for_mode(TradingMode::Normal)
    }
}

/// Circuit breaker - automatic mode switch on losses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreaker {
    /// Daily loss threshold as percent (e.g., -10.0 for -10%)
    pub daily_loss_threshold: f64,
    /// Consecutive losing trades before pause
    pub consecutive_loss_limit: u32,
    /// Auto-switch to conservative on trigger
    pub auto_conservative_on_trigger: bool,
    /// Currently triggered
    pub triggered: bool,
    /// Resume trading after this time (pause period)
    pub resume_at: Option<chrono::DateTime<Utc>>,
    /// Track consecutive losses
    pub consecutive_losses: u32,
    /// Today's P/L for threshold check
    pub daily_pnl_percent: f64,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self {
            daily_loss_threshold: -10.0,
            consecutive_loss_limit: 5,
            auto_conservative_on_trigger: true,
            triggered: false,
            resume_at: None,
            consecutive_losses: 0,
            daily_pnl_percent: 0.0,
        }
    }
}

impl CircuitBreaker {
    /// Check if circuit breaker should trigger
    pub fn should_trigger(&self) -> Option<CircuitBreakerTrigger> {
        if self.daily_pnl_percent <= self.daily_loss_threshold {
            return Some(CircuitBreakerTrigger::DailyLossThreshold);
        }
        if self.consecutive_losses >= self.consecutive_loss_limit {
            return Some(CircuitBreakerTrigger::ConsecutiveLosses);
        }
        None
    }

    /// Record a losing trade
    pub fn record_loss(&mut self) {
        self.consecutive_losses += 1;
    }

    /// Record a winning trade (resets consecutive losses)
    pub fn record_win(&mut self) {
        self.consecutive_losses = 0;
    }

    /// Update daily P/L
    pub fn update_daily_pnl(&mut self, pnl_percent: f64) {
        self.daily_pnl_percent = pnl_percent;
    }

    /// Trigger the circuit breaker with a pause duration
    pub fn trigger(&mut self, pause_hours: u32) {
        self.triggered = true;
        self.resume_at = Some(Utc::now() + chrono::Duration::hours(pause_hours as i64));
    }

    /// Check if pause period has ended
    pub fn can_resume(&self) -> bool {
        if !self.triggered {
            return true;
        }
        match self.resume_at {
            Some(resume) => Utc::now() >= resume,
            None => true,
        }
    }

    /// Reset the circuit breaker
    pub fn reset(&mut self) {
        self.triggered = false;
        self.resume_at = None;
        self.consecutive_losses = 0;
    }
}

/// Circuit breaker trigger reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakerTrigger {
    DailyLossThreshold,
    ConsecutiveLosses,
    ManualPause,
}

impl std::fmt::Display for CircuitBreakerTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerTrigger::DailyLossThreshold => write!(f, "daily_loss_threshold"),
            CircuitBreakerTrigger::ConsecutiveLosses => write!(f, "consecutive_losses"),
            CircuitBreakerTrigger::ManualPause => write!(f, "manual_pause"),
        }
    }
}

/// Trade execution result with full audit trail
#[derive(Debug, Clone, Serialize)]
pub enum TradeResult {
    /// Trade was executed successfully
    Executed {
        trade_id: String,
        symbol: String,
        action: String,
        quantity: f64,
        price: f64,
        value: f64,
        timestamp: String,
    },
    /// Trade was queued for human review
    Queued {
        reason: String,
        review_by: String,
        proposed_trade: ProposedTrade,
    },
    /// Trade was rejected by guardrails
    Rejected {
        reason: String,
        rule_triggered: String,
        proposed_trade: ProposedTrade,
    },
}

/// Proposed trade before execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedTrade {
    pub action: String,
    pub symbol: String,
    pub quantity: f64,
    pub quantity_percent: f64,
    pub estimated_value: f64,
    pub confidence: f64,
    pub reasoning: String,
}

/// STRYK override - temporary elevated permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Override {
    pub enabled: bool,
    /// Override expires at this time
    pub expires_at: Option<chrono::DateTime<Utc>>,
    /// Temporary higher position limit
    pub max_position_pct: Option<f64>,
    /// Override reason for audit
    pub reason: Option<String>,
}

impl Default for Override {
    fn default() -> Self {
        Self {
            enabled: false,
            expires_at: None,
            max_position_pct: None,
            reason: None,
        }
    }
}

impl Override {
    /// Create a time-limited override
    pub fn timed(hours: u32, max_pct: f64, reason: &str) -> Self {
        Self {
            enabled: true,
            expires_at: Some(Utc::now() + chrono::Duration::hours(hours as i64)),
            max_position_pct: Some(max_pct),
            reason: Some(reason.to_string()),
        }
    }

    /// Check if override is still active
    pub fn is_active(&self) -> bool {
        if !self.enabled {
            return false;
        }
        match self.expires_at {
            Some(expires) => Utc::now() < expires,
            None => true,
        }
    }

    /// Clear the override
    pub fn clear(&mut self) {
        self.enabled = false;
        self.expires_at = None;
        self.max_position_pct = None;
        self.reason = None;
    }
}

/// Trade rejection record for audit log
#[derive(Debug, Clone, Serialize)]
pub struct TradeRejection {
    pub timestamp: String,
    pub session_id: Option<i64>,
    pub attempted_action: String,
    pub symbol: String,
    pub quantity: Option<f64>,
    pub quantity_percent: Option<f64>,
    pub estimated_value: Option<f64>,
    pub reason: String,
    pub rule_triggered: String,
    pub trading_mode: String,
    pub raw_request: Option<String>,
}

// ============================================================================
// AiTrader Implementation
// ============================================================================

/// The main AI trading engine
pub struct AiTrader {
    pub config: AiTraderConfig,
    ollama: OllamaClient,
    signal_engine: SignalEngine,
    /// Current guardrails (derived from mode)
    pub guardrails: TradeGuardrails,
    /// Circuit breaker state
    pub circuit_breaker: CircuitBreaker,
    /// STRYK override state
    pub override_state: Override,
}

impl AiTrader {
    /// Create a new AI trader with the given configuration
    pub fn new(config: AiTraderConfig) -> Self {
        let ollama = OllamaClient::new();
        let signal_engine = SignalEngine::new();
        let mode = TradingMode::from_str(&config.trading_mode);
        let guardrails = TradeGuardrails::for_mode(mode);
        let circuit_breaker = CircuitBreaker {
            daily_loss_threshold: config.daily_loss_threshold,
            consecutive_loss_limit: config.consecutive_loss_limit as u32,
            auto_conservative_on_trigger: config.auto_conservative_on_trigger,
            ..Default::default()
        };

        Self {
            config,
            ollama,
            signal_engine,
            guardrails,
            circuit_breaker,
            override_state: Override::default(),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(AiTraderConfig::default())
    }

    // ========================================================================
    // Mode & Guardrails Management
    // ========================================================================

    /// Get current trading mode
    pub fn get_mode(&self) -> TradingMode {
        self.guardrails.mode
    }

    /// Switch trading mode (persists to database)
    pub fn switch_mode(&mut self, db: &Database, new_mode: TradingMode, reason: Option<&str>) -> Result<()> {
        let old_mode = self.guardrails.mode;
        self.guardrails = TradeGuardrails::for_mode(new_mode);

        // Persist to database
        db.update_trading_mode(&new_mode.to_string())?;

        // Log the mode switch
        println!("[AI Trader] Mode switch: {} -> {} (reason: {})",
            old_mode, new_mode, reason.unwrap_or("manual"));

        Ok(())
    }

    /// Apply STRYK override
    pub fn apply_override(&mut self, hours: u32, max_pct: f64, reason: &str) -> Result<()> {
        self.override_state = Override::timed(hours, max_pct, reason);
        println!("[AI Trader] Override applied: {}% max for {}h ({})",
            max_pct, hours, reason);
        Ok(())
    }

    /// Clear STRYK override
    pub fn clear_override(&mut self) {
        self.override_state.clear();
        println!("[AI Trader] Override cleared");
    }

    /// Get effective max position size (considers override)
    pub fn get_effective_max_position(&self) -> f64 {
        if self.override_state.is_active() {
            self.override_state.max_position_pct.unwrap_or(self.guardrails.max_position_pct)
        } else {
            self.guardrails.max_position_pct
        }
    }

    // ========================================================================
    // Circuit Breaker
    // ========================================================================

    /// Check and handle circuit breaker
    pub fn check_circuit_breaker(&mut self, db: &Database) -> Result<Option<CircuitBreakerTrigger>> {
        // Update daily P/L
        let (_, _, total_value) = db.get_paper_portfolio_value()?;
        let starting_value = self.config.starting_capital;
        let daily_pnl_pct = ((total_value - starting_value) / starting_value) * 100.0;
        self.circuit_breaker.update_daily_pnl(daily_pnl_pct);

        // Check trigger conditions
        if let Some(trigger) = self.circuit_breaker.should_trigger() {
            self.circuit_breaker.trigger(1); // 1 hour pause

            // Auto-switch to conservative if configured
            if self.circuit_breaker.auto_conservative_on_trigger {
                let _ = self.switch_mode(db, TradingMode::Conservative, Some(&trigger.to_string()));
            }

            // Log circuit breaker event
            db.log_circuit_breaker_event(
                &trigger.to_string(),
                &self.guardrails.mode.to_string(),
                "conservative",
                daily_pnl_pct,
                self.circuit_breaker.consecutive_losses as i32,
            )?;

            println!("[AI Trader] CIRCUIT BREAKER TRIGGERED: {:?}", trigger);
            return Ok(Some(trigger));
        }

        Ok(None)
    }

    /// Record trade outcome for circuit breaker
    pub fn record_trade_outcome(&mut self, is_win: bool) {
        if is_win {
            self.circuit_breaker.record_win();
        } else {
            self.circuit_breaker.record_loss();
        }
    }

    // ========================================================================
    // Trade Validation
    // ========================================================================

    /// Validate a proposed trade against guardrails
    pub fn validate_trade(
        &self,
        db: &Database,
        proposed: &ProposedTrade,
        has_confluence: bool,
    ) -> Result<TradeResult> {
        // Check if paused
        if self.guardrails.mode == TradingMode::Paused {
            return Ok(TradeResult::Rejected {
                reason: "Trading is paused".to_string(),
                rule_triggered: "mode_paused".to_string(),
                proposed_trade: proposed.clone(),
            });
        }

        // Check circuit breaker pause
        if self.circuit_breaker.triggered && !self.circuit_breaker.can_resume() {
            return Ok(TradeResult::Rejected {
                reason: format!("Circuit breaker active until {:?}",
                    self.circuit_breaker.resume_at),
                rule_triggered: "circuit_breaker_pause".to_string(),
                proposed_trade: proposed.clone(),
            });
        }

        // Check position size
        let max_pct = self.get_effective_max_position();
        if proposed.quantity_percent > max_pct {
            return Ok(TradeResult::Rejected {
                reason: format!("Position size {:.1}% exceeds max {:.1}%",
                    proposed.quantity_percent, max_pct),
                rule_triggered: "max_position_size".to_string(),
                proposed_trade: proposed.clone(),
            });
        }

        // Check single trade value
        if proposed.estimated_value > self.guardrails.max_single_trade_value {
            return Ok(TradeResult::Rejected {
                reason: format!("Trade value ${:.2} exceeds max ${:.2}",
                    proposed.estimated_value, self.guardrails.max_single_trade_value),
                rule_triggered: "max_trade_value".to_string(),
                proposed_trade: proposed.clone(),
            });
        }

        // Check confluence requirement
        if self.guardrails.require_confluence && !has_confluence {
            return Ok(TradeResult::Rejected {
                reason: "Trade requires confluence signal support".to_string(),
                rule_triggered: "require_confluence".to_string(),
                proposed_trade: proposed.clone(),
            });
        }

        // Check daily trade limit
        let today_trades = db.get_paper_trades_today()?;
        if today_trades.len() as u32 >= self.guardrails.max_daily_trades {
            return Ok(TradeResult::Rejected {
                reason: format!("Daily trade limit reached ({}/{})",
                    today_trades.len(), self.guardrails.max_daily_trades),
                rule_triggered: "max_daily_trades".to_string(),
                proposed_trade: proposed.clone(),
            });
        }

        // Check blocked hours
        let current_hour = Utc::now().hour() as u8;
        for (start, end) in &self.guardrails.blocked_hours {
            if current_hour >= *start && current_hour < *end {
                return Ok(TradeResult::Rejected {
                    reason: format!("Trading blocked during hours {}-{}", start, end),
                    rule_triggered: "blocked_hours".to_string(),
                    proposed_trade: proposed.clone(),
                });
            }
        }

        // All validations passed - trade can proceed
        // Note: Actual execution happens elsewhere, this just validates
        Ok(TradeResult::Executed {
            trade_id: String::new(), // Will be filled by execute
            symbol: proposed.symbol.clone(),
            action: proposed.action.clone(),
            quantity: proposed.quantity,
            price: 0.0, // Will be filled by execute
            value: proposed.estimated_value,
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    /// Log a trade rejection to the database
    pub fn log_rejection(&self, db: &Database, rejection: &TradeRejection) -> Result<()> {
        db.log_trade_rejection(rejection)?;
        println!("[AI Trader] REJECTION: {} {} - {} (rule: {})",
            rejection.attempted_action,
            rejection.symbol,
            rejection.reason,
            rejection.rule_triggered);
        Ok(())
    }

    /// Check if Ollama is available
    pub async fn check_ollama(&self) -> bool {
        self.ollama.is_available().await
    }

    /// Get the current AI trader status
    pub fn get_status(&self, db: &Database) -> Result<AiTraderStatus> {
        let (cash, positions_value, total_value) = db.get_paper_portfolio_value()?;
        let active_session = db.get_active_ai_session()?;
        let sessions_completed = db.get_ai_sessions_count()?;
        let total_decisions = db.get_ai_decisions_count()?;

        let trades = db.get_paper_trades(None, 10000)?;
        let total_trades = trades.len() as u32;

        Ok(AiTraderStatus {
            is_running: active_session.is_some(),
            current_session: active_session,
            portfolio_value: total_value,
            cash,
            positions_value,
            is_bankrupt: total_value < BANKRUPTCY_THRESHOLD,
            sessions_completed,
            total_decisions,
            total_trades,
        })
    }

    /// Start a new trading session
    pub fn start_session(&self, db: &Database) -> Result<AiTradingSession> {
        // Check if already in a session
        if let Some(session) = db.get_active_ai_session()? {
            return Ok(session);
        }

        let (_, _, total_value) = db.get_paper_portfolio_value()?;
        let session_id = db.start_ai_session(total_value)?;

        db.get_ai_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Failed to retrieve created session"))
    }

    /// End the current trading session
    pub fn end_session(&self, db: &Database, notes: Option<&str>) -> Result<Option<AiTradingSession>> {
        if let Some(session) = db.get_active_ai_session()? {
            let (_, _, total_value) = db.get_paper_portfolio_value()?;
            db.end_ai_session(session.id, total_value, notes)?;
            return Ok(db.get_ai_session(session.id)?);
        }
        Ok(None)
    }

    /// Run one autonomous trading cycle
    pub async fn run_cycle(&self, db: &mut Database) -> Result<Vec<AiTradeDecision>> {
        // Check bankruptcy
        let (_, _, total_value) = db.get_paper_portfolio_value()?;
        if total_value < BANKRUPTCY_THRESHOLD {
            anyhow::bail!("Portfolio is bankrupt (value: ${:.2})", total_value);
        }

        // Get active session
        let session = db.get_active_ai_session()?;
        let session_id = session.map(|s| s.id);

        // Gather market context
        let context = self.gather_market_context(db)?;

        // Query AI for decisions
        let decisions = self.query_ai_for_decisions(&context).await?;

        // Execute decisions
        let mut recorded_decisions = Vec::new();
        for decision in decisions {
            match self.execute_decision(db, session_id, &decision, &context).await {
                Ok(recorded) => recorded_decisions.push(recorded),
                Err(e) => {
                    eprintln!("[AI Trader] Failed to execute decision for {}: {}", decision.symbol, e);
                }
            }
        }

        // Record performance snapshot
        self.record_performance_snapshot(db)?;

        Ok(recorded_decisions)
    }

    /// Gather market context for AI decision making
    pub fn gather_market_context(&self, db: &Database) -> Result<MarketContext> {
        let (cash, positions_value, total_value) = db.get_paper_portfolio_value()?;
        let positions = db.get_paper_positions()?;
        let recent_trades = db.get_paper_trades(None, 10)?;

        // Get prediction accuracy
        let accuracy = db.get_ai_prediction_accuracy()?;

        // Build portfolio snapshot
        let mut position_infos = Vec::new();
        for pos in &positions {
            let current_price = db.get_latest_price(&pos.symbol)?.unwrap_or(pos.entry_price);
            let cost_basis = pos.quantity * pos.entry_price;
            let current_value = pos.quantity * current_price;
            let unrealized_pnl = current_value - cost_basis;
            let unrealized_pnl_percent = if cost_basis > 0.0 {
                (unrealized_pnl / cost_basis) * 100.0
            } else {
                0.0
            };

            position_infos.push(PositionInfo {
                symbol: pos.symbol.clone(),
                quantity: pos.quantity,
                entry_price: pos.entry_price,
                current_price,
                unrealized_pnl,
                unrealized_pnl_percent,
            });
        }

        let starting_capital = self.config.starting_capital;
        let total_pnl = total_value - starting_capital;
        let total_pnl_percent = (total_pnl / starting_capital) * 100.0;

        let portfolio = PortfolioSnapshot {
            cash,
            positions: position_infos,
            total_value,
            total_pnl,
            total_pnl_percent,
        };

        // Get symbols to analyze (positions + watchlist)
        let mut symbols: Vec<String> = positions.iter().map(|p| p.symbol.clone()).collect();

        // Add favorited symbols
        if let Ok(favorited) = db.get_favorited_symbols() {
            for sym in favorited {
                if !symbols.contains(&sym) {
                    symbols.push(sym);
                }
            }
        }

        // If no symbols, use some defaults
        if symbols.is_empty() {
            symbols = vec!["SPY".to_string(), "QQQ".to_string(), "AAPL".to_string(), "NVDA".to_string()];
        }

        // Gather symbol data
        let mut symbols_data = Vec::new();
        for symbol in &symbols {
            if let Ok(sym_context) = self.gather_symbol_context(db, symbol) {
                symbols_data.push(sym_context);
            }
        }

        // Recent trades
        let recent = recent_trades
            .iter()
            .map(|t| RecentTrade {
                symbol: t.symbol.clone(),
                action: t.action.as_str().to_string(),
                quantity: t.quantity,
                price: t.price,
                pnl: t.pnl,
                timestamp: t.timestamp.clone(),
            })
            .collect();

        Ok(MarketContext {
            timestamp: Utc::now().to_rfc3339(),
            portfolio,
            symbols_data,
            recent_trades: recent,
            prediction_accuracy: accuracy.accuracy_percent,
            constraints: TradingConstraints {
                max_position_size_percent: self.config.max_position_size_percent,
                stop_loss_percent: self.config.stop_loss_percent,
                take_profit_percent: self.config.take_profit_percent,
                min_cash_reserve_percent: 20.0,
            },
        })
    }

    /// Gather context for a single symbol
    fn gather_symbol_context(&self, db: &Database, symbol: &str) -> Result<SymbolContext> {
        let prices = db.get_prices(symbol)?;
        if prices.is_empty() {
            anyhow::bail!("No price data for {}", symbol);
        }

        let current_price = prices.last().map(|p| p.close).unwrap_or(0.0);
        let prev_price = if prices.len() >= 2 {
            prices[prices.len() - 2].close
        } else {
            current_price
        };
        let price_change_percent = if prev_price > 0.0 {
            ((current_price - prev_price) / prev_price) * 100.0
        } else {
            0.0
        };

        // Get indicators
        let indicators_list = db.get_all_indicators(symbol)?;
        let mut indicators: HashMap<String, f64> = HashMap::new();
        for ind in &indicators_list {
            indicators.insert(ind.indicator_name.clone(), ind.value);
        }

        // Get signals (all signals, not just unacknowledged)
        let signals = db.get_signals(symbol, false)?;
        let signal_summaries: Vec<SignalSummary> = signals
            .iter()
            .take(5)
            .map(|s| SignalSummary {
                signal_type: format!("{:?}", s.signal_type),
                direction: format!("{:?}", s.direction),
                strength: s.strength,
            })
            .collect();

        // Check for confluence using signal engine
        let latest_price = prices.last().unwrap();
        let confluence = if let Some(c) = self.signal_engine.detect_confluence_signal(
            symbol,
            latest_price.date,
            current_price,
            &indicators,
        ) {
            Some(ConfluenceSummary {
                direction: format!("{:?}", c.direction),
                strength: c.strength,
                agreeing_indicators: c.bullish_count.max(c.bearish_count),
            })
        } else {
            None
        };

        Ok(SymbolContext {
            symbol: symbol.to_string(),
            current_price,
            price_change_percent,
            indicators,
            signals: signal_summaries,
            confluence,
        })
    }

    /// Query AI for trading decisions (with THINKING mode enabled)
    async fn query_ai_for_decisions(&self, context: &MarketContext) -> Result<Vec<ParsedDecision>> {
        let prompt = self.format_context_prompt(context);

        // Try models in priority order
        for model in &self.config.model_priority {
            let mut log_entry = AiTradeLog::new(model, &prompt);

            // Use query_with_thinking for extended reasoning before decisions
            println!("[AI Trader] Querying {} with THINKING mode enabled...", model);
            match self
                .ollama
                .query_with_thinking(&prompt, Some(AI_TRADER_SYSTEM_PROMPT), Some(model))
                .await
            {
                Ok((response, thinking)) => {
                    // Log thinking process if available
                    if let Some(ref think_content) = thinking {
                        println!("[AI Trader] Model thinking: {}...",
                            &think_content.chars().take(200).collect::<String>());
                        // Include thinking in raw response log
                        log_entry.raw_response = format!(
                            "=== THINKING ===\n{}\n\n=== RESPONSE ===\n{}",
                            think_content, response
                        );
                    } else {
                        log_entry.raw_response = response.clone();
                    }

                    if let Err(e) = log_raw_response(model, &prompt, &log_entry.raw_response, None) {
                        eprintln!("[AI Trader] WARNING: Failed to log raw response: {}", e);
                    }

                    match self.parse_ai_response(&response) {
                        Ok(parsed) => {
                            log_entry.parsed_decisions = Some(parsed.clone());

                            // Log to both individual file and daily JSONL
                            let log_file = match log_entry.save(None) {
                                Ok(path) => path.to_string_lossy().to_string(),
                                Err(e) => {
                                    eprintln!("[AI Trader] WARNING: Failed to save decision log: {}", e);
                                    "unknown".to_string()
                                }
                            };
                            if let Err(e) = log_entry.append_to_daily_log(None) {
                                eprintln!("[AI Trader] WARNING: Failed to append to daily log: {}", e);
                            }

                            // Index each decision for outcome tracking
                            for decision in &parsed.decisions {
                                if let Err(e) = index_decision(model, decision, &log_file, None) {
                                    eprintln!("[AI Trader] WARNING: Failed to index decision: {}", e);
                                }
                            }

                            println!("[AI Trader] Decision received from model: {} (thinking: {})",
                                model, if thinking.is_some() { "YES" } else { "NO" });
                            return Ok(parsed.decisions);
                        }
                        Err(e) => {
                            log_entry.error = Some(format!("Parse error: {}", e));
                            let _ = log_entry.save(None); // Log the failure too
                            eprintln!("[AI Trader] Model {} returned invalid response, trying next", model);
                        }
                    }
                }
                Err(e) => {
                    log_entry.error = Some(format!("Query error: {}", e));
                    let _ = log_entry.save(None); // Log the failure too
                    eprintln!("[AI Trader] Model {} failed: {}, trying next", model, e);
                }
            }
        }

        anyhow::bail!("All models failed to provide valid decisions")
    }

    /// Format market context into a prompt
    fn format_context_prompt(&self, context: &MarketContext) -> String {
        let mut prompt = String::new();

        prompt.push_str("=== PORTFOLIO STATUS ===\n");
        prompt.push_str(&format!("Cash: ${:.2}\n", context.portfolio.cash));
        prompt.push_str(&format!("Total Value: ${:.2}\n", context.portfolio.total_value));
        prompt.push_str(&format!(
            "P/L: ${:.2} ({:+.2}%)\n",
            context.portfolio.total_pnl, context.portfolio.total_pnl_percent
        ));

        if !context.portfolio.positions.is_empty() {
            prompt.push_str("\nPositions:\n");
            for pos in &context.portfolio.positions {
                prompt.push_str(&format!(
                    "  {} - {} shares @ ${:.2} (current: ${:.2}, P/L: ${:.2} / {:+.2}%)\n",
                    pos.symbol,
                    pos.quantity,
                    pos.entry_price,
                    pos.current_price,
                    pos.unrealized_pnl,
                    pos.unrealized_pnl_percent
                ));
            }
        }

        prompt.push_str("\n=== MARKET SIGNALS ===\n");
        for sym in &context.symbols_data {
            prompt.push_str(&format!(
                "\n{}: ${:.2} ({:+.2}%)\n",
                sym.symbol, sym.current_price, sym.price_change_percent
            ));

            if let Some(conf) = &sym.confluence {
                prompt.push_str(&format!(
                    "  Confluence: {} (strength: {:.2}, {} indicators agree)\n",
                    conf.direction, conf.strength, conf.agreeing_indicators
                ));
            }

            // Key indicators
            if let Some(rsi) = sym.indicators.get("RSI_14") {
                prompt.push_str(&format!("  RSI: {:.1}", rsi));
                if *rsi < 30.0 {
                    prompt.push_str(" (oversold)");
                } else if *rsi > 70.0 {
                    prompt.push_str(" (overbought)");
                }
                prompt.push('\n');
            }
            if let Some(adx) = sym.indicators.get("ADX_14") {
                prompt.push_str(&format!("  ADX: {:.1}", adx));
                if *adx > 25.0 {
                    prompt.push_str(" (strong trend)");
                }
                prompt.push('\n');
            }

            if !sym.signals.is_empty() {
                prompt.push_str("  Recent signals: ");
                let sig_strs: Vec<String> = sym
                    .signals
                    .iter()
                    .take(3)
                    .map(|s| format!("{} ({})", s.signal_type, s.direction))
                    .collect();
                prompt.push_str(&sig_strs.join(", "));
                prompt.push('\n');
            }
        }

        prompt.push_str("\n=== CONSTRAINTS ===\n");
        prompt.push_str(&format!(
            "Max position size: {:.0}% of portfolio\n",
            context.constraints.max_position_size_percent
        ));
        prompt.push_str(&format!(
            "Stop-loss: {:.0}%, Take-profit: {:.0}%\n",
            context.constraints.stop_loss_percent, context.constraints.take_profit_percent
        ));

        prompt.push_str(&format!(
            "\nPast prediction accuracy: {:.1}%\n",
            context.prediction_accuracy
        ));

        prompt.push_str("\nProvide your trading decisions as JSON.");

        prompt
    }

    /// Parse AI response into decisions
    fn parse_ai_response(&self, response: &str) -> Result<AiDecisionResponse> {
        // Try to extract JSON from response (sometimes models add extra text)
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        serde_json::from_str(json_str).context("Failed to parse AI response as JSON")
    }

    /// Execute a single trading decision
    async fn execute_decision(
        &self,
        db: &mut Database,
        session_id: Option<i64>,
        decision: &ParsedDecision,
        context: &MarketContext,
    ) -> Result<AiTradeDecision> {
        let (cash, _, total_value) = db.get_paper_portfolio_value()?;

        // Get current price
        let current_price = db.get_latest_price(&decision.symbol)?.unwrap_or(0.0);
        if current_price <= 0.0 {
            anyhow::bail!("No valid price for {}", decision.symbol);
        }

        let mut paper_trade_id = None;

        match decision.action.to_uppercase().as_str() {
            "BUY" => {
                // Calculate quantity based on percentage of available cash
                let max_spend = cash * (decision.quantity_percent / 100.0);
                let quantity = (max_spend / current_price).floor();

                if quantity >= 1.0 {
                    let trade = db.execute_paper_trade(
                        &decision.symbol,
                        crate::models::PaperTradeAction::Buy,
                        quantity,
                        current_price,
                        None,
                        Some(&format!("AI: {}", &decision.reasoning[..decision.reasoning.len().min(200)])),
                    )?;
                    paper_trade_id = Some(trade.id);
                    println!(
                        "[AI Trader] BUY: {} x {:.0} @ ${:.2}",
                        decision.symbol,
                        quantity,
                        current_price
                    );
                }
            }
            "SELL" => {
                // Get current position
                if let Some(pos) = db.get_paper_position(&decision.symbol)? {
                    let quantity = (pos.quantity * (decision.quantity_percent / 100.0)).floor();
                    if quantity >= 1.0 {
                        let trade = db.execute_paper_trade(
                            &decision.symbol,
                            crate::models::PaperTradeAction::Sell,
                            quantity,
                            current_price,
                            None,
                            Some(&format!("AI: {}", &decision.reasoning[..decision.reasoning.len().min(200)])),
                        )?;
                        paper_trade_id = Some(trade.id);
                        println!(
                            "[AI Trader] SELL: {} x {:.0} @ ${:.2}",
                            decision.symbol,
                            quantity,
                            current_price
                        );
                    }
                }
            }
            "HOLD" => {
                println!("[AI Trader] HOLD: {} - {}", decision.symbol, decision.reasoning);
            }
            _ => {
                eprintln!("[AI Trader] Unknown action: {}", decision.action);
            }
        }

        // Record the decision
        let ai_decision = AiTradeDecision {
            id: 0, // Will be set by DB
            session_id,
            timestamp: Utc::now().to_rfc3339(),
            action: decision.action.clone(),
            symbol: decision.symbol.clone(),
            quantity: Some(decision.quantity_percent),
            price_at_decision: Some(current_price),
            confidence: decision.confidence,
            reasoning: decision.reasoning.clone(),
            model_used: self.config.model_priority.first().cloned().unwrap_or_default(),
            predicted_direction: decision.prediction.as_ref().map(|p| p.direction.clone()),
            predicted_price_target: decision.prediction.as_ref().map(|p| p.price_target),
            predicted_timeframe_days: decision.prediction.as_ref().map(|p| p.timeframe_days),
            actual_outcome: None,
            actual_price_at_timeframe: None,
            prediction_accurate: None,
            paper_trade_id,
        };

        let decision_id = db.record_ai_decision(&ai_decision)?;

        Ok(AiTradeDecision {
            id: decision_id,
            ..ai_decision
        })
    }

    /// Record a performance snapshot
    pub fn record_performance_snapshot(&self, db: &Database) -> Result<i64> {
        let (cash, positions_value, total_value) = db.get_paper_portfolio_value()?;
        let trades = db.get_paper_trades(None, 10000)?;

        let starting_capital = self.config.starting_capital;
        let total_pnl = total_value - starting_capital;
        let total_pnl_percent = (total_pnl / starting_capital) * 100.0;

        // Get benchmark (SPY) value
        let benchmark_value = self.get_benchmark_value(db, starting_capital)?;
        let benchmark_start = starting_capital;
        let benchmark_pnl_percent = ((benchmark_value - benchmark_start) / benchmark_start) * 100.0;

        // Count wins/losses
        let winning = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) > 0.0).count() as i32;
        let losing = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) < 0.0).count() as i32;
        let total_closed = winning + losing;
        let win_rate = if total_closed > 0 {
            Some((winning as f64 / total_closed as f64) * 100.0)
        } else {
            None
        };

        let accuracy = db.get_ai_prediction_accuracy()?;

        let snapshot = AiPerformanceSnapshot {
            id: 0,
            timestamp: Utc::now().to_rfc3339(),
            portfolio_value: total_value,
            cash,
            positions_value,
            benchmark_value,
            benchmark_symbol: self.config.benchmark_symbol.clone(),
            total_pnl,
            total_pnl_percent,
            benchmark_pnl_percent,
            prediction_accuracy: if accuracy.total_predictions > 0 {
                Some(accuracy.accuracy_percent)
            } else {
                None
            },
            trades_to_date: trades.len() as i32,
            winning_trades: winning,
            losing_trades: losing,
            win_rate,
        };

        Ok(db.record_ai_performance_snapshot(&snapshot)?)
    }

    /// Get benchmark value (normalized to starting capital)
    fn get_benchmark_value(&self, db: &Database, starting_capital: f64) -> Result<f64> {
        let first_snapshot = db.get_first_ai_snapshot()?;

        if let Some(first) = first_snapshot {
            // We have history - calculate based on SPY price change
            let spy_prices = db.get_prices(&self.config.benchmark_symbol)?;
            if spy_prices.len() >= 2 {
                let current = spy_prices.last().unwrap().close;
                // Find price closest to first snapshot date
                let start_price = spy_prices.first().unwrap().close;
                let change_ratio = current / start_price;
                return Ok(first.benchmark_value * change_ratio);
            }
        }

        // No history yet, benchmark equals starting capital
        Ok(starting_capital)
    }

    /// Calculate benchmark comparison
    pub fn get_benchmark_comparison(&self, db: &Database) -> Result<BenchmarkComparison> {
        let snapshots = db.get_ai_performance_snapshots(365)?;

        if snapshots.is_empty() {
            return Ok(BenchmarkComparison::default());
        }

        let first = &snapshots[0];
        let last = snapshots.last().unwrap();

        let portfolio_return = ((last.portfolio_value - first.portfolio_value)
            / first.portfolio_value)
            * 100.0;
        let benchmark_return =
            ((last.benchmark_value - first.benchmark_value) / first.benchmark_value) * 100.0;
        let alpha = portfolio_return - benchmark_return;

        let tracking_data: Vec<(String, f64, f64)> = snapshots
            .iter()
            .map(|s| (s.timestamp.clone(), s.portfolio_value, s.benchmark_value))
            .collect();

        Ok(BenchmarkComparison {
            portfolio_return_percent: portfolio_return,
            benchmark_return_percent: benchmark_return,
            alpha,
            tracking_data,
        })
    }

    /// Calculate compounding forecast
    pub fn get_compounding_forecast(&self, db: &Database) -> Result<CompoundingForecast> {
        let snapshots = db.get_ai_performance_snapshots(30)?;

        if snapshots.len() < 2 {
            return Ok(CompoundingForecast::insufficient_data());
        }

        // Calculate daily returns
        let daily_returns: Vec<f64> = snapshots
            .windows(2)
            .map(|w| (w[1].portfolio_value - w[0].portfolio_value) / w[0].portfolio_value)
            .collect();

        let avg_daily_return =
            daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;

        let (_, _, current_value) = db.get_paper_portfolio_value()?;

        // Win rate
        let trades = db.get_paper_trades(None, 10000)?;
        let winning = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) > 0.0).count();
        let losing = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) < 0.0).count();
        let win_rate = if winning + losing > 0 {
            (winning as f64 / (winning + losing) as f64) * 100.0
        } else {
            0.0
        };

        // Projections
        let projected_30 = current_value * (1.0 + avg_daily_return).powi(30);
        let projected_90 = current_value * (1.0 + avg_daily_return).powi(90);
        let projected_365 = current_value * (1.0 + avg_daily_return).powi(365);

        // Time to double
        let time_to_double = if avg_daily_return > 0.0 {
            Some((2.0_f64.ln() / (1.0 + avg_daily_return).ln()).ceil() as u32)
        } else {
            None
        };

        // Time to bankruptcy
        let time_to_bankruptcy = if avg_daily_return < 0.0 {
            let threshold = BANKRUPTCY_THRESHOLD;
            let days = ((threshold / current_value).ln() / (1.0 + avg_daily_return).ln()).ceil();
            Some(days as u32)
        } else {
            None
        };

        Ok(CompoundingForecast {
            current_daily_return: avg_daily_return * 100.0,
            current_win_rate: win_rate,
            projected_30_days: projected_30,
            projected_90_days: projected_90,
            projected_365_days: projected_365,
            time_to_double,
            time_to_bankruptcy,
        })
    }

    /// Evaluate past predictions that have reached their timeframe
    pub fn evaluate_predictions(&self, db: &mut Database) -> Result<u32> {
        let pending = db.get_unevaluated_ai_predictions()?;
        let mut evaluated = 0;

        for decision in pending {
            if let (Some(target), Some(direction), Some(price_at)) = (
                decision.predicted_price_target,
                &decision.predicted_direction,
                decision.price_at_decision,
            ) {
                // Get current price
                if let Ok(Some(current)) = db.get_latest_price(&decision.symbol) {
                    let was_accurate = match direction.as_str() {
                        "bullish" => current > price_at,
                        "bearish" => current < price_at,
                        _ => false,
                    };

                    let outcome = format!(
                        "{:.2} -> {:.2} (predicted: {:.2})",
                        price_at, current, target
                    );

                    db.update_ai_prediction_outcome(decision.id, &outcome, current, was_accurate)?;
                    evaluated += 1;
                }
            }
        }

        Ok(evaluated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_trader_creation() {
        let trader = AiTrader::with_defaults();
        assert_eq!(trader.config.starting_capital, 1_000_000.0);
        assert_eq!(trader.config.max_position_size_percent, 10.0);
    }

    #[test]
    fn test_parse_ai_response() {
        let trader = AiTrader::with_defaults();
        let response = r#"{
            "decisions": [{
                "action": "BUY",
                "symbol": "AAPL",
                "quantity_percent": 8.0,
                "confidence": 0.75,
                "reasoning": "Test reasoning",
                "prediction": {
                    "direction": "bullish",
                    "price_target": 180.0,
                    "timeframe_days": 5
                }
            }],
            "market_outlook": "Test outlook"
        }"#;

        let parsed = trader.parse_ai_response(response).unwrap();
        assert_eq!(parsed.decisions.len(), 1);
        assert_eq!(parsed.decisions[0].symbol, "AAPL");
        assert_eq!(parsed.decisions[0].action, "BUY");
    }
}
