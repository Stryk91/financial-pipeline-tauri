//! Tauri GUI backend for Financial Pipeline

use financial_pipeline::{
    calculate_all, AlertCondition, BacktestConfig, BacktestEngine, Database, Fred, GoogleTrends,
    IndicatorAlert, IndicatorAlertCondition, IndicatorAlertType, PositionType, SignalEngine,
    Strategy, StrategyConditionType, YahooFinance,
    VectorStore, MarketEvent, PricePattern,
    ClaudeClient, FinancialContext, PriceContext as ClaudePriceContext,
    FinnhubClient, SimpleNewsItem, PriceReaction,
    PaperWallet, PaperPosition, PaperTrade, PaperTradeAction,
    AiTrader, AiTraderConfig, AiTradingSession, AiTradeDecision, AiPerformanceSnapshot,
    AiPredictionAccuracy, AiTraderStatus, BenchmarkComparison, CompoundingForecast,
    // DC Trader types
    DcWallet, DcPosition, DcTrade, PortfolioSnapshot, TeamConfig, ImportResult, CompetitionStats,
};
use financial_pipeline::ollama::{OllamaClient, SentimentResult, PatternExplanation};
use chrono::Utc;
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

/// Get the absolute path to a data file
/// Uses FP_DATA_DIR env var if set, otherwise defaults to absolute path for development
fn get_data_path(filename: &str) -> String {
    let base = std::env::var("FP_DATA_DIR")
        .unwrap_or_else(|_| {
            // Default to absolute Windows path for the app (runs as Windows exe)
            // Note: Use Windows path format (X:\...) not WSL format (/mnt/x/...)
            r"X:\dev\financial-pipeline-rs\data".to_string()
        });
    format!("{}\\{}", base, filename)
}

/// Application state holding the database connection
struct AppState {
    db: Mutex<Database>,
}

/// Symbol with latest price and percent change
#[derive(Serialize)]
struct SymbolPrice {
    symbol: String,
    price: f64,
    change_percent: f64,
    change_direction: String, // "up", "down", or "unchanged"
    favorited: bool,          // moon icon for auto-refresh
}

/// Command result
#[derive(Serialize)]
struct CommandResult {
    success: bool,
    message: String,
}

/// Indicator data for frontend
#[derive(Serialize)]
struct IndicatorData {
    name: String,
    value: f64,
    date: String,
}

/// Macro data for frontend
#[derive(Serialize)]
struct MacroDataResponse {
    indicator: String,
    value: f64,
    date: String,
    source: String,
}

/// Get all symbols with their latest prices and percent change
#[tauri::command]
fn get_symbols(state: State<AppState>) -> Result<Vec<SymbolPrice>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let symbols = db.get_symbols_with_data().map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for symbol in symbols {
        // Check if favorited
        let favorited = db.is_symbol_favorited(&symbol).unwrap_or(false);

        // Get price history to calculate percent change
        if let Ok(prices) = db.get_prices(&symbol) {
            if prices.len() >= 2 {
                let current = prices.last().unwrap();
                let previous = &prices[prices.len() - 2];

                let change_percent = if previous.close > 0.0 {
                    ((current.close - previous.close) / previous.close) * 100.0
                } else {
                    0.0
                };

                let change_direction = if change_percent > 0.001 {
                    "up".to_string()
                } else if change_percent < -0.001 {
                    "down".to_string()
                } else {
                    "unchanged".to_string()
                };

                result.push(SymbolPrice {
                    symbol,
                    price: current.close,
                    change_percent,
                    change_direction,
                    favorited,
                });
            } else if let Some(price) = prices.last() {
                result.push(SymbolPrice {
                    symbol,
                    price: price.close,
                    change_percent: 0.0,
                    change_direction: "unchanged".to_string(),
                    favorited,
                });
            }
        }
    }

    Ok(result)
}

/// Toggle symbol favorite status (moon icon)
#[tauri::command]
fn toggle_favorite(state: State<AppState>, symbol: String) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.toggle_symbol_favorite(&symbol).map_err(|e| e.to_string())
}

/// Get all favorited symbols
#[tauri::command]
fn get_favorited_symbols(state: State<AppState>) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_favorited_symbols().map_err(|e| e.to_string())
}

/// Favorite all DC position symbols for auto-refresh
#[tauri::command]
fn favorite_dc_positions(state: State<AppState>) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbols = db.favorite_dc_positions().map_err(|e| e.to_string())?;
    Ok(CommandResult {
        success: true,
        message: format!("Added {} DC symbols to auto-refresh: {}", symbols.len(), symbols.join(", ")),
    })
}

/// Favorite all KALIC position symbols for auto-refresh
#[tauri::command]
fn favorite_paper_positions(state: State<AppState>) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbols = db.favorite_paper_positions().map_err(|e| e.to_string())?;
    Ok(CommandResult {
        success: true,
        message: format!("Added {} KALIC symbols to auto-refresh: {}", symbols.len(), symbols.join(", ")),
    })
}

/// Fetch stock prices from Yahoo Finance
#[tauri::command]
fn fetch_prices(
    state: State<AppState>,
    symbols: String,
    period: String,
) -> Result<CommandResult, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;

    let symbol_list: Vec<String> = symbols
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();

    if symbol_list.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: "No symbols provided".to_string(),
        });
    }

    let yahoo = YahooFinance::new();

    let mut success_count = 0;
    let mut fail_count = 0;

    for symbol in &symbol_list {
        match yahoo.fetch_and_store(&mut db, symbol, &period) {
            Ok(_) => success_count += 1,
            Err(_) => fail_count += 1,
        }
    }

    Ok(CommandResult {
        success: fail_count == 0,
        message: format!(
            "Fetched {} symbols ({} success, {} failed)",
            symbol_list.len(),
            success_count,
            fail_count
        ),
    })
}

/// Fetch FRED macro data
#[tauri::command]
fn fetch_fred(state: State<AppState>, indicators: String) -> Result<CommandResult, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;

    let indicator_list: Vec<&str> = indicators
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if indicator_list.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: "No indicators provided".to_string(),
        });
    }

    let fred = Fred::new();

    let mut success_count = 0;
    let mut fail_count = 0;

    for indicator in &indicator_list {
        match fred.fetch_and_store(&mut db, indicator) {
            Ok(_) => success_count += 1,
            Err(_) => fail_count += 1,
        }
    }

    Ok(CommandResult {
        success: fail_count == 0,
        message: format!(
            "Fetched {} indicators ({} success, {} failed)",
            indicator_list.len(),
            success_count,
            fail_count
        ),
    })
}

/// Get macro data summary (latest value for each indicator)
#[tauri::command]
fn get_macro_data(state: State<AppState>) -> Result<Vec<MacroDataResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let data = db.get_macro_summary().map_err(|e| e.to_string())?;

    Ok(data
        .into_iter()
        .map(|d| MacroDataResponse {
            indicator: d.indicator,
            value: d.value,
            date: d.date.to_string(),
            source: d.source,
        })
        .collect())
}

/// Get price for a single symbol
#[tauri::command]
fn get_price(state: State<AppState>, symbol: String) -> Result<Option<f64>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_latest_price(&symbol.to_uppercase())
        .map_err(|e| e.to_string())
}

/// Calculate indicators for a symbol
#[tauri::command]
fn calculate_indicators(state: State<AppState>, symbol: String) -> Result<CommandResult, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    // Get price history
    let prices = db.get_prices(&symbol).map_err(|e| e.to_string())?;

    if prices.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: format!("No price data for {}", symbol),
        });
    }

    // Calculate all indicators
    let indicators = calculate_all(&prices);
    let count = indicators.len();

    // Store them
    db.upsert_indicators(&indicators)
        .map_err(|e| e.to_string())?;

    println!("[OK] Calculated {} indicator values for {}", count, symbol);

    Ok(CommandResult {
        success: true,
        message: format!("Calculated {} indicator values for {}", count, symbol),
    })
}

/// Get latest indicators for a symbol
#[tauri::command]
fn get_indicators(state: State<AppState>, symbol: String) -> Result<Vec<IndicatorData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let indicators = db
        .get_latest_indicators(&symbol)
        .map_err(|e| e.to_string())?;

    Ok(indicators
        .into_iter()
        .map(|i| IndicatorData {
            name: i.indicator_name,
            value: i.value,
            date: i.date.to_string(),
        })
        .collect())
}

/// Get indicator history for charting
#[tauri::command]
fn get_indicator_history(
    state: State<AppState>,
    symbol: String,
    indicator_name: String,
) -> Result<Vec<IndicatorData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let indicators = db
        .get_indicator_history(&symbol, &indicator_name)
        .map_err(|e| e.to_string())?;

    Ok(indicators
        .into_iter()
        .map(|i| IndicatorData {
            name: i.indicator_name,
            value: i.value,
            date: i.date.to_string(),
        })
        .collect())
}

/// Price point for charting
#[derive(Serialize)]
struct PricePoint {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: i64,
}

/// Get price history for charting
#[tauri::command]
fn get_price_history(state: State<AppState>, symbol: String) -> Result<Vec<PricePoint>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let prices = db.get_prices(&symbol).map_err(|e| e.to_string())?;

    Ok(prices
        .into_iter()
        .map(|p| PricePoint {
            date: p.date.to_string(),
            open: p.open,
            high: p.high,
            low: p.low,
            close: p.close,
            volume: p.volume,
        })
        .collect())
}

/// Export data to CSV
#[tauri::command]
fn export_csv(state: State<AppState>, symbol: String) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    // Get price data
    let prices = db.get_prices(&symbol).map_err(|e| e.to_string())?;
    if prices.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: format!("No data for {}", symbol),
        });
    }

    // Get indicators
    let indicators = db.get_latest_indicators(&symbol).map_err(|e| e.to_string())?;

    // Create export directory
    std::fs::create_dir_all("exports").ok();

    // Export prices
    let price_file = format!("exports/{}_prices.csv", symbol);
    let mut wtr = std::fs::File::create(&price_file).map_err(|e| e.to_string())?;
    use std::io::Write;
    writeln!(wtr, "date,open,high,low,close,volume").map_err(|e| e.to_string())?;
    for p in &prices {
        writeln!(wtr, "{},{},{},{},{},{}", p.date, p.open, p.high, p.low, p.close, p.volume)
            .map_err(|e| e.to_string())?;
    }

    // Export indicators
    let ind_file = format!("exports/{}_indicators.csv", symbol);
    let mut wtr = std::fs::File::create(&ind_file).map_err(|e| e.to_string())?;
    writeln!(wtr, "indicator,value,date").map_err(|e| e.to_string())?;
    for i in &indicators {
        writeln!(wtr, "{},{},{}", i.indicator_name, i.value, i.date).map_err(|e| e.to_string())?;
    }

    println!("[OK] Exported {} to CSV", symbol);

    Ok(CommandResult {
        success: true,
        message: format!("Exported to exports/{}_prices.csv and exports/{}_indicators.csv", symbol, symbol),
    })
}

