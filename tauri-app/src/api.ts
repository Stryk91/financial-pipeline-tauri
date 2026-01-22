// Tauri API wrapper

import { invoke } from '@tauri-apps/api/core';

// Types matching Rust structs
export interface SymbolPrice {
    symbol: string;
    price: number;
    change_percent: number;
    change_direction: string;
    favorited: boolean;
}

export interface CommandResult {
    success: boolean;
    message: string;
}

export interface IndicatorData {
    name: string;
    value: number;
    date: string;
}

export interface PriceData {
    date: string;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
}

export interface MacroData {
    indicator: string;
    value: number;
    date: string;
    source: string;
}

export interface Alert {
    id: number;
    symbol: string;
    target_price: number;
    condition: string;
    triggered: boolean;
}

export interface Position {
    id: number;
    symbol: string;
    quantity: number;
    price: number;
    position_type: string;
    date: string;
    current_price: number;
    current_value: number;
    profit_loss: number;
    profit_loss_percent: number;
}

export interface Portfolio {
    positions: Position[];
    total_value: number;
    total_profit_loss: number;
    total_profit_loss_percent: number;
}

// API functions
export async function getSymbols(): Promise<SymbolPrice[]> {
    return invoke('get_symbols');
}

export async function toggleFavorite(symbol: string): Promise<boolean> {
    return invoke('toggle_favorite', { symbol });
}

export async function getFavoritedSymbols(): Promise<string[]> {
    return invoke('get_favorited_symbols');
}

// Favorite all DC position symbols for auto-refresh
export async function favoriteDcPositions(): Promise<CommandResult> {
    return invoke('favorite_dc_positions');
}

// Favorite all KALIC position symbols for auto-refresh
export async function favoritePaperPositions(): Promise<CommandResult> {
    return invoke('favorite_paper_positions');
}

export async function fetchPrices(symbols: string, period: string): Promise<CommandResult> {
    return invoke('fetch_prices', { symbols, period });
}

export async function fetchFred(indicators: string): Promise<CommandResult> {
    return invoke('fetch_fred', { indicators });
}

export async function getMacroData(): Promise<MacroData[]> {
    return invoke('get_macro_data');
}

export async function calculateIndicators(symbol: string): Promise<CommandResult> {
    return invoke('calculate_indicators', { symbol });
}

export async function getIndicators(symbol: string): Promise<IndicatorData[]> {
    return invoke('get_indicators', { symbol });
}

export async function getIndicatorHistory(symbol: string, indicatorName: string): Promise<{ date: string; value: number }[]> {
    return invoke('get_indicator_history', { symbol, indicatorName });
}

export async function getPriceHistory(symbol: string): Promise<PriceData[]> {
    return invoke('get_price_history', { symbol });
}

export async function searchSymbol(query: string): Promise<string[]> {
    return invoke('search_symbol', { query });
}

export async function exportCsv(symbol: string): Promise<CommandResult> {
    return invoke('export_csv', { symbol });
}

// Alerts
export async function addAlert(symbol: string, targetPrice: number, condition: string): Promise<CommandResult> {
    return invoke('add_alert', { symbol, targetPrice, condition });
}

export async function getAlerts(onlyActive: boolean): Promise<Alert[]> {
    return invoke('get_alerts', { onlyActive });
}

export async function deleteAlert(alertId: number): Promise<CommandResult> {
    return invoke('delete_alert', { alertId });
}

export async function checkAlerts(): Promise<Alert[]> {
    return invoke('check_alerts');
}

// Portfolio
export async function addPosition(
    symbol: string,
    quantity: number,
    price: number,
    positionType: string,
    date: string,
    notes: string | null
): Promise<CommandResult> {
    return invoke('add_position', { symbol, quantity, price, positionType, date, notes });
}

export async function getPortfolio(): Promise<Portfolio> {
    return invoke('get_portfolio');
}

export async function deletePosition(positionId: number): Promise<CommandResult> {
    return invoke('delete_position', { positionId });
}

