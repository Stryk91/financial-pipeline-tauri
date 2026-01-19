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