/// Company name to symbol mapping for fuzzy search
fn get_symbol_mapping() -> std::collections::HashMap<&'static str, &'static str> {
    let mut map = std::collections::HashMap::new();
    // Tech
    map.insert("apple", "AAPL");
    map.insert("microsoft", "MSFT");
    map.insert("google", "GOOGL");
    map.insert("alphabet", "GOOGL");
    map.insert("amazon", "AMZN");
    map.insert("meta", "META");
    map.insert("facebook", "META");
    map.insert("nvidia", "NVDA");
    map.insert("tesla", "TSLA");
    map.insert("netflix", "NFLX");
    map.insert("intel", "INTC");
    map.insert("amd", "AMD");
    map.insert("cisco", "CSCO");
    map.insert("oracle", "ORCL");
    map.insert("ibm", "IBM");
    map.insert("salesforce", "CRM");
    map.insert("adobe", "ADBE");
    map.insert("paypal", "PYPL");
    map.insert("uber", "UBER");
    map.insert("airbnb", "ABNB");
    map.insert("spotify", "SPOT");
    map.insert("snap", "SNAP");
    map.insert("snapchat", "SNAP");
    map.insert("twitter", "X");
    map.insert("palantir", "PLTR");
    // Finance
    map.insert("jpmorgan", "JPM");
    map.insert("jp morgan", "JPM");
    map.insert("goldman", "GS");
    map.insert("goldman sachs", "GS");
    map.insert("morgan stanley", "MS");
    map.insert("bank of america", "BAC");
    map.insert("wells fargo", "WFC");
    map.insert("visa", "V");
    map.insert("mastercard", "MA");
    map.insert("berkshire", "BRK.B");
    // Retail/Consumer
    map.insert("walmart", "WMT");
    map.insert("costco", "COST");
    map.insert("target", "TGT");
    map.insert("home depot", "HD");
    map.insert("lowes", "LOW");
    map.insert("nike", "NKE");
    map.insert("starbucks", "SBUX");
    map.insert("mcdonalds", "MCD");
    map.insert("coca cola", "KO");
    map.insert("coke", "KO");
    map.insert("pepsi", "PEP");
    map.insert("disney", "DIS");
    // Healthcare
    map.insert("johnson", "JNJ");
    map.insert("pfizer", "PFE");
    map.insert("moderna", "MRNA");
    map.insert("unitedhealth", "UNH");
    // Energy
    map.insert("exxon", "XOM");
    map.insert("chevron", "CVX");
    // ETFs
    map.insert("s&p", "SPY");
    map.insert("s&p 500", "SPY");
    map.insert("spy", "SPY");
    map.insert("nasdaq", "QQQ");
    map.insert("qqq", "QQQ");
    map.insert("dow", "DIA");
    map.insert("dow jones", "DIA");
    map
}

/// Search for symbol by name (fuzzy match)
#[tauri::command]
fn search_symbol(query: String) -> Result<Vec<String>, String> {
    let query = query.to_lowercase();
    let mapping = get_symbol_mapping();

    let mut results = Vec::new();

    // Direct match first
    if let Some(symbol) = mapping.get(query.as_str()) {
        results.push(symbol.to_string());
    }

    // Partial match
    for (name, symbol) in &mapping {
        if name.contains(&query) || query.contains(name) {
            if !results.contains(&symbol.to_string()) {
                results.push(symbol.to_string());
            }
        }
    }

    // If query looks like a symbol, add it directly
    if query.len() <= 5 && query.chars().all(|c| c.is_alphabetic()) {
        let upper = query.to_uppercase();
        if !results.contains(&upper) {
            results.push(upper);
        }
    }

    Ok(results)
}

/// Alert data for frontend
#[derive(Serialize)]
struct AlertData {
    id: i64,
    symbol: String,
    target_price: f64,
    condition: String,
    triggered: bool,
    created_at: String,
}

/// Add a price alert
#[tauri::command]
fn add_alert(
    state: State<AppState>,
    symbol: String,
    target_price: f64,
    condition: String,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let alert_condition = match condition.to_lowercase().as_str() {
        "above" => AlertCondition::Above,
        "below" => AlertCondition::Below,
        _ => return Err("Invalid condition. Use 'above' or 'below'".to_string()),
    };

    db.add_alert(&symbol, target_price, alert_condition)
        .map_err(|e| e.to_string())?;

    println!("[OK] Added alert for {} {} ${:.2}", symbol, condition, target_price);

    Ok(CommandResult {
        success: true,
        message: format!("Alert set: {} {} ${:.2}", symbol, condition, target_price),
    })
}

/// Get all alerts
#[tauri::command]
fn get_alerts(state: State<AppState>, only_active: bool) -> Result<Vec<AlertData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let alerts = db.get_alerts(only_active).map_err(|e| e.to_string())?;

    Ok(alerts
        .into_iter()
        .map(|a| AlertData {
            id: a.id,
            symbol: a.symbol,
            target_price: a.target_price,
            condition: match a.condition {
                AlertCondition::Above => "above".to_string(),
                AlertCondition::Below => "below".to_string(),
            },
            triggered: a.triggered,
            created_at: a.created_at,
        })
        .collect())
}

/// Delete an alert
#[tauri::command]
fn delete_alert(state: State<AppState>, alert_id: i64) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.delete_alert(alert_id).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: "Alert deleted".to_string(),
    })
}

/// Check alerts against current prices
#[tauri::command]
fn check_alerts(state: State<AppState>) -> Result<Vec<AlertData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let triggered = db.check_alerts().map_err(|e| e.to_string())?;

    Ok(triggered
        .into_iter()
        .map(|a| AlertData {
            id: a.id,
            symbol: a.symbol,
            target_price: a.target_price,
            condition: match a.condition {
                AlertCondition::Above => "above".to_string(),
                AlertCondition::Below => "below".to_string(),
            },
            triggered: a.triggered,
            created_at: a.created_at,
        })
        .collect())
}

/// Position data for frontend
#[derive(Serialize)]
struct PositionData {
    id: i64,
    symbol: String,
    quantity: f64,
    price: f64,
    position_type: String,
    date: String,
    notes: Option<String>,
    current_price: f64,
    current_value: f64,
    cost_basis: f64,
    profit_loss: f64,
    profit_loss_percent: f64,
}

/// Portfolio summary for frontend
#[derive(Serialize)]
struct PortfolioSummary {
    positions: Vec<PositionData>,
    total_value: f64,
    total_cost: f64,
    total_profit_loss: f64,
    total_profit_loss_percent: f64,
}

/// Add a portfolio position
#[tauri::command]
fn add_position(
    state: State<AppState>,
    symbol: String,
    quantity: f64,
    price: f64,
    position_type: String,
    date: String,
    notes: Option<String>,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let pos_type = match position_type.to_lowercase().as_str() {
        "buy" => PositionType::Buy,
        "sell" => PositionType::Sell,
        _ => return Err("Invalid position type. Use 'buy' or 'sell'".to_string()),
    };

    db.add_position(&symbol, quantity, price, pos_type, &date, notes.as_deref())
        .map_err(|e| e.to_string())?;

    println!(
        "[OK] Added {} position: {} x {} @ ${:.2}",
        position_type, quantity, symbol, price
    );

    Ok(CommandResult {
        success: true,
        message: format!(
            "Added {} {} shares of {} @ ${:.2}",
            position_type, quantity, symbol, price
        ),
    })
}

/// Get portfolio with current values and P&L
#[tauri::command]
fn get_portfolio(state: State<AppState>) -> Result<PortfolioSummary, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let positions = db.get_positions().map_err(|e| e.to_string())?;

    let mut position_data = Vec::new();
    let mut total_value = 0.0;
    let mut total_cost = 0.0;

    for pos in positions {
        let current_price = db
            .get_latest_price(&pos.symbol)
            .map_err(|e| e.to_string())?
            .unwrap_or(pos.price);

        let cost_basis = pos.quantity * pos.price;
        let current_value = pos.quantity * current_price;

        // For sell positions, P&L is inverted (profit when price drops)
        let (profit_loss, profit_loss_percent) = match pos.position_type {
            PositionType::Buy => {
                let pl = current_value - cost_basis;
                let pl_pct = if cost_basis > 0.0 {
                    (pl / cost_basis) * 100.0
                } else {
                    0.0
                };
                total_value += current_value;
                total_cost += cost_basis;
                (pl, pl_pct)
            }
            PositionType::Sell => {
                // Short position: profit when price goes down
                let pl = cost_basis - current_value;
                let pl_pct = if cost_basis > 0.0 {
                    (pl / cost_basis) * 100.0
                } else {
                    0.0
                };
                // For shorts, we track the liability
                total_value -= current_value;
                total_cost -= cost_basis;
                (pl, pl_pct)
            }
        };

        position_data.push(PositionData {
            id: pos.id,
            symbol: pos.symbol,
            quantity: pos.quantity,
            price: pos.price,
            position_type: match pos.position_type {
                PositionType::Buy => "buy".to_string(),
                PositionType::Sell => "sell".to_string(),
            },
            date: pos.date,
            notes: pos.notes,
            current_price,
            current_value,
            cost_basis,
            profit_loss,
            profit_loss_percent,
        });
    }

    let total_profit_loss = total_value - total_cost;
    let total_profit_loss_percent = if total_cost.abs() > 0.0 {
        (total_profit_loss / total_cost.abs()) * 100.0
    } else {
        0.0
    };

    Ok(PortfolioSummary {
        positions: position_data,
        total_value,
        total_cost,
        total_profit_loss,
        total_profit_loss_percent,
    })
}

/// Delete a portfolio position
#[tauri::command]
fn delete_position(state: State<AppState>, position_id: i64) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.delete_position(position_id).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: "Position deleted".to_string(),
    })
}

/// Trend data point for frontend
#[derive(Serialize)]
struct TrendPoint {
    date: String,
    value: i32,
}

/// Fetch Google Trends data for a keyword
#[tauri::command]
fn fetch_trends(state: State<AppState>, keyword: String) -> Result<CommandResult, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;

    let trends = GoogleTrends::new();

    match trends.fetch_and_store(&mut db, &keyword) {
        Ok(count) => {
            println!("[OK] Fetched {} trend points for {}", count, keyword);
            Ok(CommandResult {
                success: true,
                message: format!("Fetched {} trend data points for {}", count, keyword),
            })
        }
        Err(e) => {
            println!("[ERR] Failed to fetch trends for {}: {}", keyword, e);
            Ok(CommandResult {
                success: false,
                message: format!("Failed to fetch trends: {}", e),
            })
        }
    }
}

/// Get stored trends data for a keyword
#[tauri::command]
fn get_trends(state: State<AppState>, keyword: String) -> Result<Vec<TrendPoint>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let trends = db.get_trends(&keyword).map_err(|e| e.to_string())?;

    Ok(trends
        .into_iter()
        .map(|t| TrendPoint {
            date: t.date.to_string(),
            value: t.value,
        })
        .collect())
}

// ============================================================================
// Signal Commands
// ============================================================================

/// Signal data for frontend
#[derive(Serialize)]
struct SignalData {
    id: i64,
    symbol: String,
    signal_type: String,
    direction: String,
    strength: f64,
    price_at_signal: f64,
    triggered_by: String,
    trigger_value: f64,
    timestamp: String,
    created_at: String,
    acknowledged: bool,
}

/// Generate signals for a symbol
#[tauri::command]
fn generate_signals(state: State<AppState>, symbol: String) -> Result<CommandResult, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    // Get prices and indicators
    let prices = db.get_prices(&symbol).map_err(|e| e.to_string())?;
    let indicators = db.get_all_indicators(&symbol).map_err(|e| e.to_string())?;

    if prices.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: format!("No price data for {}", symbol),
        });
    }

    if indicators.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: format!("No indicator data for {}. Calculate indicators first.", symbol),
        });
    }

    // Generate signals
    let engine = SignalEngine::new();
    let signals = engine.generate_signals(&symbol, &indicators, &prices);
    let count = signals.len();

    // Store signals
    db.upsert_signals(&signals).map_err(|e| e.to_string())?;

    println!("[OK] Generated {} signals for {}", count, symbol);

    Ok(CommandResult {
        success: true,
        message: format!("Generated {} signals for {}", count, symbol),
    })
}

/// Get signals for a symbol
#[tauri::command]
fn get_signals(
    state: State<AppState>,
    symbol: String,
    only_unacknowledged: bool,
) -> Result<Vec<SignalData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let signals = db
        .get_signals(&symbol, only_unacknowledged)
        .map_err(|e| e.to_string())?;

    Ok(signals
        .into_iter()
        .map(|s| SignalData {
            id: s.id,
            symbol: s.symbol,
            signal_type: s.signal_type.as_str().to_string(),
            direction: s.direction.as_str().to_string(),
            strength: s.strength,
            price_at_signal: s.price_at_signal,
            triggered_by: s.triggered_by,
            trigger_value: s.trigger_value,
            timestamp: s.timestamp.to_string(),
            created_at: s.created_at,
            acknowledged: s.acknowledged,
        })
        .collect())
}