// Google Trends
export async function fetchTrends(keyword: string): Promise<CommandResult> {
    return invoke('fetch_trends', { keyword });
}

export async function getTrends(keyword: string): Promise<{ date: string; value: number }[]> {
    return invoke('get_trends', { keyword });
}

// Watchlists / Symbol Groups
export interface WatchlistSummary {
    id: number;
    name: string;
    description: string | null;
    symbol_count: number;
}

export interface WatchlistDetail {
    id: number;
    name: string;
    description: string | null;
    symbol_count: number;
    symbols: string[];
}

export async function createWatchlist(name: string, symbols: string[], description: string | null): Promise<CommandResult> {
    return invoke('create_watchlist', { name, symbols, description });
}

export async function getAllWatchlists(): Promise<WatchlistSummary[]> {
    return invoke('get_all_watchlists');
}

export async function getWatchlistDetail(name: string): Promise<WatchlistDetail | null> {
    return invoke('get_watchlist_detail', { name });
}

export async function deleteWatchlist(name: string): Promise<CommandResult> {
    return invoke('delete_watchlist', { name });
}

export async function addSymbolToWatchlist(watchlistName: string, symbol: string): Promise<CommandResult> {
    return invoke('add_symbol_to_watchlist', { watchlistName, symbol });
}

export async function removeSymbolFromWatchlist(watchlistName: string, symbol: string): Promise<CommandResult> {
    return invoke('remove_symbol_from_watchlist', { watchlistName, symbol });
}

export async function updateWatchlistDescription(name: string, description: string | null): Promise<CommandResult> {
    return invoke('update_watchlist_description', { name, description });
}

export async function renameWatchlist(oldName: string, newName: string): Promise<CommandResult> {
    return invoke('rename_watchlist', { oldName, newName });
}

// Vector Database / AI Search
export interface VectorSearchResult {
    id: string;
    content: string;
    score: number;
    result_type: string;
    symbol: string | null;
    date: string | null;
}

export interface VectorStats {
    events_count: number;
    patterns_count: number;
}

export async function vectorSearch(query: string, limit: number = 10): Promise<VectorSearchResult[]> {
    return invoke('vector_search', { query, limit });
}

export async function addMarketEvent(
    symbol: string,
    eventType: string,
    title: string,
    content: string,
    date: string,
    sentiment: number | null
): Promise<CommandResult> {
    return invoke('add_market_event', { symbol, eventType, title, content, date, sentiment });
}

export async function addPricePattern(
    symbol: string,
    patternType: string,
    startDate: string,
    endDate: string,
    priceChangePercent: number,
    volumeChangePercent: number,
    description: string
): Promise<CommandResult> {
    return invoke('add_price_pattern', {
        symbol,
        patternType,
        startDate,
        endDate,
        priceChangePercent,
        volumeChangePercent,
        description
    });
}

export async function getVectorStats(): Promise<VectorStats> {
    return invoke('get_vector_stats');
}

// Claude AI Chat
export interface ClaudeChatResponse {
    response: string;
    model: string;
    input_tokens: number;
    output_tokens: number;
    conversation_id: string;
}

export async function claudeChat(query: string, apiKey: string): Promise<ClaudeChatResponse> {
    return invoke('claude_chat', { query, apiKey });
}

export async function claudeQuery(query: string, apiKey: string): Promise<ClaudeChatResponse> {
    return invoke('claude_query', { query, apiKey });
}

// Finnhub News
export interface SimpleNewsItem {
    headline: string;
    summary: string;
    source: string;
    url: string;
    date: string;
    symbol: string;
}

export interface FetchNewsResponse {
    news: SimpleNewsItem[];
    count: number;
}

export async function fetchNews(symbol: string, apiKey: string, limit: number = 5): Promise<FetchNewsResponse> {
    return invoke('fetch_news', { symbol, apiKey, limit });
}

// Price Reaction (candle data around an event)
export interface PriceReactionResponse {
    symbol: string;
    event_date: string;
    start_date: string;
    end_date: string;
    pre_price: number;
    post_price: number;
    price_change_percent: number;
    volume_change_percent: number;
    candle_count: number;
}

