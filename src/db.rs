//! SQLite database layer for Financial Pipeline

use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use std::path::Path;

use crate::error::Result;
use crate::models::{
    AlertCondition, BacktestResult, BacktestTrade, DailyPrice, IndicatorAlert,
    IndicatorAlertCondition, IndicatorAlertType, MacroData, PerformanceMetrics, Position,
    PositionType, PriceAlert, Signal, SignalDirection, SignalType, Strategy,
    StrategyConditionType, Symbol, TechnicalIndicator, TradeDirection,
    PaperWallet, PaperPosition, PaperTrade, PaperTradeAction,
    // AI Trading types
    AiTraderConfig, AiTradingSession, AiTradeDecision, AiPerformanceSnapshot, AiPredictionAccuracy,
    // DC Trader types
    DcWallet, DcPosition, DcTrade, PortfolioSnapshot, TeamConfig, ImportResult, CompetitionStats,
};
use crate::trends::TrendData;

/// Extension trait for pipe-style method chaining
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for Vec<T> {}

/// Database wrapper for financial data storage
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Ok(Self { conn })
    }

    /// Initialize database schema
    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA_SQL)?;
        // Run migrations for existing databases
        self.run_migrations()?;
        println!("[OK] Database schema initialized");
        Ok(())
    }

    /// Run database migrations for existing tables
    fn run_migrations(&self) -> Result<()> {
        // Add favorited column to symbols table if it doesn't exist
        let columns: Vec<String> = self
            .conn
            .prepare("PRAGMA table_info(symbols)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<SqliteResult<Vec<_>>>()?;

        if !columns.contains(&"favorited".to_string()) {
            self.conn.execute(
                "ALTER TABLE symbols ADD COLUMN favorited INTEGER DEFAULT 0",
                [],
            )?;
            println!("[MIGRATION] Added favorited column to symbols table");
        }

        // Migrate ai_trader_config table with new guardrails columns
        let ai_config_columns: Vec<String> = self
            .conn
            .prepare("PRAGMA table_info(ai_trader_config)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<SqliteResult<Vec<_>>>()?;

        // Add trading_mode column
        if !ai_config_columns.contains(&"trading_mode".to_string()) {
            self.conn.execute(
                "ALTER TABLE ai_trader_config ADD COLUMN trading_mode TEXT NOT NULL DEFAULT 'normal'",
                [],
            )?;
            println!("[MIGRATION] Added trading_mode column to ai_trader_config");
        }

        // Add circuit breaker columns
        if !ai_config_columns.contains(&"daily_loss_threshold".to_string()) {
            self.conn.execute_batch(r#"
                ALTER TABLE ai_trader_config ADD COLUMN daily_loss_threshold REAL NOT NULL DEFAULT -10.0;
                ALTER TABLE ai_trader_config ADD COLUMN consecutive_loss_limit INTEGER NOT NULL DEFAULT 5;
                ALTER TABLE ai_trader_config ADD COLUMN auto_conservative_on_trigger INTEGER NOT NULL DEFAULT 1;
                ALTER TABLE ai_trader_config ADD COLUMN circuit_breaker_triggered INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE ai_trader_config ADD COLUMN circuit_breaker_until TIMESTAMP;
            "#)?;
            println!("[MIGRATION] Added circuit breaker columns to ai_trader_config");
        }

        // Add override columns
        if !ai_config_columns.contains(&"override_enabled".to_string()) {
            self.conn.execute_batch(r#"
                ALTER TABLE ai_trader_config ADD COLUMN override_enabled INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE ai_trader_config ADD COLUMN override_expires_at TIMESTAMP;
                ALTER TABLE ai_trader_config ADD COLUMN override_max_position_pct REAL;
            "#)?;
            println!("[MIGRATION] Added override columns to ai_trader_config");
        }

        // Add guardrail columns
        if !ai_config_columns.contains(&"max_daily_trades".to_string()) {
            self.conn.execute_batch(r#"
                ALTER TABLE ai_trader_config ADD COLUMN max_daily_trades INTEGER NOT NULL DEFAULT 10;
                ALTER TABLE ai_trader_config ADD COLUMN max_single_trade_value REAL NOT NULL DEFAULT 50000.0;
                ALTER TABLE ai_trader_config ADD COLUMN require_confluence INTEGER NOT NULL DEFAULT 1;
                ALTER TABLE ai_trader_config ADD COLUMN blocked_hours TEXT DEFAULT '09:30-09:45,15:45-16:00';
            "#)?;
            println!("[MIGRATION] Added guardrail columns to ai_trader_config");
        }

        Ok(())
    }

    /// Insert or update a symbol
    pub fn upsert_symbol(&self, symbol: &Symbol) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO symbols
            (symbol, name, sector, industry, market_cap, country, exchange, currency, isin, asset_class)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                symbol.symbol,
                symbol.name,
                symbol.sector,
                symbol.industry,
                symbol.market_cap,
                symbol.country,
                symbol.exchange,
                symbol.currency,
                symbol.isin,
                symbol.asset_class,
            ],
        )?;
        Ok(())
    }

    /// Insert or update daily price data
    pub fn upsert_daily_price(&self, price: &DailyPrice) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO daily_prices
            (symbol, timestamp, open, high, low, close, volume, source)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                price.symbol,
                price.date.to_string(),
                price.open,
                price.high,
                price.low,
                price.close,
                price.volume,
                price.source,
            ],
        )?;
        Ok(())
    }

    /// Batch insert daily prices (more efficient)
    pub fn upsert_daily_prices(&mut self, prices: &[DailyPrice]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO daily_prices
                (symbol, timestamp, open, high, low, close, volume, source)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                "#,
            )?;

            for price in prices {
                stmt.execute(params![
                    price.symbol,
                    price.date.to_string(),
                    price.open,
                    price.high,
                    price.low,
                    price.close,
                    price.volume,
                    price.source,
                ])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Insert macro data
    pub fn upsert_macro_data(&self, data: &MacroData) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO macro_data (indicator, date, value, source)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![data.indicator, data.date.to_string(), data.value, data.source,],
        )?;
        Ok(())
    }

    /// Batch insert macro data
    pub fn upsert_macro_data_batch(&mut self, data: &[MacroData]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO macro_data (indicator, date, value, source)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )?;

            for d in data {
                stmt.execute(params![d.indicator, d.date.to_string(), d.value, d.source,])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Get macro data for an indicator (latest values)
    pub fn get_macro_data(&self, indicator: &str) -> Result<Vec<MacroData>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT indicator, date, value, source
            FROM macro_data
            WHERE indicator = ?1
            ORDER BY date DESC
            LIMIT 100
            "#,
        )?;

        let data = stmt
            .query_map(params![indicator], |row| {
                let date_str: String = row.get(1)?;
                Ok(MacroData {
                    indicator: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    value: row.get(2)?,
                    source: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(data)
    }

    /// Get all unique macro indicators
    pub fn get_macro_indicators(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT indicator FROM macro_data ORDER BY indicator
            "#,
        )?;

        let indicators = stmt
            .query_map([], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(indicators)
    }

    /// Get latest value for each macro indicator
    pub fn get_macro_summary(&self) -> Result<Vec<MacroData>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT m.indicator, m.date, m.value, m.source
            FROM macro_data m
            INNER JOIN (
                SELECT indicator, MAX(date) as max_date
                FROM macro_data
                GROUP BY indicator
            ) latest ON m.indicator = latest.indicator AND m.date = latest.max_date
            ORDER BY m.indicator
            "#,
        )?;

        let data = stmt
            .query_map([], |row| {
                let date_str: String = row.get(1)?;
                Ok(MacroData {
                    indicator: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    value: row.get(2)?,
                    source: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(data)
    }

    /// Log an API call
    pub fn log_api_call(&self, source: &str, endpoint: &str, symbol: &str) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO api_calls (source, endpoint, symbol, timestamp)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![source, endpoint, symbol, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Get latest price for a symbol
    pub fn get_latest_price(&self, symbol: &str) -> Result<Option<f64>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT close FROM daily_prices
            WHERE symbol = ?1
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )?;

        let result: SqliteResult<f64> = stmt.query_row(params![symbol], |row| row.get(0));

        match result {
            Ok(price) => Ok(Some(price)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all prices for a symbol
    pub fn get_prices(&self, symbol: &str) -> Result<Vec<DailyPrice>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT symbol, timestamp, open, high, low, close, volume, source
            FROM daily_prices
            WHERE symbol = ?1
            ORDER BY timestamp ASC
            "#,
        )?;

        let prices = stmt
            .query_map(params![symbol], |row| {
                let date_str: String = row.get(1)?;
                Ok(DailyPrice {
                    symbol: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    open: row.get(2)?,
                    high: row.get(3)?,
                    low: row.get(4)?,
                    close: row.get(5)?,
                    volume: row.get(6)?,
                    source: row.get(7)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(prices)
    }

    /// Get all symbols with price data
    pub fn get_symbols_with_data(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT symbol FROM daily_prices")?;
        let symbols = stmt
            .query_map([], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;
        Ok(symbols)
    }

    /// Clear price data for a symbol
    pub fn clear_symbol_prices(&self, symbol: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM daily_prices WHERE symbol = ?1",
            params![symbol],
        )?;
        println!("[OK] Cleared price data for {}", symbol);
        Ok(())
    }

    /// Toggle symbol favorite status
    pub fn toggle_symbol_favorite(&self, symbol: &str) -> Result<bool> {
        // First ensure the symbol exists in the symbols table
        self.conn.execute(
            "INSERT OR IGNORE INTO symbols (symbol, favorited) VALUES (?1, 0)",
            params![symbol],
        )?;

        // Toggle the favorite status
        self.conn.execute(
            "UPDATE symbols SET favorited = CASE WHEN favorited = 1 THEN 0 ELSE 1 END WHERE symbol = ?1",
            params![symbol],
        )?;

        // Return new state
        let favorited: i32 = self.conn.query_row(
            "SELECT favorited FROM symbols WHERE symbol = ?1",
            params![symbol],
            |row| row.get(0),
        )?;

        Ok(favorited == 1)
    }

    /// Get favorite status for a symbol
    pub fn is_symbol_favorited(&self, symbol: &str) -> Result<bool> {
        let result: Option<i32> = self
            .conn
            .query_row(
                "SELECT favorited FROM symbols WHERE symbol = ?1",
                params![symbol],
                |row| row.get(0),
            )
            .optional()?;

        Ok(result.unwrap_or(0) == 1)
    }

    /// Get all favorited symbols
    pub fn get_favorited_symbols(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT symbol FROM symbols WHERE favorited = 1"
        )?;
        let symbols = stmt
            .query_map([], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;
        Ok(symbols)
    }

    /// Set a symbol as favorited (for auto-refresh)
    pub fn set_symbol_favorited(&self, symbol: &str, favorited: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO symbols (symbol, favorited) VALUES (?1, ?2)",
            params![symbol, favorited as i32],
        )?;
        Ok(())
    }

    /// Favorite all DC position symbols for auto-refresh
    pub fn favorite_dc_positions(&self) -> Result<Vec<String>> {
        let positions = self.get_dc_positions()?;
        let mut symbols = Vec::new();
        for pos in positions {
            self.set_symbol_favorited(&pos.symbol, true)?;
            symbols.push(pos.symbol);
        }
        Ok(symbols)
    }

    /// Favorite all paper (KALIC) position symbols for auto-refresh
    pub fn favorite_paper_positions(&self) -> Result<Vec<String>> {
        let positions = self.get_paper_positions()?;
        let mut symbols = Vec::new();
        for pos in positions {
            self.set_symbol_favorited(&pos.symbol, true)?;
            symbols.push(pos.symbol);
        }
        Ok(symbols)
    }

    /// Create a watchlist
    pub fn create_watchlist(
        &self,
        name: &str,
        symbols: &[String],
        description: Option<&str>,
    ) -> Result<i64> {
        // Delete existing watchlist entries
        self.conn
            .execute("DELETE FROM watchlists WHERE name = ?1", params![name])?;

        // Create watchlist
        self.conn.execute(
            "INSERT INTO watchlists (name, description) VALUES (?1, ?2)",
            params![name, description],
        )?;

        let watchlist_id = self.conn.last_insert_rowid();

        // Add symbols
        let mut stmt = self
            .conn
            .prepare("INSERT INTO watchlist_symbols (watchlist_id, symbol) VALUES (?1, ?2)")?;

        for symbol in symbols {
            stmt.execute(params![watchlist_id, symbol])?;
        }

        Ok(watchlist_id)
    }

    /// Get symbols in a watchlist
    pub fn get_watchlist(&self, name: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT ws.symbol
            FROM watchlists w
            JOIN watchlist_symbols ws ON w.id = ws.watchlist_id
            WHERE w.name = ?1
            "#,
        )?;

        let symbols = stmt
            .query_map(params![name], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(symbols)
    }

    /// Get all watchlists with their details
    pub fn get_all_watchlists(&self) -> Result<Vec<(i64, String, Option<String>, i64)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT w.id, w.name, w.description, COUNT(ws.symbol) as symbol_count
            FROM watchlists w
            LEFT JOIN watchlist_symbols ws ON w.id = ws.watchlist_id
            GROUP BY w.id, w.name, w.description
            ORDER BY w.name
            "#,
        )?;

        let watchlists = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(watchlists)
    }

    /// Get watchlist with full info (name, description, symbols)
    pub fn get_watchlist_full(&self, name: &str) -> Result<Option<(i64, String, Option<String>, Vec<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description FROM watchlists WHERE name = ?1",
        )?;

        let result: Option<(i64, String, Option<String>)> = stmt
            .query_row(params![name], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .optional()?;

        match result {
            Some((id, wl_name, description)) => {
                let symbols = self.get_watchlist(&wl_name)?;
                Ok(Some((id, wl_name, description, symbols)))
            }
            None => Ok(None),
        }
    }

    /// Delete a watchlist by name
    pub fn delete_watchlist(&self, name: &str) -> Result<bool> {
        // Get the watchlist ID first
        let watchlist_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM watchlists WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;

        match watchlist_id {
            Some(id) => {
                // Delete symbols first (foreign key)
                self.conn.execute(
                    "DELETE FROM watchlist_symbols WHERE watchlist_id = ?1",
                    params![id],
                )?;
                // Delete watchlist
                self.conn.execute(
                    "DELETE FROM watchlists WHERE id = ?1",
                    params![id],
                )?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Add a symbol to an existing watchlist
    pub fn add_symbol_to_watchlist(&self, watchlist_name: &str, symbol: &str) -> Result<bool> {
        let watchlist_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM watchlists WHERE name = ?1",
                params![watchlist_name],
                |row| row.get(0),
            )
            .optional()?;

        match watchlist_id {
            Some(id) => {
                // Check if symbol already exists
                let exists: bool = self
                    .conn
                    .query_row(
                        "SELECT 1 FROM watchlist_symbols WHERE watchlist_id = ?1 AND symbol = ?2",
                        params![id, symbol],
                        |_| Ok(true),
                    )
                    .optional()?
                    .unwrap_or(false);

                if !exists {
                    self.conn.execute(
                        "INSERT INTO watchlist_symbols (watchlist_id, symbol) VALUES (?1, ?2)",
                        params![id, symbol],
                    )?;
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Remove a symbol from a watchlist
    pub fn remove_symbol_from_watchlist(&self, watchlist_name: &str, symbol: &str) -> Result<bool> {
        let watchlist_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM watchlists WHERE name = ?1",
                params![watchlist_name],
                |row| row.get(0),
            )
            .optional()?;

        match watchlist_id {
            Some(id) => {
                let deleted = self.conn.execute(
                    "DELETE FROM watchlist_symbols WHERE watchlist_id = ?1 AND symbol = ?2",
                    params![id, symbol],
                )?;
                Ok(deleted > 0)
            }
            None => Ok(false),
        }
    }

    /// Update watchlist description
    pub fn update_watchlist_description(&self, name: &str, description: Option<&str>) -> Result<bool> {
        let updated = self.conn.execute(
            "UPDATE watchlists SET description = ?1 WHERE name = ?2",
            params![description, name],
        )?;
        Ok(updated > 0)
    }

    /// Rename a watchlist
    pub fn rename_watchlist(&self, old_name: &str, new_name: &str) -> Result<bool> {
        let updated = self.conn.execute(
            "UPDATE watchlists SET name = ?1 WHERE name = ?2",
            params![new_name, old_name],
        )?;
        Ok(updated > 0)
    }

    /// Vacuum/optimize the database
    pub fn vacuum(&self) -> Result<()> {
        self.conn.execute_batch("VACUUM; ANALYZE;")?;
        println!("[OK] Database optimized");
        Ok(())
    }

    /// Store a technical indicator value
    pub fn upsert_indicator(&self, ind: &TechnicalIndicator) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO technical_indicators
            (symbol, timestamp, indicator_name, value)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![ind.symbol, ind.date.to_string(), ind.indicator_name, ind.value],
        )?;
        Ok(())
    }

    /// Batch store indicators
    pub fn upsert_indicators(&mut self, indicators: &[TechnicalIndicator]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO technical_indicators
                (symbol, timestamp, indicator_name, value)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )?;

            for ind in indicators {
                stmt.execute(params![
                    ind.symbol,
                    ind.date.to_string(),
                    ind.indicator_name,
                    ind.value
                ])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Get latest indicators for a symbol
    pub fn get_latest_indicators(&self, symbol: &str) -> Result<Vec<TechnicalIndicator>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.symbol, t.timestamp, t.indicator_name, t.value
            FROM technical_indicators t
            INNER JOIN (
                SELECT symbol, indicator_name, MAX(timestamp) as max_date
                FROM technical_indicators
                WHERE symbol = ?1
                GROUP BY symbol, indicator_name
            ) latest ON t.symbol = latest.symbol
                AND t.indicator_name = latest.indicator_name
                AND t.timestamp = latest.max_date
            "#,
        )?;

        let indicators = stmt
            .query_map(params![symbol], |row| {
                let date_str: String = row.get(1)?;
                Ok(TechnicalIndicator {
                    symbol: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    indicator_name: row.get(2)?,
                    value: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(indicators)
    }

    /// Get indicator history for a symbol
    pub fn get_indicator_history(
        &self,
        symbol: &str,
        indicator_name: &str,
    ) -> Result<Vec<TechnicalIndicator>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT symbol, timestamp, indicator_name, value
            FROM technical_indicators
            WHERE symbol = ?1 AND indicator_name = ?2
            ORDER BY timestamp ASC
            "#,
        )?;

        let indicators = stmt
            .query_map(params![symbol, indicator_name], |row| {
                let date_str: String = row.get(1)?;
                Ok(TechnicalIndicator {
                    symbol: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    indicator_name: row.get(2)?,
                    value: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(indicators)
    }

    /// Add a price alert
    pub fn add_alert(&self, symbol: &str, target_price: f64, condition: AlertCondition) -> Result<i64> {
        let condition_str = match condition {
            AlertCondition::Above => "above",
            AlertCondition::Below => "below",
        };

        self.conn.execute(
            r#"
            INSERT INTO price_alerts (symbol, target_price, condition)
            VALUES (?1, ?2, ?3)
            "#,
            params![symbol, target_price, condition_str],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get all alerts (optionally filter by triggered status)
    pub fn get_alerts(&self, only_active: bool) -> Result<Vec<PriceAlert>> {
        let sql = if only_active {
            "SELECT id, symbol, target_price, condition, triggered, created_at FROM price_alerts WHERE triggered = 0 ORDER BY created_at DESC"
        } else {
            "SELECT id, symbol, target_price, condition, triggered, created_at FROM price_alerts ORDER BY created_at DESC"
        };

        let mut stmt = self.conn.prepare(sql)?;

        let alerts = stmt
            .query_map([], |row| {
                let condition_str: String = row.get(3)?;
                let condition = if condition_str == "above" {
                    AlertCondition::Above
                } else {
                    AlertCondition::Below
                };

                Ok(PriceAlert {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    target_price: row.get(2)?,
                    condition,
                    triggered: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(alerts)
    }

    /// Delete an alert
    pub fn delete_alert(&self, alert_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM price_alerts WHERE id = ?1", params![alert_id])?;
        Ok(())
    }

    /// Mark an alert as triggered
    pub fn trigger_alert(&self, alert_id: i64) -> Result<()> {
        self.conn.execute("UPDATE price_alerts SET triggered = 1 WHERE id = ?1", params![alert_id])?;
        Ok(())
    }

    /// Check alerts against current prices, returns triggered alerts
    pub fn check_alerts(&self) -> Result<Vec<PriceAlert>> {
        let alerts = self.get_alerts(true)?;
        let mut triggered = Vec::new();

        for alert in alerts {
            if let Ok(Some(current_price)) = self.get_latest_price(&alert.symbol) {
                let should_trigger = match alert.condition {
                    AlertCondition::Above => current_price >= alert.target_price,
                    AlertCondition::Below => current_price <= alert.target_price,
                };

                if should_trigger {
                    self.trigger_alert(alert.id)?;
                    triggered.push(PriceAlert {
                        triggered: true,
                        ..alert
                    });
                }
            }
        }

        Ok(triggered)
    }

    /// Add a portfolio position
    pub fn add_position(
        &self,
        symbol: &str,
        quantity: f64,
        price: f64,
        position_type: PositionType,
        date: &str,
        notes: Option<&str>,
    ) -> Result<i64> {
        let type_str = match position_type {
            PositionType::Buy => "buy",
            PositionType::Sell => "sell",
        };

        self.conn.execute(
            r#"
            INSERT INTO portfolio_positions (symbol, quantity, price, position_type, date, notes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![symbol, quantity, price, type_str, date, notes],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get all portfolio positions
    pub fn get_positions(&self) -> Result<Vec<Position>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, quantity, price, position_type, date, notes
            FROM portfolio_positions
            ORDER BY date DESC
            "#,
        )?;

        let positions = stmt
            .query_map([], |row| {
                let type_str: String = row.get(4)?;
                let position_type = if type_str == "buy" {
                    PositionType::Buy
                } else {
                    PositionType::Sell
                };

                Ok(Position {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    quantity: row.get(2)?,
                    price: row.get(3)?,
                    position_type,
                    date: row.get(5)?,
                    notes: row.get(6)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(positions)
    }

    /// Delete a portfolio position
    pub fn delete_position(&self, position_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM portfolio_positions WHERE id = ?1",
            params![position_id],
        )?;
        Ok(())
    }

    /// Store Google Trends data
    pub fn upsert_trends(&mut self, data: &[TrendData]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO trends_data (keyword, date, value)
                VALUES (?1, ?2, ?3)
                "#,
            )?;

            for point in data {
                stmt.execute(params![point.keyword, point.date.to_string(), point.value])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Get trends data for a keyword
    pub fn get_trends(&self, keyword: &str) -> Result<Vec<TrendData>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT keyword, date, value
            FROM trends_data
            WHERE keyword = ?1
            ORDER BY date ASC
            "#,
        )?;

        let trends = stmt
            .query_map(params![keyword], |row| {
                let date_str: String = row.get(1)?;
                Ok(TrendData {
                    keyword: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    value: row.get(2)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(trends)
    }

    // ========================================================================
    // Signal Methods
    // ========================================================================

    /// Store a signal
    pub fn upsert_signal(&self, signal: &Signal) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO signals
            (symbol, signal_type, direction, strength, price_at_signal,
             triggered_by, trigger_value, timestamp, acknowledged)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                signal.symbol,
                signal.signal_type.as_str(),
                signal.direction.as_str(),
                signal.strength,
                signal.price_at_signal,
                signal.triggered_by,
                signal.trigger_value,
                signal.timestamp.to_string(),
                signal.acknowledged,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Batch store signals
    pub fn upsert_signals(&mut self, signals: &[Signal]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO signals
                (symbol, signal_type, direction, strength, price_at_signal,
                 triggered_by, trigger_value, timestamp, acknowledged)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )?;

            for signal in signals {
                stmt.execute(params![
                    signal.symbol,
                    signal.signal_type.as_str(),
                    signal.direction.as_str(),
                    signal.strength,
                    signal.price_at_signal,
                    signal.triggered_by,
                    signal.trigger_value,
                    signal.timestamp.to_string(),
                    signal.acknowledged,
                ])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Get signals for a symbol
    pub fn get_signals(&self, symbol: &str, only_unacknowledged: bool) -> Result<Vec<Signal>> {
        let sql = if only_unacknowledged {
            r#"
            SELECT id, symbol, signal_type, direction, strength, price_at_signal,
                   triggered_by, trigger_value, timestamp, created_at, acknowledged
            FROM signals
            WHERE symbol = ?1 AND acknowledged = 0
            ORDER BY timestamp DESC
            "#
        } else {
            r#"
            SELECT id, symbol, signal_type, direction, strength, price_at_signal,
                   triggered_by, trigger_value, timestamp, created_at, acknowledged
            FROM signals
            WHERE symbol = ?1
            ORDER BY timestamp DESC
            "#
        };

        let mut stmt = self.conn.prepare(sql)?;

        let signals = stmt
            .query_map(params![symbol], |row| {
                let signal_type_str: String = row.get(2)?;
                let direction_str: String = row.get(3)?;
                let date_str: String = row.get(8)?;

                Ok(Signal {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    signal_type: SignalType::from_str(&signal_type_str)
                        .unwrap_or(SignalType::RsiOversold),
                    direction: SignalDirection::from_str(&direction_str),
                    strength: row.get(4)?,
                    price_at_signal: row.get(5)?,
                    triggered_by: row.get(6)?,
                    trigger_value: row.get(7)?,
                    timestamp: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    created_at: row.get(9)?,
                    acknowledged: row.get(10)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(signals)
    }

    /// Get recent signals across all symbols
    pub fn get_recent_signals(&self, limit: usize) -> Result<Vec<Signal>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, signal_type, direction, strength, price_at_signal,
                   triggered_by, trigger_value, timestamp, created_at, acknowledged
            FROM signals
            ORDER BY timestamp DESC, strength DESC
            LIMIT ?1
            "#,
        )?;

        let signals = stmt
            .query_map(params![limit as i64], |row| {
                let signal_type_str: String = row.get(2)?;
                let direction_str: String = row.get(3)?;
                let date_str: String = row.get(8)?;

                Ok(Signal {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    signal_type: SignalType::from_str(&signal_type_str)
                        .unwrap_or(SignalType::RsiOversold),
                    direction: SignalDirection::from_str(&direction_str),
                    strength: row.get(4)?,
                    price_at_signal: row.get(5)?,
                    triggered_by: row.get(6)?,
                    trigger_value: row.get(7)?,
                    timestamp: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    created_at: row.get(9)?,
                    acknowledged: row.get(10)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(signals)
    }

    /// Acknowledge a signal
    pub fn acknowledge_signal(&self, signal_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE signals SET acknowledged = 1 WHERE id = ?1",
            params![signal_id],
        )?;
        Ok(())
    }

    /// Acknowledge all signals for a symbol
    pub fn acknowledge_all_signals(&self, symbol: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE signals SET acknowledged = 1 WHERE symbol = ?1",
            params![symbol],
        )?;
        Ok(())
    }

    /// Delete old signals (cleanup)
    pub fn cleanup_old_signals(&self, days: i64) -> Result<usize> {
        let deleted = self.conn.execute(
            "DELETE FROM signals WHERE timestamp < date('now', ?1)",
            params![format!("-{} days", days)],
        )?;
        Ok(deleted)
    }

    /// Get all indicators for a symbol (for signal generation)
    pub fn get_all_indicators(&self, symbol: &str) -> Result<Vec<TechnicalIndicator>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT symbol, timestamp, indicator_name, value
            FROM technical_indicators
            WHERE symbol = ?1
            ORDER BY timestamp ASC
            "#,
        )?;

        let indicators = stmt
            .query_map(params![symbol], |row| {
                let date_str: String = row.get(1)?;
                Ok(TechnicalIndicator {
                    symbol: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    indicator_name: row.get(2)?,
                    value: row.get(3)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(indicators)
    }

    // ========================================================================
    // Indicator Alert Methods
    // ========================================================================

    /// Add an indicator alert
    pub fn add_indicator_alert(&self, alert: &IndicatorAlert) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO indicator_alerts
            (symbol, alert_type, indicator_name, secondary_indicator, condition, threshold, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                alert.symbol,
                alert.alert_type.as_str(),
                alert.indicator_name,
                alert.secondary_indicator,
                alert.condition.as_str(),
                alert.threshold,
                alert.message,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get all indicator alerts
    pub fn get_indicator_alerts(&self, only_active: bool) -> Result<Vec<IndicatorAlert>> {
        let sql = if only_active {
            r#"
            SELECT id, symbol, alert_type, indicator_name, secondary_indicator,
                   condition, threshold, triggered, last_value, created_at, message
            FROM indicator_alerts
            WHERE triggered = 0
            ORDER BY created_at DESC
            "#
        } else {
            r#"
            SELECT id, symbol, alert_type, indicator_name, secondary_indicator,
                   condition, threshold, triggered, last_value, created_at, message
            FROM indicator_alerts
            ORDER BY created_at DESC
            "#
        };

        let mut stmt = self.conn.prepare(sql)?;

        let alerts = stmt
            .query_map([], |row| {
                let alert_type_str: String = row.get(2)?;
                let condition_str: String = row.get(5)?;

                Ok(IndicatorAlert {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    alert_type: IndicatorAlertType::from_str(&alert_type_str)
                        .unwrap_or(IndicatorAlertType::Threshold),
                    indicator_name: row.get(3)?,
                    secondary_indicator: row.get(4)?,
                    condition: IndicatorAlertCondition::from_str(&condition_str)
                        .unwrap_or(IndicatorAlertCondition::CrossesAbove),
                    threshold: row.get(6)?,
                    triggered: row.get(7)?,
                    last_value: row.get(8)?,
                    created_at: row.get(9)?,
                    message: row.get(10)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(alerts)
    }

    /// Delete an indicator alert
    pub fn delete_indicator_alert(&self, alert_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM indicator_alerts WHERE id = ?1",
            params![alert_id],
        )?;
        Ok(())
    }

    /// Mark an indicator alert as triggered
    pub fn trigger_indicator_alert(&self, alert_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE indicator_alerts SET triggered = 1 WHERE id = ?1",
            params![alert_id],
        )?;
        Ok(())
    }

    /// Update last_value for an indicator alert
    pub fn update_indicator_alert_state(&self, alert_id: i64, last_value: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE indicator_alerts SET last_value = ?1 WHERE id = ?2",
            params![last_value, alert_id],
        )?;
        Ok(())
    }

    /// Get the latest value for a specific indicator
    pub fn get_latest_indicator_value(&self, symbol: &str, indicator_name: &str) -> Result<Option<f64>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT value FROM technical_indicators
            WHERE symbol = ?1 AND indicator_name = ?2
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )?;

        let result: SqliteResult<f64> = stmt.query_row(params![symbol, indicator_name], |row| row.get(0));

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get the previous (second-to-last) indicator value
    pub fn get_previous_indicator_value(&self, symbol: &str, indicator_name: &str) -> Result<Option<f64>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT value FROM technical_indicators
            WHERE symbol = ?1 AND indicator_name = ?2
            ORDER BY timestamp DESC
            LIMIT 1 OFFSET 1
            "#,
        )?;

        let result: SqliteResult<f64> = stmt.query_row(params![symbol, indicator_name], |row| row.get(0));

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Check all indicator alerts, returns triggered alerts
    pub fn check_indicator_alerts(&self) -> Result<Vec<IndicatorAlert>> {
        let alerts = self.get_indicator_alerts(true)?;
        let mut triggered_alerts = Vec::new();

        for alert in alerts {
            let current = self.get_latest_indicator_value(&alert.symbol, &alert.indicator_name)?;
            let previous = alert.last_value.or_else(|| {
                self.get_previous_indicator_value(&alert.symbol, &alert.indicator_name).ok().flatten()
            });

            let Some(current_val) = current else {
                continue;
            };

            let should_trigger = match alert.condition {
                IndicatorAlertCondition::CrossesAbove => {
                    if let (Some(prev), Some(threshold)) = (previous, alert.threshold) {
                        prev < threshold && current_val >= threshold
                    } else {
                        false
                    }
                }
                IndicatorAlertCondition::CrossesBelow => {
                    if let (Some(prev), Some(threshold)) = (previous, alert.threshold) {
                        prev > threshold && current_val <= threshold
                    } else {
                        false
                    }
                }
                IndicatorAlertCondition::BullishCrossover => {
                    if let Some(secondary) = &alert.secondary_indicator {
                        let secondary_current = self.get_latest_indicator_value(&alert.symbol, secondary)?;
                        let secondary_prev = self.get_previous_indicator_value(&alert.symbol, secondary)?;

                        match (previous, secondary_current, secondary_prev) {
                            (Some(prev_primary), Some(curr_sec), Some(prev_sec)) => {
                                prev_primary <= prev_sec && current_val > curr_sec
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                IndicatorAlertCondition::BearishCrossover => {
                    if let Some(secondary) = &alert.secondary_indicator {
                        let secondary_current = self.get_latest_indicator_value(&alert.symbol, secondary)?;
                        let secondary_prev = self.get_previous_indicator_value(&alert.symbol, secondary)?;

                        match (previous, secondary_current, secondary_prev) {
                            (Some(prev_primary), Some(curr_sec), Some(prev_sec)) => {
                                prev_primary >= prev_sec && current_val < curr_sec
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
            };

            if should_trigger {
                self.trigger_indicator_alert(alert.id)?;
                triggered_alerts.push(IndicatorAlert {
                    triggered: true,
                    ..alert
                });
            } else {
                // Update last_value for next check
                self.update_indicator_alert_state(alert.id, current_val)?;
            }
        }

        Ok(triggered_alerts)
    }

    // ========================================================================
    // Backtest Methods
    // ========================================================================

    /// Save a strategy
    pub fn save_strategy(&self, strategy: &Strategy) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO strategies
            (name, description, entry_condition, entry_threshold,
             exit_condition, exit_threshold,
             stop_loss_percent, take_profit_percent, position_size_percent)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                strategy.name,
                strategy.description,
                strategy.entry_condition.as_str(),
                strategy.entry_threshold,
                strategy.exit_condition.as_str(),
                strategy.exit_threshold,
                strategy.stop_loss_percent,
                strategy.take_profit_percent,
                strategy.position_size_percent,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get all strategies
    pub fn get_strategies(&self) -> Result<Vec<Strategy>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, name, description, entry_condition, entry_threshold,
                   exit_condition, exit_threshold,
                   stop_loss_percent, take_profit_percent, position_size_percent, created_at
            FROM strategies
            ORDER BY name ASC
            "#,
        )?;

        let strategies = stmt
            .query_map([], |row| {
                let entry_cond_str: String = row.get(3)?;
                let exit_cond_str: String = row.get(5)?;

                Ok(Strategy {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    entry_condition: StrategyConditionType::from_str(&entry_cond_str)
                        .unwrap_or(StrategyConditionType::RsiOversold),
                    entry_threshold: row.get(4)?,
                    exit_condition: StrategyConditionType::from_str(&exit_cond_str)
                        .unwrap_or(StrategyConditionType::RsiOverbought),
                    exit_threshold: row.get(6)?,
                    stop_loss_percent: row.get(7)?,
                    take_profit_percent: row.get(8)?,
                    position_size_percent: row.get(9)?,
                    created_at: row.get(10)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(strategies)
    }

    /// Get a strategy by name
    pub fn get_strategy(&self, name: &str) -> Result<Option<Strategy>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, name, description, entry_condition, entry_threshold,
                   exit_condition, exit_threshold,
                   stop_loss_percent, take_profit_percent, position_size_percent, created_at
            FROM strategies
            WHERE name = ?1
            "#,
        )?;

        let result = stmt.query_row(params![name], |row| {
            let entry_cond_str: String = row.get(3)?;
            let exit_cond_str: String = row.get(5)?;

            Ok(Strategy {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                entry_condition: StrategyConditionType::from_str(&entry_cond_str)
                    .unwrap_or(StrategyConditionType::RsiOversold),
                entry_threshold: row.get(4)?,
                exit_condition: StrategyConditionType::from_str(&exit_cond_str)
                    .unwrap_or(StrategyConditionType::RsiOverbought),
                exit_threshold: row.get(6)?,
                stop_loss_percent: row.get(7)?,
                take_profit_percent: row.get(8)?,
                position_size_percent: row.get(9)?,
                created_at: row.get(10)?,
            })
        });

        match result {
            Ok(strategy) => Ok(Some(strategy)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a strategy
    pub fn delete_strategy(&self, name: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM strategies WHERE name = ?1", params![name])?;
        Ok(())
    }

    /// Save a backtest result
    pub fn save_backtest_result(&self, result: &BacktestResult) -> Result<i64> {
        let tx = self.conn.unchecked_transaction()?;

        // Insert the backtest run
        tx.execute(
            r#"
            INSERT INTO backtest_runs
            (strategy_id, strategy_name, symbol, start_date, end_date,
             initial_capital, final_capital, total_return, total_return_dollars,
             max_drawdown, sharpe_ratio, win_rate, total_trades, winning_trades,
             losing_trades, avg_win_percent, avg_loss_percent, profit_factor,
             avg_trade_duration_days)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            "#,
            params![
                result.strategy_id,
                result.strategy_name,
                result.symbol,
                result.start_date.to_string(),
                result.end_date.to_string(),
                result.initial_capital,
                result.final_capital,
                result.metrics.total_return,
                result.metrics.total_return_dollars,
                result.metrics.max_drawdown,
                result.metrics.sharpe_ratio,
                result.metrics.win_rate,
                result.metrics.total_trades as i64,
                result.metrics.winning_trades as i64,
                result.metrics.losing_trades as i64,
                result.metrics.avg_win_percent,
                result.metrics.avg_loss_percent,
                result.metrics.profit_factor,
                result.metrics.avg_trade_duration_days,
            ],
        )?;

        let backtest_id = tx.last_insert_rowid();

        // Insert trades
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO backtest_trades
                (backtest_id, symbol, direction, entry_date, entry_price, entry_reason,
                 exit_date, exit_price, exit_reason, shares, profit_loss, profit_loss_percent)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
            )?;

            for trade in &result.trades {
                stmt.execute(params![
                    backtest_id,
                    trade.symbol,
                    trade.direction.as_str(),
                    trade.entry_date.to_string(),
                    trade.entry_price,
                    trade.entry_reason,
                    trade.exit_date.map(|d| d.to_string()),
                    trade.exit_price,
                    trade.exit_reason,
                    trade.shares,
                    trade.profit_loss,
                    trade.profit_loss_percent,
                ])?;
            }
        }

        tx.commit()?;
        Ok(backtest_id)
    }

    /// Get backtest history
    pub fn get_backtest_results(
        &self,
        strategy_name: Option<&str>,
        symbol: Option<&str>,
        limit: usize,
    ) -> Result<Vec<BacktestResult>> {
        let mut sql = String::from(
            r#"
            SELECT id, strategy_id, strategy_name, symbol, start_date, end_date,
                   initial_capital, final_capital, total_return, total_return_dollars,
                   max_drawdown, sharpe_ratio, win_rate, total_trades, winning_trades,
                   losing_trades, avg_win_percent, avg_loss_percent, profit_factor,
                   avg_trade_duration_days, created_at
            FROM backtest_runs
            WHERE 1=1
            "#,
        );

        if strategy_name.is_some() {
            sql.push_str(" AND strategy_name = ?1");
        }
        if symbol.is_some() {
            sql.push_str(if strategy_name.is_some() {
                " AND symbol = ?2"
            } else {
                " AND symbol = ?1"
            });
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ?");

        let mut stmt = self.conn.prepare(&sql)?;

        let results: Vec<BacktestResult> = match (strategy_name, symbol) {
            (Some(strat), Some(sym)) => {
                stmt.query_map(params![strat, sym, limit as i64], |row| self.map_backtest_row(row))?
                    .collect::<SqliteResult<Vec<_>>>()?
            }
            (Some(strat), None) => {
                stmt.query_map(params![strat, limit as i64], |row| self.map_backtest_row(row))?
                    .collect::<SqliteResult<Vec<_>>>()?
            }
            (None, Some(sym)) => {
                stmt.query_map(params![sym, limit as i64], |row| self.map_backtest_row(row))?
                    .collect::<SqliteResult<Vec<_>>>()?
            }
            (None, None) => {
                stmt.query_map(params![limit as i64], |row| self.map_backtest_row(row))?
                    .collect::<SqliteResult<Vec<_>>>()?
            }
        };

        Ok(results)
    }

    fn map_backtest_row(&self, row: &rusqlite::Row) -> SqliteResult<BacktestResult> {
        let start_str: String = row.get(4)?;
        let end_str: String = row.get(5)?;
        let total_trades_i64: i64 = row.get(13)?;
        let winning_trades_i64: i64 = row.get(14)?;
        let losing_trades_i64: i64 = row.get(15)?;

        Ok(BacktestResult {
            id: row.get(0)?,
            strategy_id: row.get(1)?,
            strategy_name: row.get(2)?,
            symbol: row.get(3)?,
            start_date: NaiveDate::parse_from_str(&start_str, "%Y-%m-%d")
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
            end_date: NaiveDate::parse_from_str(&end_str, "%Y-%m-%d")
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
            initial_capital: row.get(6)?,
            final_capital: row.get(7)?,
            metrics: PerformanceMetrics {
                total_return: row.get(8)?,
                total_return_dollars: row.get(9)?,
                max_drawdown: row.get(10)?,
                sharpe_ratio: row.get(11)?,
                win_rate: row.get(12)?,
                total_trades: total_trades_i64 as usize,
                winning_trades: winning_trades_i64 as usize,
                losing_trades: losing_trades_i64 as usize,
                avg_win_percent: row.get(16)?,
                avg_loss_percent: row.get(17)?,
                profit_factor: row.get(18)?,
                avg_trade_duration_days: row.get(19)?,
            },
            trades: Vec::new(), // Trades loaded separately if needed
            created_at: row.get(20)?,
        })
    }

    /// Get backtest detail with trades
    pub fn get_backtest_detail(&self, backtest_id: i64) -> Result<Option<BacktestResult>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, strategy_id, strategy_name, symbol, start_date, end_date,
                   initial_capital, final_capital, total_return, total_return_dollars,
                   max_drawdown, sharpe_ratio, win_rate, total_trades, winning_trades,
                   losing_trades, avg_win_percent, avg_loss_percent, profit_factor,
                   avg_trade_duration_days, created_at
            FROM backtest_runs
            WHERE id = ?1
            "#,
        )?;

        let result = stmt.query_row(params![backtest_id], |row| self.map_backtest_row(row));

        let mut backtest = match result {
            Ok(b) => b,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        // Load trades
        let mut trade_stmt = self.conn.prepare(
            r#"
            SELECT id, backtest_id, symbol, direction, entry_date, entry_price, entry_reason,
                   exit_date, exit_price, exit_reason, shares, profit_loss, profit_loss_percent
            FROM backtest_trades
            WHERE backtest_id = ?1
            ORDER BY entry_date ASC
            "#,
        )?;

        let trades = trade_stmt
            .query_map(params![backtest_id], |row| {
                let dir_str: String = row.get(3)?;
                let entry_str: String = row.get(4)?;
                let exit_str: Option<String> = row.get(7)?;

                Ok(BacktestTrade {
                    id: row.get(0)?,
                    backtest_id: row.get(1)?,
                    symbol: row.get(2)?,
                    direction: TradeDirection::from_str(&dir_str),
                    entry_date: NaiveDate::parse_from_str(&entry_str, "%Y-%m-%d")
                        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
                    entry_price: row.get(5)?,
                    entry_reason: row.get(6)?,
                    exit_date: exit_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                    exit_price: row.get(8)?,
                    exit_reason: row.get(9)?,
                    shares: row.get(10)?,
                    profit_loss: row.get(11)?,
                    profit_loss_percent: row.get(12)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        backtest.trades = trades;

        Ok(Some(backtest))
    }

    /// Delete a backtest result and its trades
    pub fn delete_backtest(&self, backtest_id: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM backtest_trades WHERE backtest_id = ?1",
            params![backtest_id],
        )?;
        tx.execute(
            "DELETE FROM backtest_runs WHERE id = ?1",
            params![backtest_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ========================================================================
    // Paper Trading Methods
    // ========================================================================

    /// Get paper wallet balance
    pub fn get_paper_wallet(&self) -> Result<PaperWallet> {
        let mut stmt = self.conn.prepare(
            "SELECT id, cash, created_at, updated_at FROM paper_wallet WHERE id = 1",
        )?;

        let wallet = stmt.query_row([], |row| {
            Ok(PaperWallet {
                id: row.get(0)?,
                cash: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;

        Ok(wallet)
    }

    /// Update paper wallet cash balance
    fn update_paper_cash(&self, new_cash: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE paper_wallet SET cash = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
            params![new_cash],
        )?;
        Ok(())
    }

    /// Get all paper positions
    pub fn get_paper_positions(&self) -> Result<Vec<PaperPosition>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, quantity, entry_price, entry_date, linked_event_id
            FROM paper_positions
            ORDER BY entry_date DESC
            "#,
        )?;

        let positions = stmt
            .query_map([], |row| {
                Ok(PaperPosition {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    quantity: row.get(2)?,
                    entry_price: row.get(3)?,
                    entry_date: row.get(4)?,
                    linked_event_id: row.get(5)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(positions)
    }

    /// Get paper position for a specific symbol
    pub fn get_paper_position(&self, symbol: &str) -> Result<Option<PaperPosition>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, quantity, entry_price, entry_date, linked_event_id
            FROM paper_positions
            WHERE symbol = ?1
            "#,
        )?;

        let result = stmt.query_row(params![symbol], |row| {
            Ok(PaperPosition {
                id: row.get(0)?,
                symbol: row.get(1)?,
                quantity: row.get(2)?,
                entry_price: row.get(3)?,
                entry_date: row.get(4)?,
                linked_event_id: row.get(5)?,
            })
        });

        match result {
            Ok(pos) => Ok(Some(pos)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Execute a paper trade (BUY or SELL)
    /// Returns the trade record on success
    pub fn execute_paper_trade(
        &self,
        symbol: &str,
        action: PaperTradeAction,
        quantity: f64,
        price: f64,
        linked_event_id: Option<i64>,
        notes: Option<&str>,
    ) -> Result<PaperTrade> {
        let wallet = self.get_paper_wallet()?;
        let cost = quantity * price;

        match action {
            PaperTradeAction::Buy => {
                // Validate sufficient cash
                if wallet.cash < cost {
                    return Err(crate::error::PipelineError::ApiError(format!(
                        "Insufficient cash: have ${:.2}, need ${:.2}",
                        wallet.cash, cost
                    )));
                }

                // Deduct cash
                self.update_paper_cash(wallet.cash - cost)?;

                // Add or update position
                let existing = self.get_paper_position(symbol)?;
                if let Some(pos) = existing {
                    // Average down: new avg price = (old_qty * old_price + new_qty * new_price) / total_qty
                    let total_qty = pos.quantity + quantity;
                    let avg_price =
                        (pos.quantity * pos.entry_price + quantity * price) / total_qty;
                    self.conn.execute(
                        "UPDATE paper_positions SET quantity = ?1, entry_price = ?2 WHERE id = ?3",
                        params![total_qty, avg_price, pos.id],
                    )?;
                } else {
                    // New position
                    self.conn.execute(
                        r#"
                        INSERT INTO paper_positions (symbol, quantity, entry_price, linked_event_id)
                        VALUES (?1, ?2, ?3, ?4)
                        "#,
                        params![symbol, quantity, price, linked_event_id],
                    )?;
                }

                // Record trade
                self.conn.execute(
                    r#"
                    INSERT INTO paper_trades (symbol, action, quantity, price, linked_event_id, notes)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                    params![symbol, "BUY", quantity, price, linked_event_id, notes],
                )?;
            }
            PaperTradeAction::Sell => {
                // Validate sufficient shares
                let position = self.get_paper_position(symbol)?;
                let pos = position.ok_or_else(|| {
                    crate::error::PipelineError::ApiError(format!(
                        "No position in {} to sell",
                        symbol
                    ))
                })?;

                if pos.quantity < quantity {
                    return Err(crate::error::PipelineError::ApiError(format!(
                        "Insufficient shares: have {}, trying to sell {}",
                        pos.quantity, quantity
                    )));
                }

                // Calculate P&L
                let pnl = (price - pos.entry_price) * quantity;

                // Add proceeds to cash
                self.update_paper_cash(wallet.cash + cost)?;

                // Update or delete position
                let remaining = pos.quantity - quantity;
                if remaining <= 0.0001 {
                    // Close position (using small epsilon for float comparison)
                    self.conn.execute(
                        "DELETE FROM paper_positions WHERE id = ?1",
                        params![pos.id],
                    )?;
                } else {
                    // Reduce position
                    self.conn.execute(
                        "UPDATE paper_positions SET quantity = ?1 WHERE id = ?2",
                        params![remaining, pos.id],
                    )?;
                }

                // Record trade with P&L
                self.conn.execute(
                    r#"
                    INSERT INTO paper_trades (symbol, action, quantity, price, pnl, linked_event_id, notes)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    params![symbol, "SELL", quantity, price, pnl, linked_event_id, notes],
                )?;
            }
        }

        // Return the trade we just recorded
        let trade_id = self.conn.last_insert_rowid();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, action, quantity, price, pnl, timestamp, linked_event_id, notes
            FROM paper_trades WHERE id = ?1
            "#,
        )?;

        let trade = stmt.query_row(params![trade_id], |row| {
            let action_str: String = row.get(2)?;
            Ok(PaperTrade {
                id: row.get(0)?,
                symbol: row.get(1)?,
                action: PaperTradeAction::from_str(&action_str),
                quantity: row.get(3)?,
                price: row.get(4)?,
                pnl: row.get(5)?,
                timestamp: row.get(6)?,
                linked_event_id: row.get(7)?,
                notes: row.get(8)?,
            })
        })?;

        Ok(trade)
    }

    /// Get paper trade history
    pub fn get_paper_trades(&self, symbol: Option<&str>, limit: usize) -> Result<Vec<PaperTrade>> {
        let sql = match symbol {
            Some(_) => r#"
                SELECT id, symbol, action, quantity, price, pnl, timestamp, linked_event_id, notes
                FROM paper_trades
                WHERE symbol = ?1
                ORDER BY timestamp DESC
                LIMIT ?2
            "#,
            None => r#"
                SELECT id, symbol, action, quantity, price, pnl, timestamp, linked_event_id, notes
                FROM paper_trades
                ORDER BY timestamp DESC
                LIMIT ?1
            "#,
        };

        let mut stmt = self.conn.prepare(sql)?;

        let trades: Vec<PaperTrade> = match symbol {
            Some(sym) => {
                stmt.query_map(params![sym, limit as i64], |row| {
                    let action_str: String = row.get(2)?;
                    Ok(PaperTrade {
                        id: row.get(0)?,
                        symbol: row.get(1)?,
                        action: PaperTradeAction::from_str(&action_str),
                        quantity: row.get(3)?,
                        price: row.get(4)?,
                        pnl: row.get(5)?,
                        timestamp: row.get(6)?,
                        linked_event_id: row.get(7)?,
                        notes: row.get(8)?,
                    })
                })?
                .collect::<SqliteResult<Vec<_>>>()?
            }
            None => {
                stmt.query_map(params![limit as i64], |row| {
                    let action_str: String = row.get(2)?;
                    Ok(PaperTrade {
                        id: row.get(0)?,
                        symbol: row.get(1)?,
                        action: PaperTradeAction::from_str(&action_str),
                        quantity: row.get(3)?,
                        price: row.get(4)?,
                        pnl: row.get(5)?,
                        timestamp: row.get(6)?,
                        linked_event_id: row.get(7)?,
                        notes: row.get(8)?,
                    })
                })?
                .collect::<SqliteResult<Vec<_>>>()?
            }
        };

        Ok(trades)
    }

    /// Reset paper trading account (clear all positions, trades, reset cash)
    pub fn reset_paper_account(&self, starting_cash: f64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM paper_positions", [])?;
        tx.execute("DELETE FROM paper_trades", [])?;
        tx.execute(
            "UPDATE paper_wallet SET cash = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
            params![starting_cash],
        )?;
        tx.commit()?;
        println!("[OK] Paper trading account reset with ${:.2}", starting_cash);
        Ok(())
    }

    /// Calculate total paper portfolio value (cash + positions at current prices)
    /// Returns (cash, positions_value, total_equity)
    pub fn get_paper_portfolio_value(&self) -> Result<(f64, f64, f64)> {
        let wallet = self.get_paper_wallet()?;
        let positions = self.get_paper_positions()?;

        let mut positions_value = 0.0;
        for pos in positions {
            // Try to get current price, fall back to entry price
            let current_price = self
                .get_latest_price(&pos.symbol)?
                .unwrap_or(pos.entry_price);
            positions_value += pos.quantity * current_price;
        }

        let total_equity = wallet.cash + positions_value;
        Ok((wallet.cash, positions_value, total_equity))
    }

    // ========================================================================
    // DC Trader Methods (Separate from KALIC AI paper trading)
    // ========================================================================

    /// Initialize DC wallet if not exists
    pub fn init_dc_wallet(&self) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO dc_wallet (id, cash, starting_capital) VALUES (1, 1000000.0, 1000000.0)",
            [],
        )?;
        Ok(())
    }

    /// Get DC wallet balance
    pub fn get_dc_wallet(&self) -> Result<DcWallet> {
        // Ensure wallet exists
        self.init_dc_wallet()?;

        let mut stmt = self.conn.prepare(
            "SELECT id, cash, starting_capital, created_at, updated_at FROM dc_wallet WHERE id = 1",
        )?;

        let wallet = stmt.query_row([], |row| {
            Ok(DcWallet {
                id: row.get(0)?,
                cash: row.get(1)?,
                starting_capital: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        Ok(wallet)
    }

    /// Update DC wallet cash balance
    fn update_dc_cash(&self, new_cash: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE dc_wallet SET cash = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
            params![new_cash],
        )?;
        Ok(())
    }

    /// Get all DC positions
    pub fn get_dc_positions(&self) -> Result<Vec<DcPosition>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, quantity, entry_price, entry_date
            FROM dc_positions
            ORDER BY entry_date DESC
            "#,
        )?;

        let positions = stmt
            .query_map([], |row| {
                Ok(DcPosition {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    quantity: row.get(2)?,
                    entry_price: row.get(3)?,
                    entry_date: row.get(4)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(positions)
    }

    /// Get DC position for a specific symbol
    pub fn get_dc_position(&self, symbol: &str) -> Result<Option<DcPosition>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, quantity, entry_price, entry_date
            FROM dc_positions
            WHERE symbol = ?1
            "#,
        )?;

        let result = stmt.query_row(params![symbol], |row| {
            Ok(DcPosition {
                id: row.get(0)?,
                symbol: row.get(1)?,
                quantity: row.get(2)?,
                entry_price: row.get(3)?,
                entry_date: row.get(4)?,
            })
        });

        match result {
            Ok(pos) => Ok(Some(pos)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Execute a DC trade (BUY or SELL)
    pub fn execute_dc_trade(
        &self,
        symbol: &str,
        action: &str,
        quantity: f64,
        price: f64,
        notes: Option<&str>,
    ) -> Result<DcTrade> {
        let wallet = self.get_dc_wallet()?;
        let cost = quantity * price;
        let action_upper = action.to_uppercase();

        match action_upper.as_str() {
            "BUY" => {
                // Validate sufficient cash
                if wallet.cash < cost {
                    return Err(crate::error::PipelineError::ApiError(format!(
                        "Insufficient cash: have ${:.2}, need ${:.2}",
                        wallet.cash, cost
                    )));
                }

                // Deduct cash
                self.update_dc_cash(wallet.cash - cost)?;

                // Add or update position
                let existing = self.get_dc_position(symbol)?;
                if let Some(pos) = existing {
                    // Average down
                    let total_qty = pos.quantity + quantity;
                    let avg_price = (pos.quantity * pos.entry_price + quantity * price) / total_qty;
                    self.conn.execute(
                        "UPDATE dc_positions SET quantity = ?1, entry_price = ?2 WHERE id = ?3",
                        params![total_qty, avg_price, pos.id],
                    )?;
                } else {
                    // New position
                    self.conn.execute(
                        r#"
                        INSERT INTO dc_positions (symbol, quantity, entry_price)
                        VALUES (?1, ?2, ?3)
                        "#,
                        params![symbol, quantity, price],
                    )?;
                }

                // Record trade
                self.conn.execute(
                    r#"
                    INSERT INTO dc_trades (symbol, action, quantity, price, notes)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                    params![symbol, "BUY", quantity, price, notes],
                )?;
            }
            "SELL" => {
                // Validate sufficient shares
                let position = self.get_dc_position(symbol)?;
                let pos = position.ok_or_else(|| {
                    crate::error::PipelineError::ApiError(format!(
                        "No position in {} to sell",
                        symbol
                    ))
                })?;

                if pos.quantity < quantity {
                    return Err(crate::error::PipelineError::ApiError(format!(
                        "Insufficient shares: have {}, trying to sell {}",
                        pos.quantity, quantity
                    )));
                }

                // Calculate P&L
                let pnl = (price - pos.entry_price) * quantity;

                // Add proceeds to cash
                self.update_dc_cash(wallet.cash + cost)?;

                // Update or delete position
                let remaining = pos.quantity - quantity;
                if remaining <= 0.0001 {
                    self.conn.execute(
                        "DELETE FROM dc_positions WHERE id = ?1",
                        params![pos.id],
                    )?;
                } else {
                    self.conn.execute(
                        "UPDATE dc_positions SET quantity = ?1 WHERE id = ?2",
                        params![remaining, pos.id],
                    )?;
                }

                // Record trade with P&L
                self.conn.execute(
                    r#"
                    INSERT INTO dc_trades (symbol, action, quantity, price, pnl, notes)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                    params![symbol, "SELL", quantity, price, pnl, notes],
                )?;
            }
            _ => {
                return Err(crate::error::PipelineError::ApiError(format!(
                    "Invalid action: {}. Must be BUY or SELL",
                    action
                )));
            }
        }

        // Return the trade we just recorded
        let trade_id = self.conn.last_insert_rowid();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, action, quantity, price, pnl, timestamp, notes
            FROM dc_trades WHERE id = ?1
            "#,
        )?;

        let trade = stmt.query_row(params![trade_id], |row| {
            Ok(DcTrade {
                id: row.get(0)?,
                symbol: row.get(1)?,
                action: row.get(2)?,
                quantity: row.get(3)?,
                price: row.get(4)?,
                pnl: row.get(5)?,
                timestamp: row.get(6)?,
                notes: row.get(7)?,
            })
        })?;

        Ok(trade)
    }

    /// Get DC trade history
    pub fn get_dc_trades(&self, limit: usize) -> Result<Vec<DcTrade>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, action, quantity, price, pnl, timestamp, notes
            FROM dc_trades
            ORDER BY timestamp DESC
            LIMIT ?1
            "#,
        )?;

        let trades = stmt
            .query_map(params![limit as i64], |row| {
                Ok(DcTrade {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    action: row.get(2)?,
                    quantity: row.get(3)?,
                    price: row.get(4)?,
                    pnl: row.get(5)?,
                    timestamp: row.get(6)?,
                    notes: row.get(7)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(trades)
    }

    /// Reset DC trading account
    pub fn reset_dc_account(&self, starting_cash: f64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM dc_positions", [])?;
        tx.execute("DELETE FROM dc_trades", [])?;
        tx.execute(
            "UPDATE dc_wallet SET cash = ?1, starting_capital = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
            params![starting_cash],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Calculate DC portfolio value
    pub fn get_dc_portfolio_value(&self) -> Result<(f64, f64, f64)> {
        let wallet = self.get_dc_wallet()?;
        let positions = self.get_dc_positions()?;

        let mut positions_value = 0.0;
        for pos in positions {
            let db_price = self.get_latest_price(&pos.symbol)?;
            let current_price = db_price.unwrap_or(pos.entry_price);
            let pos_value = pos.quantity * current_price;

            // Debug: print price lookup results
            if db_price.is_none() {
                println!("[DC] No price found for {}, using entry ${:.2}", pos.symbol, pos.entry_price);
            } else {
                println!("[DC] {} price: ${:.2} (entry: ${:.2})", pos.symbol, current_price, pos.entry_price);
            }

            positions_value += pos_value;
        }

        let total_equity = wallet.cash + positions_value;
        println!("[DC] Portfolio: cash=${:.2}, positions=${:.2}, total=${:.2}", wallet.cash, positions_value, total_equity);
        Ok((wallet.cash, positions_value, total_equity))
    }

    /// Import multiple DC trades from JSON
    pub fn import_dc_trades_json(&self, trades_json: &str) -> Result<ImportResult> {
        let trades: Vec<serde_json::Value> = serde_json::from_str(trades_json)
            .map_err(|e| crate::error::PipelineError::ApiError(format!("Invalid JSON: {}", e)))?;

        let mut success_count = 0;
        let mut error_count = 0;
        let mut errors: Vec<String> = Vec::new();

        for (i, trade) in trades.iter().enumerate() {
            let symbol = trade["symbol"].as_str().unwrap_or("").to_uppercase();
            let action = trade["action"].as_str().unwrap_or("BUY").to_uppercase();
            let quantity = trade["quantity"].as_f64().unwrap_or(0.0);
            let price = trade["price"].as_f64();
            let notes = trade["notes"].as_str();

            if symbol.is_empty() || quantity <= 0.0 {
                error_count += 1;
                errors.push(format!("Row {}: Invalid symbol or quantity", i + 1));
                continue;
            }

            // If price not provided, try to fetch current price
            let trade_price = match price {
                Some(p) => p,
                None => {
                    match self.get_latest_price(&symbol)? {
                        Some(p) => p,
                        None => {
                            error_count += 1;
                            errors.push(format!("Row {}: No price provided and could not fetch for {}", i + 1, symbol));
                            continue;
                        }
                    }
                }
            };

            match self.execute_dc_trade(&symbol, &action, quantity, trade_price, notes) {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error_count += 1;
                    errors.push(format!("Row {}: {}", i + 1, e));
                }
            }
        }

        Ok(ImportResult {
            success_count,
            error_count,
            errors,
        })
    }

    /// Import multiple DC trades from CSV
    pub fn import_dc_trades_csv(&self, csv_content: &str) -> Result<ImportResult> {
        let mut success_count = 0;
        let mut error_count = 0;
        let mut errors: Vec<String> = Vec::new();

        let lines: Vec<&str> = csv_content.lines().collect();
        if lines.is_empty() {
            return Ok(ImportResult { success_count: 0, error_count: 0, errors: vec![] });
        }

        // Skip header if present
        let start_idx = if lines[0].to_lowercase().contains("symbol") { 1 } else { 0 };

        // First pass: calculate total BUY value needed
        let mut total_buy_value = 0.0;
        let mut parsed_trades: Vec<(String, String, f64, f64, Option<String>)> = Vec::new();

        for (i, line) in lines.iter().skip(start_idx).enumerate() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
            if parts.len() < 3 {
                error_count += 1;
                errors.push(format!("Row {}: Not enough columns", i + 1));
                continue;
            }

            let symbol = parts[0].to_uppercase();
            let action = parts[1].to_uppercase();
            let quantity: f64 = match parts[2].parse() {
                Ok(q) => q,
                Err(_) => {
                    error_count += 1;
                    errors.push(format!("Row {}: Invalid quantity", i + 1));
                    continue;
                }
            };

            let price: Option<f64> = if parts.len() > 3 && !parts[3].is_empty() {
                parts[3].parse().ok()
            } else {
                None
            };

            let notes: Option<String> = if parts.len() > 4 && !parts[4].is_empty() {
                Some(parts[4].to_string())
            } else {
                None
            };

            // Get trade price
            let trade_price = match price {
                Some(p) => p,
                None => {
                    match self.get_latest_price(&symbol)? {
                        Some(p) => p,
                        None => {
                            error_count += 1;
                            errors.push(format!("Row {}: No price provided and could not fetch for {}", i + 1, symbol));
                            continue;
                        }
                    }
                }
            };

            if action == "BUY" {
                total_buy_value += quantity * trade_price;
            }
            parsed_trades.push((symbol, action, quantity, trade_price, notes));
        }

        // Auto-adjust starting capital if needed (add 5% buffer)
        let wallet = self.get_dc_wallet()?;
        let needed_capital = total_buy_value * 1.05;
        if needed_capital > wallet.cash {
            self.conn.execute(
                "UPDATE dc_wallet SET cash = ?1, starting_capital = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
                params![needed_capital],
            )?;
        }

        // Second pass: execute trades
        for (symbol, action, quantity, trade_price, notes) in parsed_trades {
            match self.execute_dc_trade(&symbol, &action, quantity, trade_price, notes.as_deref()) {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error_count += 1;
                    errors.push(format!("{}: {}", symbol, e));
                }
            }
        }

        Ok(ImportResult {
            success_count,
            error_count,
            errors,
        })
    }

    /// Record a portfolio snapshot for charting
    pub fn record_portfolio_snapshot(&self, team: &str) -> Result<()> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let (cash, positions_value, total_value) = match team {
            "KALIC" => self.get_paper_portfolio_value()?,
            "DC" => self.get_dc_portfolio_value()?,
            _ => return Err(crate::error::PipelineError::ApiError(format!("Invalid team: {}", team))),
        };

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO portfolio_snapshots (team, date, total_value, cash, positions_value)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![team, today, total_value, cash, positions_value],
        )?;

        Ok(())
    }

    /// Get portfolio snapshots for charting
    pub fn get_portfolio_snapshots(&self, team: Option<&str>, days: i32) -> Result<Vec<PortfolioSnapshot>> {
        let sql = match team {
            Some(_) => r#"
                SELECT id, team, date, total_value, cash, positions_value
                FROM portfolio_snapshots
                WHERE team = ?1 AND date >= date('now', '-' || ?2 || ' days')
                ORDER BY date ASC
            "#,
            None => r#"
                SELECT id, team, date, total_value, cash, positions_value
                FROM portfolio_snapshots
                WHERE date >= date('now', '-' || ?1 || ' days')
                ORDER BY team, date ASC
            "#,
        };

        let mut stmt = self.conn.prepare(sql)?;

        let snapshots: Vec<PortfolioSnapshot> = match team {
            Some(t) => {
                stmt.query_map(params![t, days], |row| {
                    Ok(PortfolioSnapshot {
                        id: row.get(0)?,
                        team: row.get(1)?,
                        date: row.get(2)?,
                        total_value: row.get(3)?,
                        cash: row.get(4)?,
                        positions_value: row.get(5)?,
                    })
                })?
                .collect::<SqliteResult<Vec<_>>>()?
            }
            None => {
                stmt.query_map(params![days], |row| {
                    Ok(PortfolioSnapshot {
                        id: row.get(0)?,
                        team: row.get(1)?,
                        date: row.get(2)?,
                        total_value: row.get(3)?,
                        cash: row.get(4)?,
                        positions_value: row.get(5)?,
                    })
                })?
                .collect::<SqliteResult<Vec<_>>>()?
            }
        };

        Ok(snapshots)
    }

    /// Save a team configuration
    pub fn save_team_config(&self, name: &str, description: Option<&str>) -> Result<i64> {
        let kalic_wallet = self.get_paper_wallet()?;
        let dc_wallet = self.get_dc_wallet()?;

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO trading_teams (name, description, kalic_starting_capital, dc_starting_capital)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![name, description, kalic_wallet.cash, dc_wallet.cash],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Load a team configuration
    pub fn load_team_config(&self, name: &str) -> Result<TeamConfig> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, name, description, kalic_starting_capital, dc_starting_capital, created_at
            FROM trading_teams
            WHERE name = ?1
            "#,
        )?;

        let config = stmt.query_row(params![name], |row| {
            Ok(TeamConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                kalic_starting_capital: row.get(3)?,
                dc_starting_capital: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        Ok(config)
    }

    /// List all team configurations
    pub fn list_team_configs(&self) -> Result<Vec<TeamConfig>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, name, description, kalic_starting_capital, dc_starting_capital, created_at
            FROM trading_teams
            ORDER BY created_at DESC
            "#,
        )?;

        let configs = stmt
            .query_map([], |row| {
                Ok(TeamConfig {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    kalic_starting_capital: row.get(3)?,
                    dc_starting_capital: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(configs)
    }

    /// Get competition stats comparing KALIC and DC
    pub fn get_competition_stats(&self) -> Result<CompetitionStats> {
        let (kalic_cash, kalic_positions, kalic_total) = self.get_paper_portfolio_value()?;
        let (dc_cash, dc_positions, dc_total) = self.get_dc_portfolio_value()?;

        let kalic_wallet = self.get_paper_wallet()?;
        let dc_wallet = self.get_dc_wallet()?;

        // Get trade counts
        let kalic_trades: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM paper_trades",
            [],
            |row| row.get(0),
        )?;
        let dc_trades: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM dc_trades",
            [],
            |row| row.get(0),
        )?;

        // Calculate P&L percentages
        let kalic_starting = 1000000.0; // Default starting capital
        let kalic_pnl_pct = ((kalic_total - kalic_starting) / kalic_starting) * 100.0;
        let dc_pnl_pct = ((dc_total - dc_wallet.starting_capital) / dc_wallet.starting_capital) * 100.0;

        // Determine leader
        let leader = if kalic_total > dc_total {
            "KALIC".to_string()
        } else if dc_total > kalic_total {
            "DC".to_string()
        } else {
            "TIE".to_string()
        };

        Ok(CompetitionStats {
            kalic_total,
            kalic_cash,
            kalic_positions,
            kalic_pnl_pct,
            kalic_trades,
            dc_total,
            dc_cash,
            dc_positions,
            dc_pnl_pct,
            dc_trades,
            leader,
            lead_amount: (kalic_total - dc_total).abs(),
        })
    }

    // ========================================================================
    // AI Trading Simulator Methods
    // ========================================================================

    /// Get AI trader configuration
    pub fn get_ai_trader_config(&self) -> Result<AiTraderConfig> {
        let row = self.conn.query_row(
            r#"SELECT starting_capital, max_position_size_percent, stop_loss_percent,
                    take_profit_percent, session_duration_minutes, benchmark_symbol, model_priority,
                    trading_mode, daily_loss_threshold, consecutive_loss_limit,
                    auto_conservative_on_trigger, max_daily_trades, max_single_trade_value,
                    require_confluence, blocked_hours
             FROM ai_trader_config WHERE id = 1"#,
            [],
            |row| {
                Ok(AiTraderConfig {
                    starting_capital: row.get(0)?,
                    max_position_size_percent: row.get(1)?,
                    stop_loss_percent: row.get(2)?,
                    take_profit_percent: row.get(3)?,
                    session_duration_minutes: row.get(4)?,
                    benchmark_symbol: row.get(5)?,
                    model_priority: row.get::<_, String>(6)?
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect(),
                    trading_mode: row.get(7)?,
                    daily_loss_threshold: row.get(8)?,
                    consecutive_loss_limit: row.get(9)?,
                    auto_conservative_on_trigger: row.get::<_, i32>(10)? != 0,
                    max_daily_trades: row.get(11)?,
                    max_single_trade_value: row.get(12)?,
                    require_confluence: row.get::<_, i32>(13)? != 0,
                    blocked_hours: row.get(14)?,
                })
            },
        )?;
        Ok(row)
    }

    /// Update AI trader configuration
    pub fn update_ai_trader_config(&self, config: &AiTraderConfig) -> Result<()> {
        self.conn.execute(
            r#"UPDATE ai_trader_config SET
                starting_capital = ?1, max_position_size_percent = ?2,
                stop_loss_percent = ?3, take_profit_percent = ?4,
                session_duration_minutes = ?5, benchmark_symbol = ?6,
                model_priority = ?7, trading_mode = ?8, daily_loss_threshold = ?9,
                consecutive_loss_limit = ?10, auto_conservative_on_trigger = ?11,
                max_daily_trades = ?12, max_single_trade_value = ?13,
                require_confluence = ?14, blocked_hours = ?15,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = 1"#,
            params![
                config.starting_capital,
                config.max_position_size_percent,
                config.stop_loss_percent,
                config.take_profit_percent,
                config.session_duration_minutes,
                config.benchmark_symbol,
                config.model_priority.join(","),
                config.trading_mode,
                config.daily_loss_threshold,
                config.consecutive_loss_limit,
                config.auto_conservative_on_trigger as i32,
                config.max_daily_trades,
                config.max_single_trade_value,
                config.require_confluence as i32,
                config.blocked_hours,
            ],
        )?;
        Ok(())
    }

    /// Start a new AI trading session
    pub fn start_ai_session(&self, starting_value: f64) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO ai_trading_sessions (starting_portfolio_value, status) VALUES (?1, 'active')",
            params![starting_value],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// End an AI trading session
    pub fn end_ai_session(&self, session_id: i64, ending_value: f64, notes: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE ai_trading_sessions SET
                end_time = CURRENT_TIMESTAMP, ending_portfolio_value = ?1,
                status = 'completed', session_notes = ?2
             WHERE id = ?3",
            params![ending_value, notes, session_id],
        )?;
        Ok(())
    }

    /// Get active AI trading session
    pub fn get_active_ai_session(&self) -> Result<Option<AiTradingSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start_time, end_time, starting_portfolio_value, ending_portfolio_value,
                    decisions_count, trades_count, session_notes, status
             FROM ai_trading_sessions WHERE status = 'active' ORDER BY id DESC LIMIT 1"
        )?;

        let result = stmt.query_row([], |row| {
            Ok(AiTradingSession {
                id: row.get(0)?,
                start_time: row.get(1)?,
                end_time: row.get(2)?,
                starting_portfolio_value: row.get(3)?,
                ending_portfolio_value: row.get(4)?,
                decisions_count: row.get(5)?,
                trades_count: row.get(6)?,
                session_notes: row.get(7)?,
                status: row.get(8)?,
            })
        });

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get AI trading session by ID
    pub fn get_ai_session(&self, session_id: i64) -> Result<Option<AiTradingSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start_time, end_time, starting_portfolio_value, ending_portfolio_value,
                    decisions_count, trades_count, session_notes, status
             FROM ai_trading_sessions WHERE id = ?1"
        )?;

        let result = stmt.query_row(params![session_id], |row| {
            Ok(AiTradingSession {
                id: row.get(0)?,
                start_time: row.get(1)?,
                end_time: row.get(2)?,
                starting_portfolio_value: row.get(3)?,
                ending_portfolio_value: row.get(4)?,
                decisions_count: row.get(5)?,
                trades_count: row.get(6)?,
                session_notes: row.get(7)?,
                status: row.get(8)?,
            })
        });

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Record an AI trade decision
    pub fn record_ai_decision(&self, decision: &AiTradeDecision) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO ai_trade_decisions
                (session_id, action, symbol, quantity, price_at_decision, confidence, reasoning,
                 model_used, predicted_direction, predicted_price_target, predicted_timeframe_days,
                 paper_trade_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                decision.session_id,
                decision.action,
                decision.symbol,
                decision.quantity,
                decision.price_at_decision,
                decision.confidence,
                decision.reasoning,
                decision.model_used,
                decision.predicted_direction,
                decision.predicted_price_target,
                decision.predicted_timeframe_days,
                decision.paper_trade_id,
            ],
        )?;

        // Update session decision count
        if let Some(sid) = decision.session_id {
            self.conn.execute(
                "UPDATE ai_trading_sessions SET decisions_count = decisions_count + 1 WHERE id = ?1",
                params![sid],
            )?;
            if decision.paper_trade_id.is_some() {
                self.conn.execute(
                    "UPDATE ai_trading_sessions SET trades_count = trades_count + 1 WHERE id = ?1",
                    params![sid],
                )?;
            }
        }

        Ok(self.conn.last_insert_rowid())
    }

    /// Get AI decisions with optional filters
    pub fn get_ai_decisions(
        &self,
        session_id: Option<i64>,
        symbol: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AiTradeDecision>> {
        let mut sql = String::from(
            "SELECT id, session_id, timestamp, action, symbol, quantity, price_at_decision,
                    confidence, reasoning, model_used, predicted_direction, predicted_price_target,
                    predicted_timeframe_days, actual_outcome, actual_price_at_timeframe,
                    prediction_accurate, paper_trade_id
             FROM ai_trade_decisions WHERE 1=1"
        );

        if session_id.is_some() {
            sql.push_str(" AND session_id = ?1");
        }
        if symbol.is_some() {
            sql.push_str(if session_id.is_some() { " AND symbol = ?2" } else { " AND symbol = ?1" });
        }
        sql.push_str(" ORDER BY timestamp DESC LIMIT ?");

        let mut stmt = self.conn.prepare(&sql)?;

        let decisions = match (session_id, symbol) {
            (Some(sid), Some(sym)) => {
                stmt.query_map(params![sid, sym, limit], Self::map_ai_decision)?
            }
            (Some(sid), None) => {
                stmt.query_map(params![sid, limit], Self::map_ai_decision)?
            }
            (None, Some(sym)) => {
                stmt.query_map(params![sym, limit], Self::map_ai_decision)?
            }
            (None, None) => {
                stmt.query_map(params![limit], Self::map_ai_decision)?
            }
        };

        decisions.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }

    fn map_ai_decision(row: &rusqlite::Row) -> rusqlite::Result<AiTradeDecision> {
        Ok(AiTradeDecision {
            id: row.get(0)?,
            session_id: row.get(1)?,
            timestamp: row.get(2)?,
            action: row.get(3)?,
            symbol: row.get(4)?,
            quantity: row.get(5)?,
            price_at_decision: row.get(6)?,
            confidence: row.get(7)?,
            reasoning: row.get(8)?,
            model_used: row.get(9)?,
            predicted_direction: row.get(10)?,
            predicted_price_target: row.get(11)?,
            predicted_timeframe_days: row.get(12)?,
            actual_outcome: row.get(13)?,
            actual_price_at_timeframe: row.get(14)?,
            prediction_accurate: row.get(15)?,
            paper_trade_id: row.get(16)?,
        })
    }

    /// Record a performance snapshot
    pub fn record_ai_performance_snapshot(&self, snapshot: &AiPerformanceSnapshot) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO ai_performance_snapshots
                (portfolio_value, cash, positions_value, benchmark_value, benchmark_symbol,
                 total_pnl, total_pnl_percent, benchmark_pnl_percent, prediction_accuracy,
                 trades_to_date, winning_trades, losing_trades, win_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                snapshot.portfolio_value,
                snapshot.cash,
                snapshot.positions_value,
                snapshot.benchmark_value,
                snapshot.benchmark_symbol,
                snapshot.total_pnl,
                snapshot.total_pnl_percent,
                snapshot.benchmark_pnl_percent,
                snapshot.prediction_accuracy,
                snapshot.trades_to_date,
                snapshot.winning_trades,
                snapshot.losing_trades,
                snapshot.win_rate,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get performance snapshots for charting
    pub fn get_ai_performance_snapshots(&self, days: u32) -> Result<Vec<AiPerformanceSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, portfolio_value, cash, positions_value, benchmark_value,
                    benchmark_symbol, total_pnl, total_pnl_percent, benchmark_pnl_percent,
                    prediction_accuracy, trades_to_date, winning_trades, losing_trades, win_rate
             FROM ai_performance_snapshots
             WHERE timestamp >= datetime('now', ?1)
             ORDER BY timestamp ASC"
        )?;

        let days_param = format!("-{} days", days);
        let snapshots = stmt.query_map(params![days_param], |row| {
            Ok(AiPerformanceSnapshot {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                portfolio_value: row.get(2)?,
                cash: row.get(3)?,
                positions_value: row.get(4)?,
                benchmark_value: row.get(5)?,
                benchmark_symbol: row.get(6)?,
                total_pnl: row.get(7)?,
                total_pnl_percent: row.get(8)?,
                benchmark_pnl_percent: row.get(9)?,
                prediction_accuracy: row.get(10)?,
                trades_to_date: row.get(11)?,
                winning_trades: row.get(12)?,
                losing_trades: row.get(13)?,
                win_rate: row.get(14)?,
            })
        })?;

        snapshots.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }

    /// Get the first performance snapshot (for benchmark baseline)
    pub fn get_first_ai_snapshot(&self) -> Result<Option<AiPerformanceSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, portfolio_value, cash, positions_value, benchmark_value,
                    benchmark_symbol, total_pnl, total_pnl_percent, benchmark_pnl_percent,
                    prediction_accuracy, trades_to_date, winning_trades, losing_trades, win_rate
             FROM ai_performance_snapshots ORDER BY timestamp ASC LIMIT 1"
        )?;

        let result = stmt.query_row([], |row| {
            Ok(AiPerformanceSnapshot {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                portfolio_value: row.get(2)?,
                cash: row.get(3)?,
                positions_value: row.get(4)?,
                benchmark_value: row.get(5)?,
                benchmark_symbol: row.get(6)?,
                total_pnl: row.get(7)?,
                total_pnl_percent: row.get(8)?,
                benchmark_pnl_percent: row.get(9)?,
                prediction_accuracy: row.get(10)?,
                trades_to_date: row.get(11)?,
                winning_trades: row.get(12)?,
                losing_trades: row.get(13)?,
                win_rate: row.get(14)?,
            })
        });

        match result {
            Ok(snap) => Ok(Some(snap)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update prediction outcome after timeframe expires
    pub fn update_ai_prediction_outcome(
        &self,
        decision_id: i64,
        actual_outcome: &str,
        actual_price: f64,
        was_accurate: bool,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE ai_trade_decisions SET
                actual_outcome = ?1, actual_price_at_timeframe = ?2, prediction_accurate = ?3
             WHERE id = ?4",
            params![actual_outcome, actual_price, was_accurate as i32, decision_id],
        )?;
        Ok(())
    }

    /// Get unevaluated predictions that have reached their timeframe
    pub fn get_unevaluated_ai_predictions(&self) -> Result<Vec<AiTradeDecision>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, timestamp, action, symbol, quantity, price_at_decision,
                    confidence, reasoning, model_used, predicted_direction, predicted_price_target,
                    predicted_timeframe_days, actual_outcome, actual_price_at_timeframe,
                    prediction_accurate, paper_trade_id
             FROM ai_trade_decisions
             WHERE prediction_accurate IS NULL
               AND predicted_timeframe_days IS NOT NULL
               AND datetime(timestamp, '+' || predicted_timeframe_days || ' days') <= datetime('now')"
        )?;

        let decisions = stmt.query_map([], Self::map_ai_decision)?;
        decisions.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }

    /// Calculate prediction accuracy statistics
    pub fn get_ai_prediction_accuracy(&self) -> Result<AiPredictionAccuracy> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM ai_trade_decisions WHERE prediction_accurate IS NOT NULL",
            [],
            |row| row.get(0),
        )?;

        let accurate: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM ai_trade_decisions WHERE prediction_accurate = 1",
            [],
            |row| row.get(0),
        )?;

        let accuracy_percent = if total > 0 {
            (accurate as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(AiPredictionAccuracy {
            total_predictions: total as u32,
            accurate_predictions: accurate as u32,
            accuracy_percent,
        })
    }

    /// Get total sessions count
    pub fn get_ai_sessions_count(&self) -> Result<u32> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM ai_trading_sessions",
            [],
            |row| row.get(0),
        )?;
        Ok(count as u32)
    }

    /// Get total AI decisions count
    pub fn get_ai_decisions_count(&self) -> Result<u32> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM ai_trade_decisions",
            [],
            |row| row.get(0),
        )?;
        Ok(count as u32)
    }

    /// Reset AI trading data (for fresh start)
    pub fn reset_ai_trading(&self, starting_capital: f64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM ai_performance_snapshots", [])?;
        tx.execute("DELETE FROM ai_trade_decisions", [])?;
        tx.execute("DELETE FROM ai_trading_sessions", [])?;
        tx.execute("DELETE FROM paper_positions", [])?;
        tx.execute("DELETE FROM paper_trades", [])?;
        tx.execute(
            "UPDATE paper_wallet SET cash = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
            params![starting_capital],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ========================================================================
    // Guardrails & Circuit Breaker Methods
    // ========================================================================

    /// Update trading mode in ai_trader_config
    pub fn update_trading_mode(&self, mode: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE ai_trader_config SET trading_mode = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
            params![mode],
        )?;
        Ok(())
    }

    /// Get current trading mode from config
    pub fn get_trading_mode(&self) -> Result<String> {
        let mode: String = self.conn.query_row(
            "SELECT trading_mode FROM ai_trader_config WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(mode)
    }

    /// Log circuit breaker event
    pub fn log_circuit_breaker_event(
        &self,
        trigger_type: &str,
        previous_mode: &str,
        new_mode: &str,
        daily_pnl: f64,
        consecutive_losses: i32,
    ) -> Result<i64> {
        self.conn.execute(
            r#"INSERT INTO circuit_breaker_events
               (trigger_type, previous_mode, new_mode, daily_pnl, consecutive_losses, resume_at)
               VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', '+1 hour'))"#,
            params![trigger_type, previous_mode, new_mode, daily_pnl, consecutive_losses],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Log trade rejection
    pub fn log_trade_rejection(&self, rejection: &crate::ai_trader::TradeRejection) -> Result<i64> {
        self.conn.execute(
            r#"INSERT INTO trade_rejections
               (session_id, attempted_action, symbol, quantity, quantity_percent,
                estimated_value, reason, rule_triggered, trading_mode, raw_request)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
            params![
                rejection.session_id,
                rejection.attempted_action,
                rejection.symbol,
                rejection.quantity,
                rejection.quantity_percent,
                rejection.estimated_value,
                rejection.reason,
                rejection.rule_triggered,
                rejection.trading_mode,
                rejection.raw_request,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get paper trades from today only
    pub fn get_paper_trades_today(&self) -> Result<Vec<PaperTrade>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, symbol, action, quantity, price, pnl, timestamp, linked_event_id, notes
               FROM paper_trades
               WHERE date(timestamp) = date('now')
               ORDER BY timestamp DESC"#
        )?;

        let trades = stmt.query_map([], |row| {
            Ok(PaperTrade {
                id: row.get(0)?,
                symbol: row.get(1)?,
                action: PaperTradeAction::from_str(&row.get::<_, String>(2)?),
                quantity: row.get(3)?,
                price: row.get(4)?,
                pnl: row.get(5)?,
                timestamp: row.get(6)?,
                linked_event_id: row.get(7)?,
                notes: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(trades)
    }

    /// Get recent trade rejections
    pub fn get_trade_rejections(&self, limit: usize) -> Result<Vec<(i64, String, String, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, timestamp, attempted_action, symbol, reason, rule_triggered
               FROM trade_rejections
               ORDER BY timestamp DESC
               LIMIT ?1"#
        )?;

        let rejections = stmt.query_map(params![limit], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rejections)
    }

    /// Get circuit breaker events
    pub fn get_circuit_breaker_events(&self, limit: usize) -> Result<Vec<(i64, String, String, String, String, f64)>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, timestamp, trigger_type, previous_mode, new_mode, daily_pnl
               FROM circuit_breaker_events
               ORDER BY timestamp DESC
               LIMIT ?1"#
        )?;

        let events = stmt.query_map(params![limit], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Update circuit breaker settings in config
    pub fn update_circuit_breaker_settings(
        &self,
        daily_loss_threshold: f64,
        consecutive_loss_limit: i32,
        auto_conservative: bool,
    ) -> Result<()> {
        self.conn.execute(
            r#"UPDATE ai_trader_config SET
               daily_loss_threshold = ?1,
               consecutive_loss_limit = ?2,
               auto_conservative_on_trigger = ?3,
               updated_at = CURRENT_TIMESTAMP
               WHERE id = 1"#,
            params![daily_loss_threshold, consecutive_loss_limit, auto_conservative as i32],
        )?;
        Ok(())
    }
}

/// Database schema SQL
const SCHEMA_SQL: &str = r#"
-- Symbol master table
CREATE TABLE IF NOT EXISTS symbols (
    symbol TEXT PRIMARY KEY,
    name TEXT,
    sector TEXT,
    industry TEXT,
    market_cap REAL,
    country TEXT,
    exchange TEXT,
    currency TEXT,
    isin TEXT,
    asset_class TEXT,
    favorited INTEGER DEFAULT 0,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);


-- Daily price data
CREATE TABLE IF NOT EXISTS daily_prices (
    symbol TEXT,
    timestamp DATE,
    open REAL,
    high REAL,
    low REAL,
    close REAL,
    volume INTEGER,
    adjusted_close REAL,
    source TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (symbol, timestamp)
);

-- Macro economic indicators
CREATE TABLE IF NOT EXISTS macro_data (
    indicator TEXT,
    date DATE,
    value REAL,
    frequency TEXT,
    source TEXT DEFAULT 'FRED',
    PRIMARY KEY (indicator, date)
);

-- Watchlists
CREATE TABLE IF NOT EXISTS watchlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS watchlist_symbols (
    watchlist_id INTEGER,
    symbol TEXT,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    notes TEXT,
    PRIMARY KEY (watchlist_id, symbol),
    FOREIGN KEY (watchlist_id) REFERENCES watchlists(id)
);

-- API call tracking
CREATE TABLE IF NOT EXISTS api_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT,
    endpoint TEXT,
    symbol TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN,
    error_message TEXT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_prices_symbol ON daily_prices(symbol);
CREATE INDEX IF NOT EXISTS idx_prices_timestamp ON daily_prices(timestamp);
CREATE INDEX IF NOT EXISTS idx_prices_source ON daily_prices(source);
CREATE INDEX IF NOT EXISTS idx_symbols_sector ON symbols(sector);
CREATE INDEX IF NOT EXISTS idx_macro_indicator ON macro_data(indicator);
CREATE INDEX IF NOT EXISTS idx_macro_date ON macro_data(date);
CREATE INDEX IF NOT EXISTS idx_api_calls_source ON api_calls(source);
CREATE INDEX IF NOT EXISTS idx_api_calls_timestamp ON api_calls(timestamp);

-- Views
CREATE VIEW IF NOT EXISTS latest_prices AS
SELECT p.*
FROM daily_prices p
INNER JOIN (
    SELECT symbol, MAX(timestamp) as max_date
    FROM daily_prices
    GROUP BY symbol
) latest ON p.symbol = latest.symbol AND p.timestamp = latest.max_date;

CREATE VIEW IF NOT EXISTS api_rate_limits AS
SELECT
    source,
    COUNT(*) as calls_today,
    MAX(timestamp) as last_call
FROM api_calls
WHERE DATE(timestamp) = DATE('now')
GROUP BY source;

-- Technical indicators
CREATE TABLE IF NOT EXISTS technical_indicators (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    timestamp DATE NOT NULL,
    indicator_name TEXT NOT NULL,
    value REAL NOT NULL,
    params TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(symbol, timestamp, indicator_name)
);

CREATE INDEX IF NOT EXISTS idx_ti_symbol_date ON technical_indicators(symbol, timestamp);
CREATE INDEX IF NOT EXISTS idx_ti_indicator ON technical_indicators(indicator_name);

-- Price alerts
CREATE TABLE IF NOT EXISTS price_alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    target_price REAL NOT NULL,
    condition TEXT NOT NULL CHECK(condition IN ('above', 'below')),
    triggered BOOLEAN DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_alerts_symbol ON price_alerts(symbol);
CREATE INDEX IF NOT EXISTS idx_alerts_triggered ON price_alerts(triggered);

-- Portfolio positions
CREATE TABLE IF NOT EXISTS portfolio_positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    quantity REAL NOT NULL,
    price REAL NOT NULL,
    position_type TEXT NOT NULL CHECK(position_type IN ('buy', 'sell')),
    date TEXT NOT NULL,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_positions_symbol ON portfolio_positions(symbol);

-- Google Trends data
CREATE TABLE IF NOT EXISTS trends_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    keyword TEXT NOT NULL,
    date DATE NOT NULL,
    value INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(keyword, date)
);

CREATE INDEX IF NOT EXISTS idx_trends_keyword ON trends_data(keyword);
CREATE INDEX IF NOT EXISTS idx_trends_date ON trends_data(date);

-- Trading signals
CREATE TABLE IF NOT EXISTS signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    signal_type TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('bullish', 'bearish', 'neutral')),
    strength REAL NOT NULL CHECK(strength >= 0.0 AND strength <= 1.0),
    price_at_signal REAL NOT NULL,
    triggered_by TEXT NOT NULL,
    trigger_value REAL NOT NULL,
    timestamp DATE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    acknowledged BOOLEAN DEFAULT 0,
    UNIQUE(symbol, signal_type, timestamp)
);

CREATE INDEX IF NOT EXISTS idx_signals_symbol ON signals(symbol);
CREATE INDEX IF NOT EXISTS idx_signals_type ON signals(signal_type);
CREATE INDEX IF NOT EXISTS idx_signals_timestamp ON signals(timestamp);
CREATE INDEX IF NOT EXISTS idx_signals_direction ON signals(direction);
CREATE INDEX IF NOT EXISTS idx_signals_acknowledged ON signals(acknowledged);

-- Indicator-based alerts
CREATE TABLE IF NOT EXISTS indicator_alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    alert_type TEXT NOT NULL CHECK(alert_type IN ('threshold', 'crossover', 'band_touch')),
    indicator_name TEXT NOT NULL,
    secondary_indicator TEXT,
    condition TEXT NOT NULL CHECK(condition IN (
        'crosses_above', 'crosses_below', 'bullish_crossover', 'bearish_crossover'
    )),
    threshold REAL,
    triggered BOOLEAN DEFAULT 0,
    last_value REAL,
    message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_ind_alerts_symbol ON indicator_alerts(symbol);
CREATE INDEX IF NOT EXISTS idx_ind_alerts_triggered ON indicator_alerts(triggered);

-- Backtesting strategies
CREATE TABLE IF NOT EXISTS strategies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    description TEXT,
    entry_condition TEXT NOT NULL,
    entry_threshold REAL NOT NULL,
    exit_condition TEXT NOT NULL,
    exit_threshold REAL NOT NULL,
    stop_loss_percent REAL,
    take_profit_percent REAL,
    position_size_percent REAL NOT NULL DEFAULT 100.0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_strategies_name ON strategies(name);

-- Backtest runs
CREATE TABLE IF NOT EXISTS backtest_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_id INTEGER NOT NULL,
    strategy_name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    initial_capital REAL NOT NULL,
    final_capital REAL NOT NULL,
    total_return REAL NOT NULL,
    total_return_dollars REAL NOT NULL,
    max_drawdown REAL NOT NULL,
    sharpe_ratio REAL NOT NULL,
    win_rate REAL NOT NULL,
    total_trades INTEGER NOT NULL,
    winning_trades INTEGER NOT NULL,
    losing_trades INTEGER NOT NULL,
    avg_win_percent REAL NOT NULL,
    avg_loss_percent REAL NOT NULL,
    profit_factor REAL NOT NULL,
    avg_trade_duration_days REAL NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (strategy_id) REFERENCES strategies(id)
);

CREATE INDEX IF NOT EXISTS idx_backtest_runs_strategy ON backtest_runs(strategy_id);
CREATE INDEX IF NOT EXISTS idx_backtest_runs_symbol ON backtest_runs(symbol);
CREATE INDEX IF NOT EXISTS idx_backtest_runs_date ON backtest_runs(created_at);

-- Backtest trades
CREATE TABLE IF NOT EXISTS backtest_trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    backtest_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('long', 'short')),
    entry_date DATE NOT NULL,
    entry_price REAL NOT NULL,
    entry_reason TEXT NOT NULL,
    exit_date DATE,
    exit_price REAL,
    exit_reason TEXT,
    shares REAL NOT NULL,
    profit_loss REAL,
    profit_loss_percent REAL,
    FOREIGN KEY (backtest_id) REFERENCES backtest_runs(id)
);

CREATE INDEX IF NOT EXISTS idx_backtest_trades_run ON backtest_trades(backtest_id);
CREATE INDEX IF NOT EXISTS idx_backtest_trades_symbol ON backtest_trades(symbol);

-- Paper trading wallet (singleton - one paper account)
CREATE TABLE IF NOT EXISTS paper_wallet (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    cash REAL NOT NULL DEFAULT 1000000.0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Initialize wallet if not exists (default $1M for AI trading)
INSERT OR IGNORE INTO paper_wallet (id, cash) VALUES (1, 1000000.0);

-- Paper trading positions (open positions)
CREATE TABLE IF NOT EXISTS paper_positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    quantity REAL NOT NULL,
    entry_price REAL NOT NULL,
    entry_date TEXT DEFAULT CURRENT_TIMESTAMP,
    linked_event_id INTEGER,
    FOREIGN KEY (linked_event_id) REFERENCES market_events(id)
);

CREATE INDEX IF NOT EXISTS idx_paper_positions_symbol ON paper_positions(symbol);

-- Paper trading history (all trades)
CREATE TABLE IF NOT EXISTS paper_trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('BUY', 'SELL')),
    quantity REAL NOT NULL,
    price REAL NOT NULL,
    pnl REAL,
    timestamp TEXT DEFAULT CURRENT_TIMESTAMP,
    linked_event_id INTEGER,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_paper_trades_symbol ON paper_trades(symbol);
CREATE INDEX IF NOT EXISTS idx_paper_trades_timestamp ON paper_trades(timestamp);

-- ============================================================================
-- AI Trading Simulator Tables
-- ============================================================================

-- AI Trading Sessions
CREATE TABLE IF NOT EXISTS ai_trading_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    start_time DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    end_time DATETIME,
    starting_portfolio_value REAL NOT NULL,
    ending_portfolio_value REAL,
    decisions_count INTEGER DEFAULT 0,
    trades_count INTEGER DEFAULT 0,
    session_notes TEXT,
    status TEXT DEFAULT 'active' CHECK(status IN ('active', 'completed', 'interrupted'))
);

CREATE INDEX IF NOT EXISTS idx_ai_sessions_status ON ai_trading_sessions(status);
CREATE INDEX IF NOT EXISTS idx_ai_sessions_start ON ai_trading_sessions(start_time);

-- AI Trade Decisions (detailed reasoning log)
CREATE TABLE IF NOT EXISTS ai_trade_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER REFERENCES ai_trading_sessions(id),
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    action TEXT NOT NULL CHECK(action IN ('BUY', 'SELL', 'HOLD')),
    symbol TEXT NOT NULL,
    quantity REAL,
    price_at_decision REAL,
    confidence REAL NOT NULL,
    reasoning TEXT NOT NULL,
    model_used TEXT NOT NULL,
    predicted_direction TEXT CHECK(predicted_direction IN ('bullish', 'bearish', 'neutral')),
    predicted_price_target REAL,
    predicted_timeframe_days INTEGER,
    actual_outcome TEXT,
    actual_price_at_timeframe REAL,
    prediction_accurate INTEGER,
    paper_trade_id INTEGER REFERENCES paper_trades(id)
);

CREATE INDEX IF NOT EXISTS idx_ai_decisions_session ON ai_trade_decisions(session_id);
CREATE INDEX IF NOT EXISTS idx_ai_decisions_symbol ON ai_trade_decisions(symbol);
CREATE INDEX IF NOT EXISTS idx_ai_decisions_timestamp ON ai_trade_decisions(timestamp);

-- AI Performance Snapshots (for charting equity curve)
CREATE TABLE IF NOT EXISTS ai_performance_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    portfolio_value REAL NOT NULL,
    cash REAL NOT NULL,
    positions_value REAL NOT NULL,
    benchmark_value REAL NOT NULL,
    benchmark_symbol TEXT DEFAULT 'SPY',
    total_pnl REAL NOT NULL,
    total_pnl_percent REAL NOT NULL,
    benchmark_pnl_percent REAL NOT NULL,
    prediction_accuracy REAL,
    trades_to_date INTEGER NOT NULL,
    winning_trades INTEGER NOT NULL,
    losing_trades INTEGER NOT NULL,
    win_rate REAL
);

CREATE INDEX IF NOT EXISTS idx_ai_performance_timestamp ON ai_performance_snapshots(timestamp);

-- AI Trader Configuration (singleton like paper_wallet)
CREATE TABLE IF NOT EXISTS ai_trader_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    starting_capital REAL NOT NULL DEFAULT 1000000.0,
    max_position_size_percent REAL NOT NULL DEFAULT 10.0,
    stop_loss_percent REAL NOT NULL DEFAULT 5.0,
    take_profit_percent REAL NOT NULL DEFAULT 15.0,
    session_duration_minutes INTEGER NOT NULL DEFAULT 60,
    benchmark_symbol TEXT NOT NULL DEFAULT 'SPY',
    model_priority TEXT NOT NULL DEFAULT 'deepseek-v3.2:cloud,gpt-oss:120b-cloud,qwen3:235b',
    -- Trading mode: 'aggressive', 'normal', 'conservative', 'paused'
    trading_mode TEXT NOT NULL DEFAULT 'normal',
    -- Circuit breaker settings
    daily_loss_threshold REAL NOT NULL DEFAULT -10.0,
    consecutive_loss_limit INTEGER NOT NULL DEFAULT 5,
    auto_conservative_on_trigger INTEGER NOT NULL DEFAULT 1,
    circuit_breaker_triggered INTEGER NOT NULL DEFAULT 0,
    circuit_breaker_until TIMESTAMP,
    -- STRYK override settings
    override_enabled INTEGER NOT NULL DEFAULT 0,
    override_expires_at TIMESTAMP,
    override_max_position_pct REAL,
    -- Guardrail settings
    max_daily_trades INTEGER NOT NULL DEFAULT 10,
    max_single_trade_value REAL NOT NULL DEFAULT 50000.0,
    require_confluence INTEGER NOT NULL DEFAULT 1,
    blocked_hours TEXT DEFAULT '09:30-09:45,15:45-16:00',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO ai_trader_config (id) VALUES (1);

-- Trade rejections audit log
CREATE TABLE IF NOT EXISTS trade_rejections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    session_id INTEGER,
    attempted_action TEXT NOT NULL,
    symbol TEXT NOT NULL,
    quantity REAL,
    quantity_percent REAL,
    estimated_value REAL,
    reason TEXT NOT NULL,
    rule_triggered TEXT NOT NULL,
    trading_mode TEXT,
    raw_request TEXT,
    FOREIGN KEY (session_id) REFERENCES ai_trading_sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_trade_rejections_timestamp ON trade_rejections(timestamp);
CREATE INDEX IF NOT EXISTS idx_trade_rejections_rule ON trade_rejections(rule_triggered);

-- Circuit breaker event log
CREATE TABLE IF NOT EXISTS circuit_breaker_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    trigger_type TEXT NOT NULL,
    previous_mode TEXT NOT NULL,
    new_mode TEXT NOT NULL,
    daily_pnl REAL,
    consecutive_losses INTEGER,
    resume_at TIMESTAMP,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_circuit_breaker_timestamp ON circuit_breaker_events(timestamp);

-- ============================================================================
-- DC TRADER TABLES (Separate from KALIC AI paper trading)
-- ============================================================================

-- DC trader wallet (mirrors paper_wallet structure)
CREATE TABLE IF NOT EXISTS dc_wallet (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    cash REAL NOT NULL DEFAULT 1000000.0,
    starting_capital REAL NOT NULL DEFAULT 1000000.0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- DC positions (mirrors paper_positions structure)
CREATE TABLE IF NOT EXISTS dc_positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    quantity REAL NOT NULL,
    entry_price REAL NOT NULL,
    entry_date TEXT DEFAULT CURRENT_TIMESTAMP
);

-- DC trades (mirrors paper_trades structure)
CREATE TABLE IF NOT EXISTS dc_trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('BUY', 'SELL')),
    quantity REAL NOT NULL,
    price REAL NOT NULL,
    pnl REAL,
    timestamp TEXT DEFAULT CURRENT_TIMESTAMP,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_dc_trades_timestamp ON dc_trades(timestamp);
CREATE INDEX IF NOT EXISTS idx_dc_trades_symbol ON dc_trades(symbol);

-- Portfolio snapshots for performance charting (both KALIC and DC)
CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    team TEXT NOT NULL CHECK(team IN ('KALIC', 'DC')),
    date TEXT NOT NULL,
    total_value REAL NOT NULL,
    cash REAL NOT NULL,
    positions_value REAL NOT NULL,
    UNIQUE(team, date)
);

CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_team_date ON portfolio_snapshots(team, date);

-- Team configurations (saveable competition presets)
CREATE TABLE IF NOT EXISTS trading_teams (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    description TEXT,
    kalic_starting_capital REAL DEFAULT 1000000.0,
    dc_starting_capital REAL DEFAULT 1000000.0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
"#;