/// Get all recent signals across all symbols
#[tauri::command]
fn get_all_signals(state: State<AppState>, limit: usize) -> Result<Vec<SignalData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let signals = db.get_recent_signals(limit).map_err(|e| e.to_string())?;

    Ok(signals
        .into_iter()
        .map(|s| SignalData {
            id: s.id,
            symbol: s.symbol,
            signal_type: s.signal_type.as_str().to_string(),
            direction: s.direction.as_str().to_string(),
            strength: s.strength,
            price_at_signal: s.price_at_signal,
            triggered_by: s.triggered_by,
            trigger_value: s.trigger_value,
            timestamp: s.timestamp.to_string(),
            created_at: s.created_at,
            acknowledged: s.acknowledged,
        })
        .collect())
}

/// Acknowledge a signal
#[tauri::command]
fn acknowledge_signal(state: State<AppState>, signal_id: i64) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.acknowledge_signal(signal_id)
        .map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: "Signal acknowledged".to_string(),
    })
}

/// Acknowledge all signals for a symbol
#[tauri::command]
fn acknowledge_all_signals(
    state: State<AppState>,
    symbol: String,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    db.acknowledge_all_signals(&symbol)
        .map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: format!("All signals for {} acknowledged", symbol),
    })
}

// ============================================================================
// Indicator Alert Commands
// ============================================================================

/// Indicator alert data for frontend
#[derive(Serialize)]
struct IndicatorAlertData {
    id: i64,
    symbol: String,
    alert_type: String,
    indicator_name: String,
    secondary_indicator: Option<String>,
    condition: String,
    threshold: Option<f64>,
    triggered: bool,
    last_value: Option<f64>,
    created_at: String,
    message: Option<String>,
}

/// Add an indicator alert
#[tauri::command]
fn add_indicator_alert(
    state: State<AppState>,
    symbol: String,
    alert_type: String,
    indicator_name: String,
    secondary_indicator: Option<String>,
    condition: String,
    threshold: Option<f64>,
    message: Option<String>,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let alert_type_enum = IndicatorAlertType::from_str(&alert_type)
        .ok_or_else(|| "Invalid alert type. Use 'threshold', 'crossover', or 'band_touch'".to_string())?;

    let condition_enum = IndicatorAlertCondition::from_str(&condition)
        .ok_or_else(|| "Invalid condition. Use 'crosses_above', 'crosses_below', 'bullish_crossover', or 'bearish_crossover'".to_string())?;

    let alert = IndicatorAlert {
        id: 0,
        symbol: symbol.clone(),
        alert_type: alert_type_enum,
        indicator_name: indicator_name.clone(),
        secondary_indicator,
        condition: condition_enum,
        threshold,
        triggered: false,
        last_value: None,
        created_at: String::new(),
        message,
    };

    db.add_indicator_alert(&alert).map_err(|e| e.to_string())?;

    println!(
        "[OK] Added indicator alert for {} {} {} {}",
        symbol, indicator_name, condition, threshold.map(|t| format!("{}", t)).unwrap_or_default()
    );

    Ok(CommandResult {
        success: true,
        message: format!(
            "Indicator alert set: {} {} {} {}",
            symbol, indicator_name, condition, threshold.map(|t| format!("{}", t)).unwrap_or_default()
        ),
    })
}

/// Get all indicator alerts
#[tauri::command]
fn get_indicator_alerts(
    state: State<AppState>,
    only_active: bool,
) -> Result<Vec<IndicatorAlertData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let alerts = db.get_indicator_alerts(only_active).map_err(|e| e.to_string())?;

    Ok(alerts
        .into_iter()
        .map(|a| IndicatorAlertData {
            id: a.id,
            symbol: a.symbol,
            alert_type: a.alert_type.as_str().to_string(),
            indicator_name: a.indicator_name,
            secondary_indicator: a.secondary_indicator,
            condition: a.condition.as_str().to_string(),
            threshold: a.threshold,
            triggered: a.triggered,
            last_value: a.last_value,
            created_at: a.created_at,
            message: a.message,
        })
        .collect())
}

/// Delete an indicator alert
#[tauri::command]
fn delete_indicator_alert(
    state: State<AppState>,
    alert_id: i64,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.delete_indicator_alert(alert_id).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: "Indicator alert deleted".to_string(),
    })
}

/// Check all indicator alerts, returns triggered alerts
#[tauri::command]
fn check_indicator_alerts(state: State<AppState>) -> Result<Vec<IndicatorAlertData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let triggered = db.check_indicator_alerts().map_err(|e| e.to_string())?;

    Ok(triggered
        .into_iter()
        .map(|a| IndicatorAlertData {
            id: a.id,
            symbol: a.symbol,
            alert_type: a.alert_type.as_str().to_string(),
            indicator_name: a.indicator_name,
            secondary_indicator: a.secondary_indicator,
            condition: a.condition.as_str().to_string(),
            threshold: a.threshold,
            triggered: a.triggered,
            last_value: a.last_value,
            created_at: a.created_at,
            message: a.message,
        })
        .collect())
}

// ============================================================================
// Backtest Commands
// ============================================================================

/// Strategy data for frontend
#[derive(Serialize)]
struct StrategyData {
    id: i64,
    name: String,
    description: Option<String>,
    entry_condition: String,
    entry_threshold: f64,
    exit_condition: String,
    exit_threshold: f64,
    stop_loss_percent: Option<f64>,
    take_profit_percent: Option<f64>,
    position_size_percent: f64,
    created_at: String,
}

/// Backtest trade data for frontend
#[derive(Serialize)]
struct BacktestTradeData {
    id: i64,
    symbol: String,
    direction: String,
    entry_date: String,
    entry_price: f64,
    entry_reason: String,
    exit_date: Option<String>,
    exit_price: Option<f64>,
    exit_reason: Option<String>,
    shares: f64,
    profit_loss: Option<f64>,
    profit_loss_percent: Option<f64>,
}

/// Performance metrics for frontend
#[derive(Serialize)]
struct MetricsData {
    total_return: f64,
    total_return_dollars: f64,
    max_drawdown: f64,
    sharpe_ratio: f64,
    win_rate: f64,
    total_trades: usize,
    winning_trades: usize,
    losing_trades: usize,
    avg_win_percent: f64,
    avg_loss_percent: f64,
    profit_factor: f64,
    avg_trade_duration_days: f64,
}

/// Backtest result data for frontend
#[derive(Serialize)]
struct BacktestResultData {
    id: i64,
    strategy_id: i64,
    strategy_name: String,
    symbol: String,
    start_date: String,
    end_date: String,
    initial_capital: f64,
    final_capital: f64,
    metrics: MetricsData,
    trades: Vec<BacktestTradeData>,
    created_at: String,
}

/// Save a strategy
#[tauri::command]
fn save_strategy(
    state: State<AppState>,
    name: String,
    description: Option<String>,
    entry_condition: String,
    entry_threshold: f64,
    exit_condition: String,
    exit_threshold: f64,
    stop_loss_percent: Option<f64>,
    take_profit_percent: Option<f64>,
    position_size_percent: f64,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let entry_cond = StrategyConditionType::from_str(&entry_condition)
        .ok_or_else(|| format!("Invalid entry condition: {}", entry_condition))?;
    let exit_cond = StrategyConditionType::from_str(&exit_condition)
        .ok_or_else(|| format!("Invalid exit condition: {}", exit_condition))?;

    let strategy = Strategy {
        id: 0,
        name: name.clone(),
        description,
        entry_condition: entry_cond,
        entry_threshold,
        exit_condition: exit_cond,
        exit_threshold,
        stop_loss_percent,
        take_profit_percent,
        position_size_percent,
        created_at: String::new(),
    };

    db.save_strategy(&strategy).map_err(|e| e.to_string())?;

    println!("[OK] Saved strategy: {}", name);

    Ok(CommandResult {
        success: true,
        message: format!("Strategy '{}' saved", name),
    })
}

/// Get all strategies
#[tauri::command]
fn get_strategies(state: State<AppState>) -> Result<Vec<StrategyData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let strategies = db.get_strategies().map_err(|e| e.to_string())?;

    Ok(strategies
        .into_iter()
        .map(|s| StrategyData {
            id: s.id,
            name: s.name,
            description: s.description,
            entry_condition: s.entry_condition.as_str().to_string(),
            entry_threshold: s.entry_threshold,
            exit_condition: s.exit_condition.as_str().to_string(),
            exit_threshold: s.exit_threshold,
            stop_loss_percent: s.stop_loss_percent,
            take_profit_percent: s.take_profit_percent,
            position_size_percent: s.position_size_percent,
            created_at: s.created_at,
        })
        .collect())
}

/// Delete a strategy
#[tauri::command]
fn delete_strategy(state: State<AppState>, name: String) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.delete_strategy(&name).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: format!("Strategy '{}' deleted", name),
    })
}

/// Run a backtest
#[tauri::command]
fn run_backtest(
    state: State<AppState>,
    strategy_name: String,
    symbol: String,
    initial_capital: f64,
) -> Result<BacktestResultData, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    // Get strategy
    let strategy = db
        .get_strategy(&strategy_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Strategy '{}' not found", strategy_name))?;

    // Get prices and indicators
    let prices = db.get_prices(&symbol).map_err(|e| e.to_string())?;
    let indicators = db.get_all_indicators(&symbol).map_err(|e| e.to_string())?;

    if prices.is_empty() {
        return Err(format!("No price data for {}", symbol));
    }

    if indicators.is_empty() {
        return Err(format!(
            "No indicator data for {}. Calculate indicators first.",
            symbol
        ));
    }

    // Run backtest
    let config = BacktestConfig {
        initial_capital,
        commission_per_trade: 0.0,
    };
    let engine = BacktestEngine::new(config);
    let result = engine.run(&strategy, &symbol, &prices, &indicators);

    // Save result
    db.save_backtest_result(&result).map_err(|e| e.to_string())?;

    println!(
        "[OK] Backtest completed for {} on {}: {:.2}% return",
        strategy_name, symbol, result.metrics.total_return
    );

    // Convert to frontend format
    Ok(BacktestResultData {
        id: result.id,
        strategy_id: result.strategy_id,
        strategy_name: result.strategy_name,
        symbol: result.symbol,
        start_date: result.start_date.to_string(),
        end_date: result.end_date.to_string(),
        initial_capital: result.initial_capital,
        final_capital: result.final_capital,
        metrics: MetricsData {
            total_return: result.metrics.total_return,
            total_return_dollars: result.metrics.total_return_dollars,
            max_drawdown: result.metrics.max_drawdown,
            sharpe_ratio: result.metrics.sharpe_ratio,
            win_rate: result.metrics.win_rate,
            total_trades: result.metrics.total_trades,
            winning_trades: result.metrics.winning_trades,
            losing_trades: result.metrics.losing_trades,
            avg_win_percent: result.metrics.avg_win_percent,
            avg_loss_percent: result.metrics.avg_loss_percent,
            profit_factor: result.metrics.profit_factor,
            avg_trade_duration_days: result.metrics.avg_trade_duration_days,
        },
        trades: result
            .trades
            .into_iter()
            .map(|t| BacktestTradeData {
                id: t.id,
                symbol: t.symbol,
                direction: t.direction.as_str().to_string(),
                entry_date: t.entry_date.to_string(),
                entry_price: t.entry_price,
                entry_reason: t.entry_reason,
                exit_date: t.exit_date.map(|d| d.to_string()),
                exit_price: t.exit_price,
                exit_reason: t.exit_reason,
                shares: t.shares,
                profit_loss: t.profit_loss,
                profit_loss_percent: t.profit_loss_percent,
            })
            .collect(),
        created_at: result.created_at,
    })
}