export async function fetchPriceReaction(
    symbol: string,
    eventDate: string,
    apiKey: string,
    daysWindow: number = 3
): Promise<PriceReactionResponse> {
    return invoke('fetch_price_reaction', { symbol, eventDate, apiKey, daysWindow });
}

// Raw candle data
export interface CandleDataResponse {
    symbol: string;
    close: number[];
    high: number[];
    low: number[];
    open: number[];
    volume: number[];
    timestamp: number[];
    dates: string[];  // YYYY-MM-DD format
}

export async function fetchCandles(
    symbol: string,
    fromDate: string,
    toDate: string,
    apiKey: string,
    resolution: string = 'D'
): Promise<CandleDataResponse> {
    return invoke('fetch_candles', { symbol, fromDate, toDate, apiKey, resolution });
}

// Enhanced event saving with pattern linking
export interface EventWithPatternResponse {
    success: boolean;
    message: string;
    event_id: string;
    pattern_id: string | null;
    price_change_percent: number | null;
    pattern_error: string | null;  // Actual error reason for debugging
}

export async function addMarketEventWithPattern(
    symbol: string,
    eventType: string,
    title: string,
    content: string,
    date: string,
    sentiment: number | null,
    apiKey: string | null,
    linkPattern: boolean,
    daysWindow: number = 3
): Promise<EventWithPatternResponse> {
    return invoke('add_market_event_with_pattern', {
        symbol,
        eventType,
        title,
        content,
        date,
        sentiment,
        apiKey,
        linkPattern,
        daysWindow
    });
}

// Open article in lightweight Tauri webview window
export async function openArticleWindow(url: string, title: string): Promise<void> {
    return invoke('open_article_window', { url, title });
}

// ============================================================================
// PAPER TRADING
// ============================================================================

export interface PaperWalletBalance {
    cash: number;
    positions_value: number;
    total_equity: number;
    starting_capital: number;
    total_pnl: number;
    total_pnl_percent: number;
}

export interface PaperPosition {
    id: number;
    symbol: string;
    quantity: number;
    entry_price: number;
    entry_date: string;
    current_price: number;
    current_value: number;
    cost_basis: number;
    unrealized_pnl: number;
    unrealized_pnl_percent: number;
}

export interface PaperTrade {
    id: number;
    symbol: string;
    action: 'BUY' | 'SELL';
    quantity: number;
    price: number;
    pnl: number | null;
    timestamp: string;
    notes: string | null;
}

// Get paper trading balance and portfolio summary
export async function getPaperBalance(): Promise<PaperWalletBalance> {
    return invoke('get_paper_balance');
}

// Get all paper trading positions with current values
export async function getPaperPositions(): Promise<PaperPosition[]> {
    return invoke('get_paper_positions');
}

// Execute a paper trade (BUY or SELL)
export async function executePaperTrade(
    symbol: string,
    action: 'BUY' | 'SELL',
    quantity: number,
    price?: number,
    notes?: string
): Promise<PaperTrade> {
    return invoke('execute_paper_trade', { symbol, action, quantity, price, notes });
}

// Get paper trade history
export async function getPaperTrades(symbol?: string, limit?: number): Promise<PaperTrade[]> {
    return invoke('get_paper_trades', { symbol, limit });
}

// Reset paper trading account
export async function resetPaperAccount(startingCash?: number): Promise<CommandResult> {
    return invoke('reset_paper_account', { startingCash });
}

// ============================================================================
// DC TRADER (Separate from KALIC AI paper trading)
// ============================================================================

export interface DcWalletBalance {
    cash: number;
    positions_value: number;
    total_equity: number;
    starting_capital: number;
    total_pnl: number;
    total_pnl_percent: number;
}

export interface DcPosition {
    id: number;
    symbol: string;
    quantity: number;
    entry_price: number;
    entry_date: string;
    current_price: number;
    current_value: number;
    cost_basis: number;
    unrealized_pnl: number;
    unrealized_pnl_percent: number;
}

