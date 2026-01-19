//! Backtesting Engine
//!
//! Simulates trading strategies against historical data

use crate::models::{
    BacktestResult, BacktestTrade, DailyPrice, PerformanceMetrics, Strategy, StrategyConditionType,
    TechnicalIndicator, TradeDirection,
};
use chrono::NaiveDate;
use std::collections::HashMap;

/// Backtest configuration
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub initial_capital: f64,
    pub commission_per_trade: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 10000.0,
            commission_per_trade: 0.0,
        }
    }
}

/// Open position during backtest
#[derive(Debug, Clone)]
struct OpenPosition {
    entry_date: NaiveDate,
    entry_price: f64,
    shares: f64,
    entry_reason: String,
}

/// Main backtesting engine
pub struct BacktestEngine {
    config: BacktestConfig,
}

impl Default for BacktestEngine {
    fn default() -> Self {
        Self::new(BacktestConfig::default())
    }
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self { config }
    }

    /// Build indicator map by date for O(1) lookups
    fn build_indicator_map(
        &self,
        indicators: &[TechnicalIndicator],
    ) -> HashMap<NaiveDate, HashMap<String, f64>> {
        let mut map: HashMap<NaiveDate, HashMap<String, f64>> = HashMap::new();

        for ind in indicators {
            map.entry(ind.date)
                .or_default()
                .insert(ind.indicator_name.clone(), ind.value);
        }

        map
    }

    /// Check if entry condition is met
    fn check_entry_condition(
        &self,
        strategy: &Strategy,
        price: f64,
        today: &HashMap<String, f64>,
        prev: Option<&HashMap<String, f64>>,
    ) -> bool {
        match strategy.entry_condition {
            StrategyConditionType::RsiOversold => {
                today.get("RSI_14").map_or(false, |&rsi| rsi < strategy.entry_threshold)
            }
            StrategyConditionType::RsiOverbought => {
                today.get("RSI_14").map_or(false, |&rsi| rsi > strategy.entry_threshold)
            }
            StrategyConditionType::MacdCrossUp => {
                if let (Some(prev_ind), Some(macd), Some(signal)) = (
                    prev,
                    today.get("MACD_12_26"),
                    today.get("MACD_SIGNAL_9"),
                ) {
                    if let (Some(&prev_macd), Some(&prev_signal)) = (
                        prev_ind.get("MACD_12_26"),
                        prev_ind.get("MACD_SIGNAL_9"),
                    ) {
                        prev_macd <= prev_signal && *macd > *signal
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            StrategyConditionType::MacdCrossDown => {
                if let (Some(prev_ind), Some(macd), Some(signal)) = (
                    prev,
                    today.get("MACD_12_26"),
                    today.get("MACD_SIGNAL_9"),
                ) {
                    if let (Some(&prev_macd), Some(&prev_signal)) = (
                        prev_ind.get("MACD_12_26"),
                        prev_ind.get("MACD_SIGNAL_9"),
                    ) {
                        prev_macd >= prev_signal && *macd < *signal
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            StrategyConditionType::PriceAboveSma => {
                today.get("SMA_20").map_or(false, |&sma| price > sma)
            }
            StrategyConditionType::PriceBelowSma => {
                today.get("SMA_20").map_or(false, |&sma| price < sma)
            }
            StrategyConditionType::SmaCrossUp => {
                if let (Some(prev_ind), Some(&fast), Some(&slow)) =
                    (prev, today.get("SMA_20"), today.get("SMA_50"))
                {
                    if let (Some(&prev_fast), Some(&prev_slow)) =
                        (prev_ind.get("SMA_20"), prev_ind.get("SMA_50"))
                    {
                        prev_fast <= prev_slow && fast > slow
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            StrategyConditionType::SmaCrossDown => {
                if let (Some(prev_ind), Some(&fast), Some(&slow)) =
                    (prev, today.get("SMA_20"), today.get("SMA_50"))
                {
                    if let (Some(&prev_fast), Some(&prev_slow)) =
                        (prev_ind.get("SMA_20"), prev_ind.get("SMA_50"))
                    {
                        prev_fast >= prev_slow && fast < slow
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            // StopLoss and TakeProfit are exit-only conditions
            StrategyConditionType::StopLoss | StrategyConditionType::TakeProfit => false,
        }
    }

    /// Check if exit condition is met
    fn check_exit_condition(
        &self,
        strategy: &Strategy,
        price: f64,
        entry_price: f64,
        today: &HashMap<String, f64>,
        prev: Option<&HashMap<String, f64>>,
    ) -> (bool, String) {
        // Check stop loss
        if let Some(stop_loss_pct) = strategy.stop_loss_percent {
            let stop_price = entry_price * (1.0 - stop_loss_pct / 100.0);
            if price <= stop_price {
                return (true, "stop_loss".to_string());
            }
        }

        // Check take profit
        if let Some(take_profit_pct) = strategy.take_profit_percent {
            let target_price = entry_price * (1.0 + take_profit_pct / 100.0);
            if price >= target_price {
                return (true, "take_profit".to_string());
            }
        }

        // Check strategy exit condition
        let condition_met = match strategy.exit_condition {
            StrategyConditionType::RsiOversold => {
                today.get("RSI_14").map_or(false, |&rsi| rsi < strategy.exit_threshold)
            }
            StrategyConditionType::RsiOverbought => {
                today.get("RSI_14").map_or(false, |&rsi| rsi > strategy.exit_threshold)
            }
            StrategyConditionType::MacdCrossUp => {
                if let (Some(prev_ind), Some(macd), Some(signal)) = (
                    prev,
                    today.get("MACD_12_26"),
                    today.get("MACD_SIGNAL_9"),
                ) {
                    if let (Some(&prev_macd), Some(&prev_signal)) = (
                        prev_ind.get("MACD_12_26"),
                        prev_ind.get("MACD_SIGNAL_9"),
                    ) {
                        prev_macd <= prev_signal && *macd > *signal
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            StrategyConditionType::MacdCrossDown => {
                if let (Some(prev_ind), Some(macd), Some(signal)) = (
                    prev,
                    today.get("MACD_12_26"),
                    today.get("MACD_SIGNAL_9"),
                ) {
                    if let (Some(&prev_macd), Some(&prev_signal)) = (
                        prev_ind.get("MACD_12_26"),
                        prev_ind.get("MACD_SIGNAL_9"),
                    ) {
                        prev_macd >= prev_signal && *macd < *signal
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            StrategyConditionType::PriceAboveSma => {
                today.get("SMA_20").map_or(false, |&sma| price > sma)
            }
            StrategyConditionType::PriceBelowSma => {
                today.get("SMA_20").map_or(false, |&sma| price < sma)
            }
            StrategyConditionType::SmaCrossUp => {
                if let (Some(prev_ind), Some(&fast), Some(&slow)) =
                    (prev, today.get("SMA_20"), today.get("SMA_50"))
                {
                    if let (Some(&prev_fast), Some(&prev_slow)) =
                        (prev_ind.get("SMA_20"), prev_ind.get("SMA_50"))
                    {
                        prev_fast <= prev_slow && fast > slow
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            StrategyConditionType::SmaCrossDown => {
                if let (Some(prev_ind), Some(&fast), Some(&slow)) =
                    (prev, today.get("SMA_20"), today.get("SMA_50"))
                {
                    if let (Some(&prev_fast), Some(&prev_slow)) =
                        (prev_ind.get("SMA_20"), prev_ind.get("SMA_50"))
                    {
                        prev_fast >= prev_slow && fast < slow
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            StrategyConditionType::StopLoss | StrategyConditionType::TakeProfit => false,
        };

        if condition_met {
            (true, strategy.exit_condition.as_str().to_string())
        } else {
            (false, String::new())
        }
    }

    /// Run a backtest
    pub fn run(
        &self,
        strategy: &Strategy,
        symbol: &str,
        prices: &[DailyPrice],
        indicators: &[TechnicalIndicator],
    ) -> BacktestResult {
        let indicator_map = self.build_indicator_map(indicators);

        let mut cash = self.config.initial_capital;
        let mut position: Option<OpenPosition> = None;
        let mut trades: Vec<BacktestTrade> = Vec::new();
        let mut equity_history: Vec<f64> = Vec::new();

        // Sort prices by date
        let mut sorted_prices = prices.to_vec();
        sorted_prices.sort_by_key(|p| p.date);

        // Walk through each day
        for (i, price_data) in sorted_prices.iter().enumerate() {
            let date = price_data.date;
            let price = price_data.close;

            let today_indicators = indicator_map.get(&date);
            let prev_indicators = if i > 0 {
                indicator_map.get(&sorted_prices[i - 1].date)
            } else {
                None
            };

            // Calculate current equity
            let current_equity = if let Some(ref pos) = position {
                cash + pos.shares * price
            } else {
                cash
            };
            equity_history.push(current_equity);

            // Skip if no indicators for today
            let Some(today) = today_indicators else {
                continue;
            };

            // If we have a position, check exit conditions
            if let Some(ref pos) = position {
                let (should_exit, exit_reason) =
                    self.check_exit_condition(strategy, price, pos.entry_price, today, prev_indicators);

                if should_exit {
                    // Close position
                    let profit_loss = (price - pos.entry_price) * pos.shares - self.config.commission_per_trade;
                    let profit_loss_percent = (price - pos.entry_price) / pos.entry_price * 100.0;

                    cash += pos.shares * price - self.config.commission_per_trade;

                    trades.push(BacktestTrade {
                        id: 0,
                        backtest_id: 0,
                        symbol: symbol.to_string(),
                        direction: TradeDirection::Long,
                        entry_date: pos.entry_date,
                        entry_price: pos.entry_price,
                        exit_date: Some(date),
                        exit_price: Some(price),
                        shares: pos.shares,
                        entry_reason: pos.entry_reason.clone(),
                        exit_reason: Some(exit_reason),
                        profit_loss: Some(profit_loss),
                        profit_loss_percent: Some(profit_loss_percent),
                    });

                    position = None;
                }
            }

            // If no position, check entry conditions
            if position.is_none() {
                if self.check_entry_condition(strategy, price, today, prev_indicators) {
                    // Open position
                    let position_value = cash * (strategy.position_size_percent / 100.0);
                    let shares = (position_value - self.config.commission_per_trade) / price;

                    if shares > 0.0 {
                        cash -= shares * price + self.config.commission_per_trade;

                        position = Some(OpenPosition {
                            entry_date: date,
                            entry_price: price,
                            shares,
                            entry_reason: strategy.entry_condition.as_str().to_string(),
                        });
                    }
                }
            }
        }

        // Close any remaining position at end
        if let Some(pos) = position {
            if let Some(last_price) = sorted_prices.last() {
                let profit_loss =
                    (last_price.close - pos.entry_price) * pos.shares - self.config.commission_per_trade;
                let profit_loss_percent =
                    (last_price.close - pos.entry_price) / pos.entry_price * 100.0;

                cash += pos.shares * last_price.close;

                trades.push(BacktestTrade {
                    id: 0,
                    backtest_id: 0,
                    symbol: symbol.to_string(),
                    direction: TradeDirection::Long,
                    entry_date: pos.entry_date,
                    entry_price: pos.entry_price,
                    exit_date: Some(last_price.date),
                    exit_price: Some(last_price.close),
                    shares: pos.shares,
                    entry_reason: pos.entry_reason,
                    exit_reason: Some("end_of_data".to_string()),
                    profit_loss: Some(profit_loss),
                    profit_loss_percent: Some(profit_loss_percent),
                });
            }
        }

        // Calculate metrics
        let metrics = self.calculate_metrics(&trades, &equity_history);

        let start_date = sorted_prices.first().map(|p| p.date).unwrap_or_else(|| {
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
        });
        let end_date = sorted_prices.last().map(|p| p.date).unwrap_or_else(|| {
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
        });

        BacktestResult {
            id: 0,
            strategy_id: strategy.id,
            strategy_name: strategy.name.clone(),
            symbol: symbol.to_string(),
            start_date,
            end_date,
            initial_capital: self.config.initial_capital,
            final_capital: cash,
            metrics,
            trades,
            created_at: String::new(),
        }
    }

    /// Calculate performance metrics
    fn calculate_metrics(&self, trades: &[BacktestTrade], equity_history: &[f64]) -> PerformanceMetrics {
        let initial = self.config.initial_capital;
        let final_equity = *equity_history.last().unwrap_or(&initial);

        let total_return_dollars = final_equity - initial;
        let total_return = (total_return_dollars / initial) * 100.0;

        // Max drawdown
        let mut max_drawdown = 0.0;
        let mut peak = initial;
        for &equity in equity_history {
            if equity > peak {
                peak = equity;
            }
            let drawdown = (peak - equity) / peak * 100.0;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        // Trade statistics
        let winning_trades: Vec<_> = trades
            .iter()
            .filter(|t| t.profit_loss.unwrap_or(0.0) > 0.0)
            .collect();
        let losing_trades: Vec<_> = trades
            .iter()
            .filter(|t| t.profit_loss.unwrap_or(0.0) < 0.0)
            .collect();

        let total_trades = trades.len();
        let num_winners = winning_trades.len();
        let num_losers = losing_trades.len();

        let win_rate = if total_trades > 0 {
            (num_winners as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let avg_win = if !winning_trades.is_empty() {
            winning_trades
                .iter()
                .map(|t| t.profit_loss_percent.unwrap_or(0.0))
                .sum::<f64>()
                / winning_trades.len() as f64
        } else {
            0.0
        };

        let avg_loss = if !losing_trades.is_empty() {
            losing_trades
                .iter()
                .map(|t| t.profit_loss_percent.unwrap_or(0.0).abs())
                .sum::<f64>()
                / losing_trades.len() as f64
        } else {
            0.0
        };

        let gross_profit: f64 = winning_trades
            .iter()
            .map(|t| t.profit_loss.unwrap_or(0.0))
            .sum();
        let gross_loss: f64 = losing_trades
            .iter()
            .map(|t| t.profit_loss.unwrap_or(0.0).abs())
            .sum();

        let profit_factor = if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else if gross_profit > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // Average trade duration
        let avg_duration = if !trades.is_empty() {
            trades
                .iter()
                .filter_map(|t| {
                    t.exit_date.map(|exit| {
                        (exit - t.entry_date).num_days() as f64
                    })
                })
                .sum::<f64>()
                / trades.len() as f64
        } else {
            0.0
        };

        // Simple Sharpe ratio approximation (assuming 252 trading days)
        let daily_returns: Vec<f64> = equity_history
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();

        let avg_return = if !daily_returns.is_empty() {
            daily_returns.iter().sum::<f64>() / daily_returns.len() as f64
        } else {
            0.0
        };

        let std_dev = if daily_returns.len() > 1 {
            let variance = daily_returns
                .iter()
                .map(|r| (r - avg_return).powi(2))
                .sum::<f64>()
                / daily_returns.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };

        let sharpe_ratio = if std_dev > 0.0 {
            (avg_return / std_dev) * (252.0_f64).sqrt()
        } else {
            0.0
        };

        PerformanceMetrics {
            total_return,
            total_return_dollars,
            max_drawdown,
            sharpe_ratio,
            win_rate,
            total_trades,
            winning_trades: num_winners,
            losing_trades: num_losers,
            avg_win_percent: avg_win,
            avg_loss_percent: avg_loss,
            profit_factor,
            avg_trade_duration_days: avg_duration,
        }
    }
}