/// Get backtest history
#[tauri::command]
fn get_backtest_results(
    state: State<AppState>,
    strategy_name: Option<String>,
    symbol: Option<String>,
    limit: usize,
) -> Result<Vec<BacktestResultData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let results = db
        .get_backtest_results(
            strategy_name.as_deref(),
            symbol.as_deref(),
            limit,
        )
        .map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .map(|r| BacktestResultData {
            id: r.id,
            strategy_id: r.strategy_id,
            strategy_name: r.strategy_name,
            symbol: r.symbol,
            start_date: r.start_date.to_string(),
            end_date: r.end_date.to_string(),
            initial_capital: r.initial_capital,
            final_capital: r.final_capital,
            metrics: MetricsData {
                total_return: r.metrics.total_return,
                total_return_dollars: r.metrics.total_return_dollars,
                max_drawdown: r.metrics.max_drawdown,
                sharpe_ratio: r.metrics.sharpe_ratio,
                win_rate: r.metrics.win_rate,
                total_trades: r.metrics.total_trades,
                winning_trades: r.metrics.winning_trades,
                losing_trades: r.metrics.losing_trades,
                avg_win_percent: r.metrics.avg_win_percent,
                avg_loss_percent: r.metrics.avg_loss_percent,
                profit_factor: r.metrics.profit_factor,
                avg_trade_duration_days: r.metrics.avg_trade_duration_days,
            },
            trades: Vec::new(), // Trades not loaded in list view
            created_at: r.created_at,
        })
        .collect())
}

/// Get backtest detail with trades
#[tauri::command]
fn get_backtest_detail(
    state: State<AppState>,
    backtest_id: i64,
) -> Result<Option<BacktestResultData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let result = db
        .get_backtest_detail(backtest_id)
        .map_err(|e| e.to_string())?;

    Ok(result.map(|r| BacktestResultData {
        id: r.id,
        strategy_id: r.strategy_id,
        strategy_name: r.strategy_name,
        symbol: r.symbol,
        start_date: r.start_date.to_string(),
        end_date: r.end_date.to_string(),
        initial_capital: r.initial_capital,
        final_capital: r.final_capital,
        metrics: MetricsData {
            total_return: r.metrics.total_return,
            total_return_dollars: r.metrics.total_return_dollars,
            max_drawdown: r.metrics.max_drawdown,
            sharpe_ratio: r.metrics.sharpe_ratio,
            win_rate: r.metrics.win_rate,
            total_trades: r.metrics.total_trades,
            winning_trades: r.metrics.winning_trades,
            losing_trades: r.metrics.losing_trades,
            avg_win_percent: r.metrics.avg_win_percent,
            avg_loss_percent: r.metrics.avg_loss_percent,
            profit_factor: r.metrics.profit_factor,
            avg_trade_duration_days: r.metrics.avg_trade_duration_days,
        },
        trades: r
            .trades
            .into_iter()
            .map(|t| BacktestTradeData {
                id: t.id,
                symbol: t.symbol,
                direction: t.direction.as_str().to_string(),
                entry_date: t.entry_date.to_string(),
                entry_price: t.entry_price,
                entry_reason: t.entry_reason,
                exit_date: t.exit_date.map(|d| d.to_string()),
                exit_price: t.exit_price,
                exit_reason: t.exit_reason,
                shares: t.shares,
                profit_loss: t.profit_loss,
                profit_loss_percent: t.profit_loss_percent,
            })
            .collect(),
        created_at: r.created_at,
    }))
}

/// Delete a backtest result
#[tauri::command]
fn delete_backtest(state: State<AppState>, backtest_id: i64) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.delete_backtest(backtest_id).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: "Backtest deleted".to_string(),
    })
}

// ============================================================================
// Watchlist/Symbol Group Commands
// ============================================================================

/// Watchlist data for frontend
#[derive(Serialize)]
struct WatchlistData {
    id: i64,
    name: String,
    description: Option<String>,
    symbol_count: i64,
    symbols: Vec<String>,
}

/// Watchlist summary (without symbols) for list views
#[derive(Serialize)]
struct WatchlistSummary {
    id: i64,
    name: String,
    description: Option<String>,
    symbol_count: i64,
}

/// Create a new watchlist/symbol group
#[tauri::command]
fn create_watchlist(
    state: State<AppState>,
    name: String,
    symbols: Vec<String>,
    description: Option<String>,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let symbols_upper: Vec<String> = symbols.iter().map(|s| s.to_uppercase()).collect();

    db.create_watchlist(&name, &symbols_upper, description.as_deref())
        .map_err(|e| e.to_string())?;

    println!("[OK] Created watchlist '{}' with {} symbols", name, symbols_upper.len());

    Ok(CommandResult {
        success: true,
        message: format!("Watchlist '{}' created with {} symbols", name, symbols_upper.len()),
    })
}

/// Get all watchlists (summary view)
#[tauri::command]
fn get_all_watchlists(state: State<AppState>) -> Result<Vec<WatchlistSummary>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let watchlists = db.get_all_watchlists().map_err(|e| e.to_string())?;

    Ok(watchlists
        .into_iter()
        .map(|(id, name, description, symbol_count)| WatchlistSummary {
            id,
            name,
            description,
            symbol_count,
        })
        .collect())
}

/// Get a watchlist with its symbols
#[tauri::command]
fn get_watchlist_detail(state: State<AppState>, name: String) -> Result<Option<WatchlistData>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let result = db.get_watchlist_full(&name).map_err(|e| e.to_string())?;

    Ok(result.map(|(id, name, description, symbols)| WatchlistData {
        id,
        name,
        description,
        symbol_count: symbols.len() as i64,
        symbols,
    }))
}

/// Delete a watchlist
#[tauri::command]
fn delete_watchlist(state: State<AppState>, name: String) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let deleted = db.delete_watchlist(&name).map_err(|e| e.to_string())?;

    if deleted {
        println!("[OK] Deleted watchlist '{}'", name);
        Ok(CommandResult {
            success: true,
            message: format!("Watchlist '{}' deleted", name),
        })
    } else {
        Ok(CommandResult {
            success: false,
            message: format!("Watchlist '{}' not found", name),
        })
    }
}

/// Add a symbol to an existing watchlist
#[tauri::command]
fn add_symbol_to_watchlist(
    state: State<AppState>,
    watchlist_name: String,
    symbol: String,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let success = db
        .add_symbol_to_watchlist(&watchlist_name, &symbol)
        .map_err(|e| e.to_string())?;

    if success {
        println!("[OK] Added {} to watchlist '{}'", symbol, watchlist_name);
        Ok(CommandResult {
            success: true,
            message: format!("Added {} to '{}'", symbol, watchlist_name),
        })
    } else {
        Ok(CommandResult {
            success: false,
            message: format!("Watchlist '{}' not found", watchlist_name),
        })
    }
}

/// Remove a symbol from a watchlist
#[tauri::command]
fn remove_symbol_from_watchlist(
    state: State<AppState>,
    watchlist_name: String,
    symbol: String,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    let success = db
        .remove_symbol_from_watchlist(&watchlist_name, &symbol)
        .map_err(|e| e.to_string())?;

    if success {
        println!("[OK] Removed {} from watchlist '{}'", symbol, watchlist_name);
        Ok(CommandResult {
            success: true,
            message: format!("Removed {} from '{}'", symbol, watchlist_name),
        })
    } else {
        Ok(CommandResult {
            success: false,
            message: format!("Symbol {} not found in watchlist '{}'", symbol, watchlist_name),
        })
    }
}

/// Update watchlist description
#[tauri::command]
fn update_watchlist_description(
    state: State<AppState>,
    name: String,
    description: Option<String>,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let success = db
        .update_watchlist_description(&name, description.as_deref())
        .map_err(|e| e.to_string())?;

    if success {
        Ok(CommandResult {
            success: true,
            message: format!("Watchlist '{}' description updated", name),
        })
    } else {
        Ok(CommandResult {
            success: false,
            message: format!("Watchlist '{}' not found", name),
        })
    }
}

/// Rename a watchlist
#[tauri::command]
fn rename_watchlist(
    state: State<AppState>,
    old_name: String,
    new_name: String,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let success = db
        .rename_watchlist(&old_name, &new_name)
        .map_err(|e| e.to_string())?;

    if success {
        println!("[OK] Renamed watchlist '{}' to '{}'", old_name, new_name);
        Ok(CommandResult {
            success: true,
            message: format!("Watchlist renamed from '{}' to '{}'", old_name, new_name),
        })
    } else {
        Ok(CommandResult {
            success: false,
            message: format!("Watchlist '{}' not found", old_name),
        })
    }
}

// ============================================================================
// VECTOR DATABASE COMMANDS
// ============================================================================

/// Vector search result for frontend
#[derive(Serialize)]
struct VectorSearchResponse {
    id: String,
    content: String,
    score: f64,
    result_type: String,
    symbol: Option<String>,
    date: Option<String>,
}

/// Vector stats response
#[derive(Serialize)]
struct VectorStatsResponse {
    events_count: usize,
    patterns_count: usize,
}

/// Search the vector database for relevant market events and patterns
#[tauri::command]
fn vector_search(query: String, limit: usize) -> Result<Vec<VectorSearchResponse>, String> {
    let store = VectorStore::new(&get_data_path("vectors.db")).map_err(|e| e.to_string())?;

    let results = store.search_all(&query, limit).map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .map(|r| VectorSearchResponse {
            id: r.id,
            content: r.content,
            score: r.score as f64,
            result_type: r.result_type,
            symbol: r.symbol,
            date: r.date,
        })
        .collect())
}

/// Add a market event to the vector database
#[tauri::command]
fn add_market_event(
    symbol: String,
    event_type: String,
    title: String,
    content: String,
    date: String,
    sentiment: Option<f32>,
) -> Result<CommandResult, String> {
    let store = VectorStore::new(&get_data_path("vectors.db")).map_err(|e| e.to_string())?;

    // Generate deterministic ID from content - same article always gets same ID
    // This allows INSERT OR REPLACE to work correctly and prevent duplicates
    let title_hash: u32 = title.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32).wrapping_mul(31));
    let content_hash: u32 = content.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32).wrapping_mul(31));

    let event = MarketEvent {
        id: format!("{}-{}-{}-{:x}{:x}", symbol, event_type, date, title_hash, content_hash),
        symbol,
        event_type,
        title,
        content,
        date,
        sentiment,
        metadata: None,
    };

    store.add_market_event(&event).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: "Market event added to vector database".to_string(),
    })
}

/// Add a price pattern to the vector database
#[tauri::command]
fn add_price_pattern(
    symbol: String,
    pattern_type: String,
    start_date: String,
    end_date: String,
    price_change_percent: f32,
    volume_change_percent: f32,
    description: String,
) -> Result<CommandResult, String> {
    let store = VectorStore::new(&get_data_path("vectors.db")).map_err(|e| e.to_string())?;

    let pattern = PricePattern {
        id: format!("{}-{}-{}", symbol, pattern_type, start_date),
        symbol,
        pattern_type,
        start_date,
        end_date,
        price_change_percent,
        volume_change_percent,
        description,
    };

    store.add_price_pattern(&pattern).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: "Price pattern added to vector database".to_string(),
    })
}

/// Response for add_market_event_with_pattern command
#[derive(Serialize)]
struct EventWithPatternResponse {
    success: bool,
    message: String,
    event_id: String,
    pattern_id: Option<String>,
    price_change_percent: Option<f64>,
    pattern_error: Option<String>,  // Capture actual error reason
}