export interface DcTrade {
    id: number;
    symbol: string;
    action: 'BUY' | 'SELL';
    quantity: number;
    price: number;
    pnl: number | null;
    timestamp: string;
    notes: string | null;
}

export interface ImportResult {
    success_count: number;
    error_count: number;
    errors: string[];
}

export interface PortfolioSnapshot {
    id: number;
    team: string;
    date: string;
    total_value: number;
    cash: number;
    positions_value: number;
}

export interface TeamConfig {
    id: number;
    name: string;
    description: string | null;
    kalic_starting_capital: number;
    dc_starting_capital: number;
    created_at: string;
}

export interface CompetitionStats {
    kalic_total: number;
    kalic_cash: number;
    kalic_positions: number;
    kalic_pnl_pct: number;
    kalic_trades: number;
    dc_total: number;
    dc_cash: number;
    dc_positions: number;
    dc_pnl_pct: number;
    dc_trades: number;
    leader: string;
    lead_amount: number;
}

// Get DC wallet balance and portfolio summary
export async function getDcBalance(): Promise<DcWalletBalance> {
    return invoke('get_dc_balance');
}

// Get all DC positions with current values
export async function getDcPositions(): Promise<DcPosition[]> {
    return invoke('get_dc_positions');
}

// Execute a DC trade (BUY or SELL)
export async function executeDcTrade(
    symbol: string,
    action: 'BUY' | 'SELL',
    quantity: number,
    price?: number,
    notes?: string
): Promise<DcTrade> {
    return invoke('execute_dc_trade', { symbol, action, quantity, price, notes });
}

// Get DC trade history
export async function getDcTrades(limit?: number): Promise<DcTrade[]> {
    return invoke('get_dc_trades', { limit });
}

// Reset DC trading account
export async function resetDcAccount(startingCash?: number): Promise<CommandResult> {
    return invoke('reset_dc_account', { starting_cash: startingCash });
}

// Import DC trades from CSV
export async function importDcTradesCsv(csvContent: string): Promise<ImportResult> {
    return invoke('import_dc_trades_csv', { csvContent });
}

// Import DC trades from JSON
export async function importDcTradesJson(jsonContent: string): Promise<ImportResult> {
    return invoke('import_dc_trades_json', { jsonContent });
}

// Lookup current price for a symbol
export async function lookupCurrentPrice(symbol: string): Promise<number> {
    return invoke('lookup_current_price', { symbol });
}

// Record portfolio snapshot for a team
export async function recordPortfolioSnapshot(team: 'KALIC' | 'DC'): Promise<CommandResult> {
    return invoke('record_portfolio_snapshot', { team });
}

// Get portfolio snapshots for charting
export async function getPortfolioSnapshots(team?: 'KALIC' | 'DC', days?: number): Promise<PortfolioSnapshot[]> {
    return invoke('get_portfolio_snapshots', { team, days });
}

// Save team configuration
export async function saveTeamConfig(name: string, description?: string): Promise<number> {
    return invoke('save_team_config', { name, description });
}

// Load team configuration
export async function loadTeamConfig(name: string): Promise<TeamConfig> {
    return invoke('load_team_config', { name });
}

// List all team configurations
export async function listTeamConfigs(): Promise<TeamConfig[]> {
    return invoke('list_team_configs');
}

// Get competition stats
export async function getCompetitionStats(): Promise<CompetitionStats> {
    return invoke('get_competition_stats');
}

// ============================================================================
// AI TRADER
// ============================================================================

export interface AiTradingSession {
    id: number;
    start_time: string;
    end_time: string | null;
    starting_portfolio_value: number;
    ending_portfolio_value: number | null;
    decisions_count: number;
    trades_count: number;
    session_notes: string | null;
    status: string;
}

