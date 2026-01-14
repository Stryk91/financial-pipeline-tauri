//! Tauri GUI backend for Financial Pipeline

use financial_pipeline::{calculate_all, AlertCondition, Database, Fred, YahooFinance};
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

/// Application state holding the database connection
struct AppState {
    db: Mutex<Database>,
}

/// Symbol with latest price
#[derive(Serialize)]
struct SymbolPrice {
    symbol: String,
    price: f64,
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

/// Get all symbols with their latest prices
#[tauri::command]
fn get_symbols(state: State<AppState>) -> Result<Vec<SymbolPrice>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let symbols = db.get_symbols_with_data().map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for symbol in symbols {
        if let Ok(Some(price)) = db.get_latest_price(&symbol) {
            result.push(SymbolPrice { symbol, price });
        }
    }

    Ok(result)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize database
    let db = Database::open("data/finance.db").expect("Failed to open database");
    db.init_schema().expect("Failed to initialize schema");

    tauri::Builder::default()
        .manage(AppState { db: Mutex::new(db) })
        .invoke_handler(tauri::generate_handler![
            get_symbols,
            fetch_prices,
            fetch_fred,
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