/// Add a market event with an auto-linked price pattern
/// Uses local Yahoo price data for pattern linking (Finnhub free tier doesn't allow candle access)
#[tauri::command]
fn add_market_event_with_pattern(
    state: State<AppState>,
    symbol: String,
    event_type: String,
    title: String,
    content: String,
    date: String,
    sentiment: Option<f32>,
    _api_key: Option<String>,  // Kept for API compatibility but not used
    link_pattern: bool,
    days_window: Option<i64>,
) -> Result<EventWithPatternResponse, String> {
    let store = VectorStore::new(&get_data_path("vectors.db")).map_err(|e| e.to_string())?;

    // Generate deterministic ID from content
    let title_hash: u32 = title.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32).wrapping_mul(31));
    let content_hash: u32 = content.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32).wrapping_mul(31));
    let event_id = format!("{}-{}-{}-{:x}{:x}", symbol, event_type, date, title_hash, content_hash);

    // Create and store the event
    let event = MarketEvent {
        id: event_id.clone(),
        symbol: symbol.clone(),
        event_type: event_type.clone(),
        title: title.clone(),
        content,
        date: date.clone(),
        sentiment,
        metadata: None,
    };

    store.add_market_event(&event).map_err(|e| e.to_string())?;

    // If link_pattern is true, use local Yahoo price data to calculate reaction
    let mut pattern_id = None;
    let mut price_change = None;
    let mut pattern_error: Option<String> = None;

    if link_pattern {
        let window = days_window.unwrap_or(3) as i32;

        // Get local price data from Yahoo (already fetched)
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let prices = db.get_prices(&symbol).map_err(|e| e.to_string())?;

        if prices.is_empty() {
            pattern_error = Some(format!("No local price data for {}", symbol));
        } else {
            // Parse event date
            use chrono::NaiveDate;
            if let Ok(event_date) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
                // Find prices around the event date
                let mut pre_price: Option<f64> = None;
                let mut post_price: Option<f64> = None;
                let mut start_date_str = String::new();
                let mut end_date_str = String::new();
                let mut pre_volume: i64 = 0;
                let mut post_volume: i64 = 0;

                for price in &prices {
                    if let Ok(price_date) = NaiveDate::parse_from_str(&price.date.to_string(), "%Y-%m-%d") {
                        let days_diff = (price_date - event_date).num_days();

                        // Find closest price BEFORE event (within window)
                        if days_diff >= -(window as i64) && days_diff <= 0 {
                            if pre_price.is_none() || days_diff > -(window as i64) {
                                pre_price = Some(price.close);
                                start_date_str = price.date.to_string();
                                pre_volume = price.volume;
                            }
                        }

                        // Find closest price AFTER event (within window)
                        if days_diff >= 0 && days_diff <= window as i64 {
                            post_price = Some(price.close);
                            end_date_str = price.date.to_string();
                            post_volume = price.volume;
                        }
                    }
                }

                if let (Some(pre), Some(post)) = (pre_price, post_price) {
                    let price_change_pct = ((post - pre) / pre) * 100.0;
                    let volume_change_pct = if pre_volume > 0 {
                        ((post_volume - pre_volume) as f64 / pre_volume as f64) * 100.0
                    } else {
                        0.0
                    };

                    // Create linked pattern
                    let pid = format!("news-reaction-{}", event_id);
                    let pattern = PricePattern {
                        id: pid.clone(),
                        symbol: symbol.clone(),
                        pattern_type: "news_reaction".to_string(),
                        start_date: start_date_str.clone(),
                        end_date: end_date_str.clone(),
                        price_change_percent: price_change_pct as f32,
                        volume_change_percent: volume_change_pct as f32,
                        description: format!(
                            "Price reaction to: {} | Pre: ${:.2} → Post: ${:.2} ({:+.2}%)",
                            title,
                            pre,
                            post,
                            price_change_pct
                        ),
                    };

                    if store.add_price_pattern(&pattern).is_ok() {
                        pattern_id = Some(pid);
                        price_change = Some(price_change_pct);
                    }
                } else {
                    pattern_error = Some(format!("No price data found around {} for {}", date, symbol));
                }
            } else {
                pattern_error = Some(format!("Invalid date format: {}", date));
            }
        }
    }

    let message = if let Some(ref pid) = pattern_id {
        format!(
            "Event saved with linked pattern (price change: {:+.2}%)",
            price_change.unwrap_or(0.0)
        )
    } else if link_pattern {
        if let Some(ref err) = pattern_error {
            format!("Event saved (pattern failed: {})", err)
        } else {
            "Event saved (pattern linking failed - unknown reason)".to_string()
        }
    } else {
        "Market event added to vector database".to_string()
    };

    Ok(EventWithPatternResponse {
        success: true,
        message,
        event_id,
        pattern_id,
        price_change_percent: price_change,
        pattern_error,
    })
}

/// Get vector database statistics
#[tauri::command]
fn get_vector_stats() -> Result<VectorStatsResponse, String> {
    let store = VectorStore::new(&get_data_path("vectors.db")).map_err(|e| e.to_string())?;

    let (events_count, patterns_count) = store.get_stats().map_err(|e| e.to_string())?;

    Ok(VectorStatsResponse {
        events_count,
        patterns_count,
    })
}

// ============================================================================
// CLAUDE AI COMMANDS
// ============================================================================

/// Claude chat response for frontend
#[derive(Serialize)]
struct ClaudeChatResponse {
    response: String,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
    conversation_id: String,
}

/// Chat with Claude using financial context from the database
#[tauri::command]
fn claude_chat(
    state: State<AppState>,
    query: String,
    api_key: String,
) -> Result<ClaudeChatResponse, String> {
    // Build financial context from database
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Get tracked symbols and their prices
    let symbols = db.get_symbols_with_data().map_err(|e| e.to_string())?;

    let mut price_contexts = Vec::new();
    for symbol in &symbols {
        if let Ok(prices) = db.get_prices(symbol) {
            if prices.len() >= 2 {
                let current = prices.last().unwrap();
                let previous = &prices[prices.len() - 2];
                let change_pct = if previous.close > 0.0 {
                    ((current.close - previous.close) / previous.close) * 100.0
                } else {
                    0.0
                };

                price_contexts.push(ClaudePriceContext {
                    symbol: symbol.clone(),
                    price: current.close,
                    change_percent: Some(change_pct),
                    date: current.date.to_string(),
                });
            } else if let Some(price) = prices.last() {
                price_contexts.push(ClaudePriceContext {
                    symbol: symbol.clone(),
                    price: price.close,
                    change_percent: None,
                    date: price.date.to_string(),
                });
            }
        }
    }

    // Drop the db lock before making the API call
    drop(db);

    let context = FinancialContext {
        symbols,
        recent_prices: price_contexts,
        query: query.clone(),
    };

    // Create Claude client and query
    let client = ClaudeClient::with_api_key(api_key)
        .map_err(|e| e.to_string())?;

    let result = client
        .query_with_context(&query, Some(&context), None)
        .map_err(|e| e.to_string())?;

    // Store the conversation in vector database for future reference
    let store = VectorStore::new(&get_data_path("vectors.db")).map_err(|e| e.to_string())?;

    let event = MarketEvent {
        id: format!("chat-{}", result.conversation_id),
        symbol: "CHAT".to_string(),
        event_type: "ai_analysis".to_string(),
        title: query.chars().take(100).collect(),
        content: format!("Q: {}\n\nA: {}", query, result.response),
        date: Utc::now().format("%Y-%m-%d").to_string(),
        sentiment: None,
        metadata: Some(format!("model:{},tokens:{}", result.model, result.input_tokens + result.output_tokens)),
    };

    // Store but don't fail if it errors
    let _ = store.add_market_event(&event);

    Ok(ClaudeChatResponse {
        response: result.response,
        model: result.model,
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
        conversation_id: result.conversation_id,
    })
}

/// Simple Claude query without financial context
#[tauri::command]
fn claude_query(
    query: String,
    api_key: String,
) -> Result<ClaudeChatResponse, String> {
    let client = ClaudeClient::with_api_key(api_key)
        .map_err(|e| e.to_string())?;

    let result = client
        .query(&query)
        .map_err(|e| e.to_string())?;

    Ok(ClaudeChatResponse {
        response: result.response,
        model: result.model,
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
        conversation_id: result.conversation_id,
    })
}

// ============================================================================
// Ollama LLM Commands (local AI via localhost:11434)
// ============================================================================

/// Check if Ollama is available (2-second timeout)
#[tauri::command]
async fn ollama_available() -> bool {
    let client = OllamaClient::new();
    client.is_available().await
}

/// Analyze sentiment of text using local Ollama
#[tauri::command]
async fn ollama_sentiment(text: String) -> Result<SentimentResult, String> {
    let client = OllamaClient::new();
    client.analyze_sentiment(&text).await.map_err(|e| e.to_string())
}

/// Explain a technical pattern using local Ollama
#[tauri::command]
async fn ollama_explain(pattern: String, context: Option<String>) -> Result<PatternExplanation, String> {
    let client = OllamaClient::new();
    let ctx = context.unwrap_or_default();
    client.explain_pattern(&pattern, &ctx).await.map_err(|e| e.to_string())
}

/// Ask Ollama a question with financial context
#[tauri::command]
async fn ollama_ask(question: String, context: String) -> Result<String, String> {
    let client = OllamaClient::new();
    client.answer_query(&question, &context).await.map_err(|e| e.to_string())
}

/// Response for fetch_news command
#[derive(Serialize)]
struct FetchNewsResponse {
    news: Vec<SimpleNewsItem>,
    count: usize,
}

/// Fetch news for a symbol from Finnhub API
#[tauri::command]
fn fetch_news(
    symbol: String,
    api_key: String,
    limit: Option<usize>,
) -> Result<FetchNewsResponse, String> {
    if api_key.is_empty() {
        return Err("Finnhub API key is required. Get one free at https://finnhub.io".to_string());
    }

    let client = FinnhubClient::new(api_key)
        .map_err(|e| e.to_string())?;

    let news_limit = limit.unwrap_or(5);
    let news = client.fetch_simple_news(&symbol, news_limit)
        .map_err(|e| e.to_string())?;

    let count = news.len();

    Ok(FetchNewsResponse { news, count })
}

/// Response for price reaction command
#[derive(Serialize)]
struct PriceReactionResponse {
    symbol: String,
    event_date: String,
    start_date: String,
    end_date: String,
    pre_price: f64,
    post_price: f64,
    price_change_percent: f64,
    volume_change_percent: f64,
    candle_count: usize,
}

/// Fetch price reaction around an event date
/// Returns price change from days_before to days_after the event
#[tauri::command]
fn fetch_price_reaction(
    symbol: String,
    event_date: String,
    api_key: String,
    days_window: Option<i64>,
) -> Result<PriceReactionResponse, String> {
    if api_key.is_empty() {
        return Err("Finnhub API key is required".to_string());
    }

    let client = FinnhubClient::new(api_key)
        .map_err(|e| e.to_string())?;

    let window = days_window.unwrap_or(3);
    let reaction = client.fetch_price_reaction(&symbol, &event_date, window)
        .map_err(|e| e.to_string())?;

    Ok(PriceReactionResponse {
        symbol: reaction.symbol,
        event_date: reaction.event_date,
        start_date: reaction.start_date,
        end_date: reaction.end_date,
        pre_price: reaction.pre_price,
        post_price: reaction.post_price,
        price_change_percent: reaction.price_change_percent,
        volume_change_percent: reaction.volume_change_percent,
        candle_count: reaction.candle_count,
    })
}

/// Response for fetch_candles command - raw OHLCV data
#[derive(Serialize)]
struct CandleDataResponse {
    symbol: String,
    close: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    open: Vec<f64>,
    volume: Vec<i64>,
    timestamp: Vec<i64>,
    dates: Vec<String>,  // Human-readable dates (YYYY-MM-DD)
}