export interface AiTradeDecision {
    id: number;
    session_id: number | null;
    timestamp: string;
    action: string;
    symbol: string;
    quantity: number | null;
    price_at_decision: number | null;
    confidence: number;
    reasoning: string;
    model_used: string;
    predicted_direction: string | null;
    predicted_price_target: number | null;
    predicted_timeframe_days: number | null;
    actual_outcome: string | null;
    actual_price_at_timeframe: number | null;
    prediction_accurate: boolean | null;
    paper_trade_id: number | null;
}

export interface AiPerformanceSnapshot {
    id: number;
    timestamp: string;
    portfolio_value: number;
    cash: number;
    positions_value: number;
    benchmark_value: number;
    benchmark_symbol: string;
    total_pnl: number;
    total_pnl_percent: number;
    benchmark_pnl_percent: number;
    prediction_accuracy: number | null;
    trades_to_date: number;
    winning_trades: number;
    losing_trades: number;
    win_rate: number | null;
}

export interface AiTraderStatus {
    is_running: boolean;
    current_session: AiTradingSession | null;
    portfolio_value: number;
    cash: number;
    positions_value: number;
    is_bankrupt: boolean;
    sessions_completed: number;
    total_decisions: number;
    total_trades: number;
}

export interface AiBenchmarkComparison {
    portfolio_return_percent: number;
    benchmark_return_percent: number;
    alpha: number;
    tracking_data: [string, number, number][]; // [timestamp, portfolio, benchmark]
}

export interface AiCompoundingForecast {
    current_daily_return: number;
    current_win_rate: number;
    projected_30_days: number;
    projected_90_days: number;
    projected_365_days: number;
    time_to_double: number | null;
    time_to_bankruptcy: number | null;
}

export interface AiPredictionAccuracy {
    total_predictions: number;
    accurate_predictions: number;
    accuracy_percent: number;
}

export interface AiTraderConfig {
    starting_capital: number;
    max_position_size_percent: number;
    stop_loss_percent: number;
    take_profit_percent: number;
    session_duration_minutes: number;
    benchmark_symbol: string;
    model_priority: string[];
}

// Get AI trader status
export async function aiTraderGetStatus(): Promise<AiTraderStatus> {
    return invoke('ai_trader_get_status');
}

// Get AI trader configuration
export async function aiTraderGetConfig(): Promise<AiTraderConfig> {
    return invoke('ai_trader_get_config');
}

// Start a new AI trading session
export async function aiTraderStartSession(): Promise<AiTradingSession> {
    return invoke('ai_trader_start_session');
}

// End the current AI trading session
export async function aiTraderEndSession(notes?: string): Promise<AiTradingSession | null> {
    return invoke('ai_trader_end_session', { notes });
}

// Run one AI trading cycle
export async function aiTraderRunCycle(): Promise<AiTradeDecision[]> {
    return invoke('ai_trader_run_cycle');
}

// Get AI trading decisions
export async function aiTraderGetDecisions(
    sessionId?: number,
    symbol?: string,
    limit?: number
): Promise<AiTradeDecision[]> {
    return invoke('ai_trader_get_decisions', { sessionId, symbol, limit });
}

// Get AI performance history (snapshots)
export async function aiTraderGetPerformanceHistory(days?: number): Promise<AiPerformanceSnapshot[]> {
    return invoke('ai_trader_get_performance_history', { days });
}

// Get benchmark comparison
export async function aiTraderGetBenchmarkComparison(): Promise<AiBenchmarkComparison> {
    return invoke('ai_trader_get_benchmark_comparison');
}

// Get compounding forecast
export async function aiTraderGetCompoundingForecast(): Promise<AiCompoundingForecast> {
    return invoke('ai_trader_get_compounding_forecast');
}

// Get prediction accuracy
export async function aiTraderGetPredictionAccuracy(): Promise<AiPredictionAccuracy> {
    return invoke('ai_trader_get_prediction_accuracy');
}

// Evaluate pending predictions
export async function aiTraderEvaluatePredictions(): Promise<number> {
    return invoke('ai_trader_evaluate_predictions');
}

// Reset AI trading
export async function aiTraderReset(startingCapital?: number): Promise<CommandResult> {
    return invoke('ai_trader_reset', { startingCapital });
}
