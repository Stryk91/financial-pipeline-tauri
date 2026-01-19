//! Tauri GUI backend for Financial Pipeline

use financial_pipeline::{
    calculate_all, AlertCondition, BacktestConfig, BacktestEngine, Database, Fred, GoogleTrends,
    IndicatorAlert, IndicatorAlertCondition, IndicatorAlertType, PositionType, SignalEngine,
    Strategy, StrategyConditionType, YahooFinance,
    VectorStore, MarketEvent, PricePattern,
    ClaudeClient, FinancialContext, PriceContext as ClaudePriceContext,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

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
    let store = VectorStore::new("../data/vectors.db").map_err(|e| e.to_string())?;

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
    let store = VectorStore::new("../data/vectors.db").map_err(|e| e.to_string())?;

    let event = MarketEvent {
        id: format!("{}-{}-{}", symbol, event_type, date),
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
    let store = VectorStore::new("../data/vectors.db").map_err(|e| e.to_string())?;

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

/// Get vector database statistics
#[tauri::command]
fn get_vector_stats() -> Result<VectorStatsResponse, String> {
    let store = VectorStore::new("../data/vectors.db").map_err(|e| e.to_string())?;

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
    let store = VectorStore::new("../data/vectors.db").map_err(|e| e.to_string())?;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize database
    // Use path outside src-tauri to avoid triggering hot-reload on DB changes
    let db = Database::open("../data/finance.db").expect("Failed to open database");
    db.init_schema().expect("Failed to initialize schema");

    tauri::Builder::default()
        .manage(AppState { db: Mutex::new(db) })
        .invoke_handler(tauri::generate_handler![
            get_symbols,
            toggle_favorite,
            get_favorited_symbols,
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