/// Fetch OHLCV candle data for a symbol and date range
/// Returns raw candle data for charting and analysis
#[tauri::command]
fn fetch_candles(
    symbol: String,
    from_date: String,
    to_date: String,
    api_key: String,
    resolution: Option<String>,
) -> Result<CandleDataResponse, String> {
    if api_key.is_empty() {
        return Err("Finnhub API key is required".to_string());
    }

    use chrono::NaiveDate;

    let from = NaiveDate::parse_from_str(&from_date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid from_date: {}", e))?;
    let to = NaiveDate::parse_from_str(&to_date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid to_date: {}", e))?;

    let from_ts = from.and_hms_opt(0, 0, 0)
        .ok_or("Failed to create start timestamp")?
        .and_utc()
        .timestamp();
    let to_ts = to.and_hms_opt(23, 59, 59)
        .ok_or("Failed to create end timestamp")?
        .and_utc()
        .timestamp();

    let client = FinnhubClient::new(api_key)
        .map_err(|e| e.to_string())?;

    let res = resolution.unwrap_or_else(|| "D".to_string());
    let candles = client.fetch_candles(&symbol, &res, from_ts, to_ts)
        .map_err(|e| e.to_string())?;

    // Convert timestamps to human-readable dates
    let dates: Vec<String> = candles.timestamp.iter()
        .filter_map(|&ts| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
        })
        .collect();

    Ok(CandleDataResponse {
        symbol: symbol.to_uppercase(),
        close: candles.close,
        high: candles.high,
        low: candles.low,
        open: candles.open,
        volume: candles.volume,
        timestamp: candles.timestamp,
        dates,
    })
}

// ============================================================================
// PAPER TRADING COMMANDS
// ============================================================================

/// Paper wallet balance response
#[derive(Serialize)]
struct PaperWalletResponse {
    cash: f64,
    positions_value: f64,
    total_equity: f64,
    starting_capital: f64,
    total_pnl: f64,
    total_pnl_percent: f64,
}

/// Paper position with current price and P&L
#[derive(Serialize)]
struct PaperPositionResponse {
    id: i64,
    symbol: String,
    quantity: f64,
    entry_price: f64,
    entry_date: String,
    current_price: f64,
    current_value: f64,
    cost_basis: f64,
    unrealized_pnl: f64,
    unrealized_pnl_percent: f64,
}

/// Paper trade response
#[derive(Serialize)]
struct PaperTradeResponse {
    id: i64,
    symbol: String,
    action: String,
    quantity: f64,
    price: f64,
    pnl: Option<f64>,
    timestamp: String,
    notes: Option<String>,
}

/// Get paper wallet balance and portfolio summary
#[tauri::command]
fn get_paper_balance(state: State<AppState>) -> Result<PaperWalletResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let (cash, positions_value, total_equity) = db
        .get_paper_portfolio_value()
        .map_err(|e| e.to_string())?;

    let starting_capital = 100000.0; // Default starting capital
    let total_pnl = total_equity - starting_capital;
    let total_pnl_percent = if starting_capital > 0.0 {
        (total_pnl / starting_capital) * 100.0
    } else {
        0.0
    };

    Ok(PaperWalletResponse {
        cash,
        positions_value,
        total_equity,
        starting_capital,
        total_pnl,
        total_pnl_percent,
    })
}

/// Get all paper positions with current values
#[tauri::command]
fn get_paper_positions(state: State<AppState>) -> Result<Vec<PaperPositionResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let positions = db.get_paper_positions().map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for pos in positions {
        let current_price = db
            .get_latest_price(&pos.symbol)
            .map_err(|e| e.to_string())?
            .unwrap_or(pos.entry_price);

        let cost_basis = pos.quantity * pos.entry_price;
        let current_value = pos.quantity * current_price;
        let unrealized_pnl = current_value - cost_basis;
        let unrealized_pnl_percent = if cost_basis > 0.0 {
            (unrealized_pnl / cost_basis) * 100.0
        } else {
            0.0
        };

        result.push(PaperPositionResponse {
            id: pos.id,
            symbol: pos.symbol,
            quantity: pos.quantity,
            entry_price: pos.entry_price,
            entry_date: pos.entry_date,
            current_price,
            current_value,
            cost_basis,
            unrealized_pnl,
            unrealized_pnl_percent,
        });
    }

    Ok(result)
}

/// Execute a paper trade
#[tauri::command]
fn execute_paper_trade(
    state: State<AppState>,
    symbol: String,
    action: String,
    quantity: f64,
    price: Option<f64>,
    notes: Option<String>,
) -> Result<PaperTradeResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    // Get current price if not provided
    let trade_price = match price {
        Some(p) => p,
        None => db
            .get_latest_price(&symbol)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("No price data for {}. Fetch prices first or specify price.", symbol))?,
    };

    let trade_action = PaperTradeAction::from_str(&action);

    let trade = db
        .execute_paper_trade(
            &symbol,
            trade_action,
            quantity,
            trade_price,
            None,
            notes.as_deref(),
        )
        .map_err(|e| e.to_string())?;

    println!(
        "[OK] Paper trade: {} {} {} @ ${:.2}",
        trade.action.as_str(),
        trade.quantity,
        trade.symbol,
        trade.price
    );

    Ok(PaperTradeResponse {
        id: trade.id,
        symbol: trade.symbol,
        action: trade.action.as_str().to_string(),
        quantity: trade.quantity,
        price: trade.price,
        pnl: trade.pnl,
        timestamp: trade.timestamp,
        notes: trade.notes,
    })
}

/// Get paper trade history
#[tauri::command]
fn get_paper_trades(
    state: State<AppState>,
    symbol: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<PaperTradeResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let trades = db
        .get_paper_trades(symbol.as_deref(), limit.unwrap_or(100))
        .map_err(|e| e.to_string())?;

    Ok(trades
        .into_iter()
        .map(|t| PaperTradeResponse {
            id: t.id,
            symbol: t.symbol,
            action: t.action.as_str().to_string(),
            quantity: t.quantity,
            price: t.price,
            pnl: t.pnl,
            timestamp: t.timestamp,
            notes: t.notes,
        })
        .collect())
}

/// Reset paper trading account
#[tauri::command]
fn reset_paper_account(
    state: State<AppState>,
    starting_cash: Option<f64>,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let cash = starting_cash.unwrap_or(100000.0);
    db.reset_paper_account(cash).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: format!("Paper trading account reset with ${:.2}", cash),
    })
}

// ============================================================================
// DC TRADER COMMANDS (Separate from KALIC AI paper trading)
// ============================================================================

/// DC wallet balance response
#[derive(Serialize)]
struct DcWalletResponse {
    cash: f64,
    positions_value: f64,
    total_equity: f64,
    starting_capital: f64,
    total_pnl: f64,
    total_pnl_percent: f64,
}

/// DC position with current price and P&L
#[derive(Serialize)]
struct DcPositionResponse {
    id: i64,
    symbol: String,
    quantity: f64,
    entry_price: f64,
    entry_date: String,
    current_price: f64,
    current_value: f64,
    cost_basis: f64,
    unrealized_pnl: f64,
    unrealized_pnl_percent: f64,
}

/// DC trade response
#[derive(Serialize)]
struct DcTradeResponse {
    id: i64,
    symbol: String,
    action: String,
    quantity: f64,
    price: f64,
    pnl: Option<f64>,
    timestamp: String,
    notes: Option<String>,
}

/// Import result response
#[derive(Serialize)]
struct ImportResultResponse {
    success_count: i32,
    error_count: i32,
    errors: Vec<String>,
}

/// Portfolio snapshot response
#[derive(Serialize)]
struct PortfolioSnapshotResponse {
    id: i64,
    team: String,
    date: String,
    total_value: f64,
    cash: f64,
    positions_value: f64,
}

/// Team config response
#[derive(Serialize)]
struct TeamConfigResponse {
    id: i64,
    name: String,
    description: Option<String>,
    kalic_starting_capital: f64,
    dc_starting_capital: f64,
    created_at: String,
}

/// Competition stats response
#[derive(Serialize)]
struct CompetitionStatsResponse {
    kalic_total: f64,
    kalic_cash: f64,
    kalic_positions: f64,
    kalic_pnl_pct: f64,
    kalic_trades: i32,
    dc_total: f64,
    dc_cash: f64,
    dc_positions: f64,
    dc_pnl_pct: f64,
    dc_trades: i32,
    leader: String,
    lead_amount: f64,
}

/// Get DC wallet balance and portfolio summary
#[tauri::command]
fn get_dc_balance(state: State<AppState>) -> Result<DcWalletResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let wallet = db.get_dc_wallet().map_err(|e| e.to_string())?;
    let (cash, positions_value, total_equity) = db
        .get_dc_portfolio_value()
        .map_err(|e| e.to_string())?;

    let total_pnl = total_equity - wallet.starting_capital;
    let total_pnl_percent = if wallet.starting_capital > 0.0 {
        (total_pnl / wallet.starting_capital) * 100.0
    } else {
        0.0
    };

    Ok(DcWalletResponse {
        cash,
        positions_value,
        total_equity,
        starting_capital: wallet.starting_capital,
        total_pnl,
        total_pnl_percent,
    })
}

/// Get all DC positions with current values
#[tauri::command]
fn get_dc_positions(state: State<AppState>) -> Result<Vec<DcPositionResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let positions = db.get_dc_positions().map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for pos in positions {
        let current_price = db
            .get_latest_price(&pos.symbol)
            .map_err(|e| e.to_string())?
            .unwrap_or(pos.entry_price);

        let cost_basis = pos.quantity * pos.entry_price;
        let current_value = pos.quantity * current_price;
        let unrealized_pnl = current_value - cost_basis;
        let unrealized_pnl_percent = if cost_basis > 0.0 {
            (unrealized_pnl / cost_basis) * 100.0
        } else {
            0.0
        };

        result.push(DcPositionResponse {
            id: pos.id,
            symbol: pos.symbol,
            quantity: pos.quantity,
            entry_price: pos.entry_price,
            entry_date: pos.entry_date,
            current_price,
            current_value,
            cost_basis,
            unrealized_pnl,
            unrealized_pnl_percent,
        });
    }

    Ok(result)
}

/// Execute a DC trade
#[tauri::command]
fn execute_dc_trade(
    state: State<AppState>,
    symbol: String,
    action: String,
    quantity: f64,
    price: Option<f64>,
    notes: Option<String>,
) -> Result<DcTradeResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    // Get current price if not provided
    let trade_price = match price {
        Some(p) => p,
        None => db
            .get_latest_price(&symbol)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("No price data for {}. Fetch prices first or specify price.", symbol))?,
    };

    let trade = db
        .execute_dc_trade(
            &symbol,
            &action,
            quantity,
            trade_price,
            notes.as_deref(),
        )
        .map_err(|e| e.to_string())?;

    println!(
        "[OK] DC trade: {} {} {} @ ${:.2}",
        trade.action,
        trade.quantity,
        trade.symbol,
        trade.price
    );

    Ok(DcTradeResponse {
        id: trade.id,
        symbol: trade.symbol,
        action: trade.action,
        quantity: trade.quantity,
        price: trade.price,
        pnl: trade.pnl,
        timestamp: trade.timestamp,
        notes: trade.notes,
    })
}

/// Get DC trade history
#[tauri::command]
fn get_dc_trades(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Vec<DcTradeResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let trades = db
        .get_dc_trades(limit.unwrap_or(100))
        .map_err(|e| e.to_string())?;

    Ok(trades
        .into_iter()
        .map(|t| DcTradeResponse {
            id: t.id,
            symbol: t.symbol,
            action: t.action,
            quantity: t.quantity,
            price: t.price,
            pnl: t.pnl,
            timestamp: t.timestamp,
            notes: t.notes,
        })
        .collect())
}

/// Reset DC trading account
#[tauri::command]
fn reset_dc_account(
    state: State<AppState>,
    starting_cash: Option<f64>,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let cash = starting_cash.unwrap_or(1000000.0);
    db.reset_dc_account(cash).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: format!("DC trading account reset with ${:.2}", cash),
    })
}

/// Import DC trades from CSV
#[tauri::command]
fn import_dc_trades_csv(
    state: State<AppState>,
    #[allow(non_snake_case)]
    csvContent: String,
) -> Result<ImportResultResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let result = db.import_dc_trades_csv(&csvContent).map_err(|e| e.to_string())?;

    Ok(ImportResultResponse {
        success_count: result.success_count,
        error_count: result.error_count,
        errors: result.errors,
    })
}

/// Import DC trades from JSON
#[tauri::command]
fn import_dc_trades_json(
    state: State<AppState>,
    #[allow(non_snake_case)]
    jsonContent: String,
) -> Result<ImportResultResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let result = db.import_dc_trades_json(&jsonContent).map_err(|e| e.to_string())?;

    Ok(ImportResultResponse {
        success_count: result.success_count,
        error_count: result.error_count,
        errors: result.errors,
    })
}

/// Lookup current price for a symbol
#[tauri::command]
fn lookup_current_price(
    state: State<AppState>,
    symbol: String,
) -> Result<f64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let symbol = symbol.to_uppercase();

    db.get_latest_price(&symbol)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No price data for {}", symbol))
}

