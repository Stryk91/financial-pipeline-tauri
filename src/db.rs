//! SQLite database layer for Financial Pipeline

use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::Path;

use crate::error::Result;
use crate::models::{AlertCondition, DailyPrice, MacroData, PriceAlert, Symbol, TechnicalIndicator};

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
        println!("[OK] Database schema initialized");
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
"#;