/// Record portfolio snapshot for a team
#[tauri::command]
fn record_portfolio_snapshot(
    state: State<AppState>,
    team: String,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.record_portfolio_snapshot(&team).map_err(|e| e.to_string())?;

    Ok(CommandResult {
        success: true,
        message: format!("Recorded snapshot for {}", team),
    })
}

/// Get portfolio snapshots for charting
#[tauri::command]
fn get_portfolio_snapshots(
    state: State<AppState>,
    team: Option<String>,
    days: Option<i32>,
) -> Result<Vec<PortfolioSnapshotResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let snapshots = db
        .get_portfolio_snapshots(team.as_deref(), days.unwrap_or(30))
        .map_err(|e| e.to_string())?;

    Ok(snapshots
        .into_iter()
        .map(|s| PortfolioSnapshotResponse {
            id: s.id,
            team: s.team,
            date: s.date,
            total_value: s.total_value,
            cash: s.cash,
            positions_value: s.positions_value,
        })
        .collect())
}

/// Save team configuration
#[tauri::command]
fn save_team_config(
    state: State<AppState>,
    name: String,
    description: Option<String>,
) -> Result<i64, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.save_team_config(&name, description.as_deref())
        .map_err(|e| e.to_string())
}

/// Load team configuration
#[tauri::command]
fn load_team_config(
    state: State<AppState>,
    name: String,
) -> Result<TeamConfigResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.load_team_config(&name).map_err(|e| e.to_string())?;

    Ok(TeamConfigResponse {
        id: config.id,
        name: config.name,
        description: config.description,
        kalic_starting_capital: config.kalic_starting_capital,
        dc_starting_capital: config.dc_starting_capital,
        created_at: config.created_at,
    })
}

/// List all team configurations
#[tauri::command]
fn list_team_configs(state: State<AppState>) -> Result<Vec<TeamConfigResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let configs = db.list_team_configs().map_err(|e| e.to_string())?;

    Ok(configs
        .into_iter()
        .map(|c| TeamConfigResponse {
            id: c.id,
            name: c.name,
            description: c.description,
            kalic_starting_capital: c.kalic_starting_capital,
            dc_starting_capital: c.dc_starting_capital,
            created_at: c.created_at,
        })
        .collect())
}

/// Get competition stats
#[tauri::command]
fn get_competition_stats(state: State<AppState>) -> Result<CompetitionStatsResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let stats = db.get_competition_stats().map_err(|e| e.to_string())?;

    Ok(CompetitionStatsResponse {
        kalic_total: stats.kalic_total,
        kalic_cash: stats.kalic_cash,
        kalic_positions: stats.kalic_positions,
        kalic_pnl_pct: stats.kalic_pnl_pct,
        kalic_trades: stats.kalic_trades,
        dc_total: stats.dc_total,
        dc_cash: stats.dc_cash,
        dc_positions: stats.dc_positions,
        dc_pnl_pct: stats.dc_pnl_pct,
        dc_trades: stats.dc_trades,
        leader: stats.leader,
        lead_amount: stats.lead_amount,
    })
}

// ============================================================================
// AI TRADER COMMANDS
// ============================================================================

/// AI trading session response
#[derive(Serialize)]
struct AiSessionResponse {
    id: i64,
    start_time: String,
    end_time: Option<String>,
    starting_portfolio_value: f64,
    ending_portfolio_value: Option<f64>,
    decisions_count: i32,
    trades_count: i32,
    session_notes: Option<String>,
    status: String,
}

impl From<AiTradingSession> for AiSessionResponse {
    fn from(s: AiTradingSession) -> Self {
        Self {
            id: s.id,
            start_time: s.start_time,
            end_time: s.end_time,
            starting_portfolio_value: s.starting_portfolio_value,
            ending_portfolio_value: s.ending_portfolio_value,
            decisions_count: s.decisions_count,
            trades_count: s.trades_count,
            session_notes: s.session_notes,
            status: s.status,
        }
    }
}

/// AI trade decision response
#[derive(Serialize)]
struct AiDecisionResponse {
    id: i64,
    session_id: Option<i64>,
    timestamp: String,
    action: String,
    symbol: String,
    quantity: Option<f64>,
    price_at_decision: Option<f64>,
    confidence: f64,
    reasoning: String,
    model_used: String,
    predicted_direction: Option<String>,
    predicted_price_target: Option<f64>,
    predicted_timeframe_days: Option<i32>,
    actual_outcome: Option<String>,
    actual_price_at_timeframe: Option<f64>,
    prediction_accurate: Option<bool>,
    paper_trade_id: Option<i64>,
}

impl From<AiTradeDecision> for AiDecisionResponse {
    fn from(d: AiTradeDecision) -> Self {
        Self {
            id: d.id,
            session_id: d.session_id,
            timestamp: d.timestamp,
            action: d.action,
            symbol: d.symbol,
            quantity: d.quantity,
            price_at_decision: d.price_at_decision,
            confidence: d.confidence,
            reasoning: d.reasoning,
            model_used: d.model_used,
            predicted_direction: d.predicted_direction,
            predicted_price_target: d.predicted_price_target,
            predicted_timeframe_days: d.predicted_timeframe_days,
            actual_outcome: d.actual_outcome,
            actual_price_at_timeframe: d.actual_price_at_timeframe,
            prediction_accurate: d.prediction_accurate,
            paper_trade_id: d.paper_trade_id,
        }
    }
}

/// AI performance snapshot response
#[derive(Serialize)]
struct AiSnapshotResponse {
    id: i64,
    timestamp: String,
    portfolio_value: f64,
    cash: f64,
    positions_value: f64,
    benchmark_value: f64,
    benchmark_symbol: String,
    total_pnl: f64,
    total_pnl_percent: f64,
    benchmark_pnl_percent: f64,
    prediction_accuracy: Option<f64>,
    trades_to_date: i32,
    winning_trades: i32,
    losing_trades: i32,
    win_rate: Option<f64>,
}

impl From<AiPerformanceSnapshot> for AiSnapshotResponse {
    fn from(s: AiPerformanceSnapshot) -> Self {
        Self {
            id: s.id,
            timestamp: s.timestamp,
            portfolio_value: s.portfolio_value,
            cash: s.cash,
            positions_value: s.positions_value,
            benchmark_value: s.benchmark_value,
            benchmark_symbol: s.benchmark_symbol,
            total_pnl: s.total_pnl,
            total_pnl_percent: s.total_pnl_percent,
            benchmark_pnl_percent: s.benchmark_pnl_percent,
            prediction_accuracy: s.prediction_accuracy,
            trades_to_date: s.trades_to_date,
            winning_trades: s.winning_trades,
            losing_trades: s.losing_trades,
            win_rate: s.win_rate,
        }
    }
}

/// AI trader status response
#[derive(Serialize)]
struct AiStatusResponse {
    is_running: bool,
    current_session: Option<AiSessionResponse>,
    portfolio_value: f64,
    cash: f64,
    positions_value: f64,
    is_bankrupt: bool,
    sessions_completed: u32,
    total_decisions: u32,
    total_trades: u32,
}

/// AI benchmark comparison response
#[derive(Serialize)]
struct AiBenchmarkResponse {
    portfolio_return_percent: f64,
    benchmark_return_percent: f64,
    alpha: f64,
    tracking_data: Vec<(String, f64, f64)>, // (timestamp, portfolio, benchmark)
}

/// AI compounding forecast response
#[derive(Serialize)]
struct AiForecastResponse {
    current_daily_return: f64,
    current_win_rate: f64,
    projected_30_days: f64,
    projected_90_days: f64,
    projected_365_days: f64,
    time_to_double: Option<u32>,
    time_to_bankruptcy: Option<u32>,
}

/// AI prediction accuracy response
#[derive(Serialize)]
struct AiAccuracyResponse {
    total_predictions: u32,
    accurate_predictions: u32,
    accuracy_percent: f64,
}

/// AI config response
#[derive(Serialize)]
struct AiConfigResponse {
    starting_capital: f64,
    max_position_size_percent: f64,
    stop_loss_percent: f64,
    take_profit_percent: f64,
    session_duration_minutes: u32,
    benchmark_symbol: String,
    model_priority: Vec<String>,
}

/// Get AI trader status
#[tauri::command]
fn ai_trader_get_status(state: State<AppState>) -> Result<AiStatusResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;
    let trader = AiTrader::new(config);

    let status = trader.get_status(&db).map_err(|e| e.to_string())?;

    Ok(AiStatusResponse {
        is_running: status.is_running,
        current_session: status.current_session.map(|s| s.into()),
        portfolio_value: status.portfolio_value,
        cash: status.cash,
        positions_value: status.positions_value,
        is_bankrupt: status.is_bankrupt,
        sessions_completed: status.sessions_completed,
        total_decisions: status.total_decisions,
        total_trades: status.total_trades,
    })
}

/// Get AI trader configuration
#[tauri::command]
fn ai_trader_get_config(state: State<AppState>) -> Result<AiConfigResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;

    Ok(AiConfigResponse {
        starting_capital: config.starting_capital,
        max_position_size_percent: config.max_position_size_percent,
        stop_loss_percent: config.stop_loss_percent,
        take_profit_percent: config.take_profit_percent,
        session_duration_minutes: config.session_duration_minutes,
        benchmark_symbol: config.benchmark_symbol,
        model_priority: config.model_priority,
    })
}

/// Start a new AI trading session
#[tauri::command]
fn ai_trader_start_session(state: State<AppState>) -> Result<AiSessionResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;
    let trader = AiTrader::new(config);

    let session = trader.start_session(&db).map_err(|e| e.to_string())?;

    println!("[AI Trader] Session {} started", session.id);

    Ok(session.into())
}

/// End the current AI trading session
#[tauri::command]
fn ai_trader_end_session(
    state: State<AppState>,
    notes: Option<String>,
) -> Result<Option<AiSessionResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;
    let trader = AiTrader::new(config);

    let session = trader
        .end_session(&db, notes.as_deref())
        .map_err(|e| e.to_string())?;

    if let Some(ref s) = session {
        println!("[AI Trader] Session {} ended", s.id);
    }

    Ok(session.map(|s| s.into()))
}

/// Run one AI trading cycle (gather context, query AI, execute trades)
#[tauri::command]
async fn ai_trader_run_cycle() -> Result<Vec<AiDecisionResponse>, String> {
    // Open a separate database connection for async operations
    // This is necessary because MutexGuard can't be held across await points
    let db_path = get_data_path("finance.db");
    let mut db = Database::open(&db_path).map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;
    let trader = AiTrader::new(config);

    // Check if Ollama is available
    if !trader.check_ollama().await {
        return Err("Ollama is not available. Start it with: ollama serve".to_string());
    }

    let decisions = trader.run_cycle(&mut db).await.map_err(|e| e.to_string())?;

    println!("[AI Trader] Cycle completed with {} decisions", decisions.len());

    Ok(decisions.into_iter().map(|d| d.into()).collect())
}

/// Get AI trading decisions
#[tauri::command]
fn ai_trader_get_decisions(
    state: State<AppState>,
    session_id: Option<i64>,
    symbol: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AiDecisionResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let decisions = db
        .get_ai_decisions(session_id, symbol.as_deref(), limit.unwrap_or(100))
        .map_err(|e| e.to_string())?;

    Ok(decisions.into_iter().map(|d| d.into()).collect())
}

/// Get AI performance history (snapshots)
#[tauri::command]
fn ai_trader_get_performance_history(
    state: State<AppState>,
    days: Option<u32>,
) -> Result<Vec<AiSnapshotResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let snapshots = db
        .get_ai_performance_snapshots(days.unwrap_or(30))
        .map_err(|e| e.to_string())?;

    Ok(snapshots.into_iter().map(|s| s.into()).collect())
}

/// Get benchmark comparison (portfolio vs SPY)
#[tauri::command]
fn ai_trader_get_benchmark_comparison(state: State<AppState>) -> Result<AiBenchmarkResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;
    let trader = AiTrader::new(config);

    let comparison = trader
        .get_benchmark_comparison(&db)
        .map_err(|e| e.to_string())?;

    Ok(AiBenchmarkResponse {
        portfolio_return_percent: comparison.portfolio_return_percent,
        benchmark_return_percent: comparison.benchmark_return_percent,
        alpha: comparison.alpha,
        tracking_data: comparison.tracking_data,
    })
}

/// Get compounding forecast
#[tauri::command]
fn ai_trader_get_compounding_forecast(state: State<AppState>) -> Result<AiForecastResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;
    let trader = AiTrader::new(config);

    let forecast = trader
        .get_compounding_forecast(&db)
        .map_err(|e| e.to_string())?;

    Ok(AiForecastResponse {
        current_daily_return: forecast.current_daily_return,
        current_win_rate: forecast.current_win_rate,
        projected_30_days: forecast.projected_30_days,
        projected_90_days: forecast.projected_90_days,
        projected_365_days: forecast.projected_365_days,
        time_to_double: forecast.time_to_double,
        time_to_bankruptcy: forecast.time_to_bankruptcy,
    })
}

/// Get AI prediction accuracy
#[tauri::command]
fn ai_trader_get_prediction_accuracy(state: State<AppState>) -> Result<AiAccuracyResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let accuracy = db.get_ai_prediction_accuracy().map_err(|e| e.to_string())?;

    Ok(AiAccuracyResponse {
        total_predictions: accuracy.total_predictions,
        accurate_predictions: accuracy.accurate_predictions,
        accuracy_percent: accuracy.accuracy_percent,
    })
}

/// Evaluate pending AI predictions that have reached their timeframe
#[tauri::command]
fn ai_trader_evaluate_predictions(state: State<AppState>) -> Result<u32, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;

    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;
    let trader = AiTrader::new(config);

    let evaluated = trader
        .evaluate_predictions(&mut db)
        .map_err(|e| e.to_string())?;

    if evaluated > 0 {
        println!("[AI Trader] Evaluated {} predictions", evaluated);
    }

    Ok(evaluated)
}

/// Reset AI trading (clear all data and start fresh)
#[tauri::command]
fn ai_trader_reset(
    state: State<AppState>,
    starting_capital: Option<f64>,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let capital = starting_capital.unwrap_or(1_000_000.0);
    db.reset_ai_trading(capital).map_err(|e| e.to_string())?;

    println!("[AI Trader] Reset with ${:.0} starting capital", capital);

    Ok(CommandResult {
        success: true,
        message: format!("AI trading reset with ${:.2} starting capital", capital),
    })
}

// ============================================================================
// Guardrails & Circuit Breaker Commands
// ============================================================================

/// Response for trading mode
#[derive(Debug, Serialize)]
struct TradingModeResponse {
    mode: String,
    max_position_pct: f64,
    max_daily_trades: i32,
    max_single_trade_value: f64,
    require_confluence: bool,
}

/// Response for circuit breaker status
#[derive(Debug, Serialize)]
struct CircuitBreakerResponse {
    daily_loss_threshold: f64,
    consecutive_loss_limit: i32,
    auto_conservative_on_trigger: bool,
    // Current state (from runtime, not DB)
    is_triggered: bool,
}

/// Response for trade rejection
#[derive(Debug, Serialize)]
struct TradeRejectionResponse {
    id: i64,
    timestamp: String,
    action: String,
    symbol: String,
    reason: String,
    rule_triggered: String,
}

/// Get current trading mode and guardrails
#[tauri::command]
fn ai_trader_get_mode(state: State<AppState>) -> Result<TradingModeResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;

    Ok(TradingModeResponse {
        mode: config.trading_mode,
        max_position_pct: config.max_position_size_percent,
        max_daily_trades: config.max_daily_trades,
        max_single_trade_value: config.max_single_trade_value,
        require_confluence: config.require_confluence,
    })
}

/// Switch trading mode
#[tauri::command]
fn ai_trader_switch_mode(
    state: State<AppState>,
    mode: String,
) -> Result<CommandResult, String> {
    let valid_modes = ["aggressive", "normal", "conservative", "paused"];
    if !valid_modes.contains(&mode.as_str()) {
        return Err(format!("Invalid mode: {}. Must be one of: {:?}", mode, valid_modes));
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_trading_mode(&mode).map_err(|e| e.to_string())?;

    println!("[AI Trader] Mode switched to: {}", mode);

    Ok(CommandResult {
        success: true,
        message: format!("Trading mode switched to: {}", mode),
    })
}

/// Get circuit breaker settings
#[tauri::command]
fn ai_trader_get_circuit_breaker(state: State<AppState>) -> Result<CircuitBreakerResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let config = db.get_ai_trader_config().map_err(|e| e.to_string())?;

    Ok(CircuitBreakerResponse {
        daily_loss_threshold: config.daily_loss_threshold,
        consecutive_loss_limit: config.consecutive_loss_limit,
        auto_conservative_on_trigger: config.auto_conservative_on_trigger,
        is_triggered: false, // Would need runtime state to know this
    })
}

/// Update circuit breaker settings
#[tauri::command]
fn ai_trader_update_circuit_breaker(
    state: State<AppState>,
    daily_loss_threshold: f64,
    consecutive_loss_limit: i32,
    auto_conservative: bool,
) -> Result<CommandResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.update_circuit_breaker_settings(daily_loss_threshold, consecutive_loss_limit, auto_conservative)
        .map_err(|e| e.to_string())?;

    println!("[AI Trader] Circuit breaker updated: threshold={:.1}%, consecutive_limit={}, auto_conservative={}",
        daily_loss_threshold, consecutive_loss_limit, auto_conservative);

    Ok(CommandResult {
        success: true,
        message: format!("Circuit breaker settings updated"),
    })
}

/// Get recent trade rejections
#[tauri::command]
fn ai_trader_get_rejections(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Vec<TradeRejectionResponse>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let rejections = db.get_trade_rejections(limit.unwrap_or(20))
        .map_err(|e| e.to_string())?;

    Ok(rejections.into_iter().map(|(id, timestamp, action, symbol, reason, rule)| {
        TradeRejectionResponse {
            id,
            timestamp,
            action,
            symbol,
            reason,
            rule_triggered: rule,
        }
    }).collect())
}

/// Get circuit breaker event history
#[tauri::command]
fn ai_trader_get_circuit_breaker_events(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let events = db.get_circuit_breaker_events(limit.unwrap_or(10))
        .map_err(|e| e.to_string())?;

    Ok(events.into_iter().map(|(id, timestamp, trigger_type, prev_mode, new_mode, daily_pnl)| {
        serde_json::json!({
            "id": id,
            "timestamp": timestamp,
            "trigger_type": trigger_type,
            "previous_mode": prev_mode,
            "new_mode": new_mode,
            "daily_pnl": daily_pnl,
        })
    }).collect())
}

/// Open a URL in a lightweight Tauri webview window
/// Security: Only HTTPS allowed, JavaScript sandboxed, reuses single window to save RAM
#[tauri::command]
async fn open_article_window(app: tauri::AppHandle, url: String, title: String) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder, Manager};

    // SECURITY: Only allow HTTPS URLs (no HTTP, no file://, no javascript:, etc.)
    let parsed_url: url::Url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;
    if parsed_url.scheme() != "https" {
        return Err("Only HTTPS URLs are allowed for security".to_string());
    }

    // SECURITY: Block potentially dangerous domains (can expand this list)
    let host = parsed_url.host_str().unwrap_or("");
    let blocked_patterns = ["localhost", "127.0.0.1", "0.0.0.0", "file://"];
    for pattern in blocked_patterns {
        if host.contains(pattern) {
            return Err("This URL is not allowed for security reasons".to_string());
        }
    }

    // Truncate title for window
    let window_title = if title.len() > 60 {
        format!("{}...", &title[..57])
    } else {
        title
    };

    // PERFORMANCE: Reuse single article-viewer window to reduce RAM usage
    // Close existing article window if it exists
    const ARTICLE_WINDOW_LABEL: &str = "article-viewer";
    if let Some(existing) = app.get_webview_window(ARTICLE_WINDOW_LABEL) {
        let _ = existing.close();
        // Small delay to ensure window closes
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    WebviewWindowBuilder::new(
        &app,
        ARTICLE_WINDOW_LABEL,
        WebviewUrl::External(parsed_url),
    )
    .title(&window_title)
    .inner_size(1000.0, 700.0)
    .min_inner_size(600.0, 400.0)
    .center()
    // SECURITY: Disable devtools in production
    .devtools(cfg!(debug_assertions))
    .build()
    .map_err(|e| format!("Failed to create window: {}", e))?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize database with absolute path to avoid CWD issues
    let db_path = get_data_path("finance.db");
    let db = Database::open(&db_path).expect("Failed to open database");
    db.init_schema().expect("Failed to initialize schema");

    tauri::Builder::default()
        .manage(AppState { db: Mutex::new(db) })
        .invoke_handler(tauri::generate_handler![
            get_symbols,
            toggle_favorite,
            get_favorited_symbols,
            favorite_dc_positions,
            favorite_paper_positions,
            fetch_prices,
            fetch_fred,
            get_macro_data,
            get_price,
            calculate_indicators,
            get_indicators,
            get_indicator_history,
            get_price_history,
            export_csv,
            search_symbol,
            add_alert,
            get_alerts,
            delete_alert,
            check_alerts,
            add_position,
            get_portfolio,
            delete_position,
            fetch_trends,
            get_trends,
            // Signal commands
            generate_signals,
            get_signals,
            get_all_signals,
            acknowledge_signal,
            acknowledge_all_signals,
            // Indicator alert commands
            add_indicator_alert,
            get_indicator_alerts,
            delete_indicator_alert,
            check_indicator_alerts,
            // Backtest commands
            save_strategy,
            get_strategies,
            delete_strategy,
            run_backtest,
            get_backtest_results,
            get_backtest_detail,
            delete_backtest,
            // Watchlist/Symbol Group commands
            create_watchlist,
            get_all_watchlists,
            get_watchlist_detail,
            delete_watchlist,
            add_symbol_to_watchlist,
            remove_symbol_from_watchlist,
            update_watchlist_description,
            rename_watchlist,
            // Vector database commands
            vector_search,
            add_market_event,
            add_price_pattern,
            get_vector_stats,
            // Claude AI commands
            claude_chat,
            claude_query,
            // Ollama local LLM commands
            ollama_available,
            ollama_sentiment,
            ollama_explain,
            ollama_ask,
            // Finnhub news commands
            fetch_news,
            fetch_price_reaction,
            fetch_candles,
            // Enhanced event saving with pattern linking
            add_market_event_with_pattern,
            // Article viewer
            open_article_window,
            // Paper trading commands
            get_paper_balance,
            get_paper_positions,
            execute_paper_trade,
            get_paper_trades,
            reset_paper_account,
            // DC trader commands
            get_dc_balance,
            get_dc_positions,
            execute_dc_trade,
            get_dc_trades,
            reset_dc_account,
            import_dc_trades_csv,
            import_dc_trades_json,
            lookup_current_price,
            record_portfolio_snapshot,
            get_portfolio_snapshots,
            save_team_config,
            load_team_config,
            list_team_configs,
            get_competition_stats,
            // AI trader commands
            ai_trader_get_status,
            ai_trader_get_config,
            ai_trader_start_session,
            ai_trader_end_session,
            ai_trader_run_cycle,
            ai_trader_get_decisions,
            ai_trader_get_performance_history,
            ai_trader_get_benchmark_comparison,
            ai_trader_get_compounding_forecast,
            ai_trader_get_prediction_accuracy,
            ai_trader_evaluate_predictions,
            ai_trader_reset,
            // Guardrails & circuit breaker commands
            ai_trader_get_mode,
            ai_trader_switch_mode,
            ai_trader_get_circuit_breaker,
            ai_trader_update_circuit_breaker,
            ai_trader_get_rejections,
            ai_trader_get_circuit_breaker_events,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
