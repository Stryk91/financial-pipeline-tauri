// Main entry point
import './styles.css';
import * as api from './api';
import { TradingViewChart, IndicatorChart } from './chart';

// =============================================================================
// SECURITY: HTML Sanitization to prevent XSS attacks
// =============================================================================

/**
 * Escapes HTML special characters to prevent XSS
 * ALWAYS use this when inserting user-controlled data into innerHTML
 */
function escapeHtml(unsafe: string): string {
    if (typeof unsafe !== 'string') return String(unsafe);
    return unsafe
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}

/**
 * Sanitizes a number for display (prevents NaN injection)
 */
function sanitizeNumber(value: number, decimals: number = 2): string {
    if (typeof value !== 'number' || isNaN(value)) return '0.00';
    return value.toFixed(decimals);
}

/**
 * Validates and sanitizes a symbol string (uppercase alphanumeric + dots/dashes only)
 */
function sanitizeSymbol(symbol: string): string {
    if (typeof symbol !== 'string') return '';
    return symbol.toUpperCase().replace(/[^A-Z0-9.\-]/g, '').substring(0, 20);
}

// S&P 100 Symbol List (OEX constituents)
const SP100_SYMBOLS = [
    'AAPL', 'ABBV', 'ABT', 'ACN', 'ADBE', 'AIG', 'AMD', 'AMGN', 'AMT', 'AMZN',
    'AVGO', 'AXP', 'BA', 'BAC', 'BK', 'BKNG', 'BLK', 'BMY', 'BRK.B', 'C',
    'CAT', 'CHTR', 'CL', 'CMCSA', 'COF', 'COP', 'COST', 'CRM', 'CSCO', 'CVS',
    'CVX', 'DE', 'DHR', 'DIS', 'DOW', 'DUK', 'EMR', 'EXC', 'F', 'FDX',
    'GD', 'GE', 'GILD', 'GM', 'GOOG', 'GOOGL', 'GS', 'HD', 'HON', 'IBM',
    'INTC', 'JNJ', 'JPM', 'KHC', 'KO', 'LIN', 'LLY', 'LMT', 'LOW', 'MA',
    'MCD', 'MDLZ', 'MDT', 'MET', 'META', 'MMM', 'MO', 'MRK', 'MS', 'MSFT',
    'NEE', 'NFLX', 'NKE', 'NVDA', 'ORCL', 'PEP', 'PFE', 'PG', 'PM', 'PYPL',
    'QCOM', 'RTX', 'SBUX', 'SCHW', 'SO', 'SPG', 'T', 'TGT', 'TMO', 'TMUS',
    'TSLA', 'TXN', 'UNH', 'UNP', 'UPS', 'USB', 'V', 'VZ', 'WFC', 'WMT', 'XOM'
];

// ASX 100 Symbol List (with .AX suffix for Yahoo Finance)
const ASX100_SYMBOLS = [
    'BHP.AX', 'CBA.AX', 'CSL.AX', 'NAB.AX', 'WBC.AX', 'ANZ.AX', 'WES.AX', 'MQG.AX', 'FMG.AX', 'WDS.AX',
    'TLS.AX', 'RIO.AX', 'WOW.AX', 'GMG.AX', 'TCL.AX', 'STO.AX', 'ALL.AX', 'QBE.AX', 'REA.AX', 'COL.AX',
    'SUN.AX', 'JHX.AX', 'RMD.AX', 'NCM.AX', 'AMC.AX', 'IAG.AX', 'ORG.AX', 'AGL.AX', 'S32.AX', 'APA.AX',
    'MIN.AX', 'XRO.AX', 'TWE.AX', 'ASX.AX', 'CPU.AX', 'QAN.AX', 'SHL.AX', 'SOL.AX', 'AZJ.AX', 'DXS.AX',
    'FPH.AX', 'GPT.AX', 'SCG.AX', 'SEK.AX', 'MPL.AX', 'ORI.AX', 'EVN.AX', 'NST.AX', 'ILU.AX', 'ALQ.AX',
    'ALD.AX', 'JBH.AX', 'COH.AX', 'OZL.AX', 'WHC.AX', 'CTX.AX', 'EDV.AX', 'NHF.AX', 'BXB.AX', 'SVW.AX',
    'BEN.AX', 'MGR.AX', 'VCX.AX', 'BSL.AX', 'SDF.AX', 'LLC.AX', 'CAR.AX', 'IGO.AX', 'AMP.AX', 'NEC.AX',
    'WOR.AX', 'REH.AX', 'CCL.AX', 'BOQ.AX', 'TAH.AX', 'HVN.AX', 'ALU.AX', 'IPL.AX', 'NWS.AX', 'SGP.AX',
    'FLT.AX', 'PME.AX', 'CWN.AX', 'PLS.AX', 'LYC.AX', 'AWC.AX', 'WEB.AX', 'CGF.AX', 'SFR.AX', 'PDN.AX',
    'NXT.AX', 'VEA.AX', 'IEL.AX', 'APE.AX', 'HUB.AX', 'TLC.AX', 'WTC.AX', 'CCP.AX', 'LNK.AX', 'ABC.AX'
];

// LocalStorage keys
const STORAGE_KEYS = {
    AUTO_REFRESH_ENABLED: 'fp_auto_refresh_enabled',
    AUTO_REFRESH_INTERVAL: 'fp_auto_refresh_interval',
};

// State
let tvChart: TradingViewChart | null = null;
let indicatorChart: IndicatorChart | null = null;
let autoRefreshTimer: number | null = null;
let selectedGroupName: string | null = null;

// Logger
function log(message: string, type: 'info' | 'success' | 'error' = 'info'): void {
    const container = document.getElementById('log-container');
    if (!container) return;

    const entry = document.createElement('div');
    entry.className = `log-entry log-${type}`;
    entry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
    container.appendChild(entry);
    container.scrollTop = container.scrollHeight;

    console.log(`[${type.toUpperCase()}] ${message}`);
}

// Tab management
function switchTab(tabName: string): void {
    // Update tab buttons
    document.querySelectorAll('.tab').forEach(tab => {
        tab.classList.toggle('active', tab.getAttribute('data-tab') === tabName);
    });

    // Update tab content
    document.querySelectorAll('.tab-content').forEach(content => {
        const id = content.id.replace('-tab', '');
        (content as HTMLElement).style.display = id === tabName ? 'block' : 'none';
    });

    // Load data for specific tabs
    if (tabName === 'alerts') loadAlerts();
    if (tabName === 'portfolio') loadPortfolio();
    if (tabName === 'macro') loadMacroData();
    if (tabName === 'chart') initializeChart();
    if (tabName === 'indicators') initializeIndicatorChart();
    if (tabName === 'groups') loadGroups();
    if (tabName === 'ai-search') loadVectorStats();
    if (tabName === 'ai-trader') loadAiTrader();
    if (tabName === 'dc-trader') loadDcTrader();
}

// Initialize TradingView chart
function initializeChart(): void {
    if (!tvChart) {
        try {
            tvChart = new TradingViewChart('tradingview-chart', 'volume-chart');
            tvChart.initialize();
            log('TradingView chart initialized', 'success');
        } catch (error) {
            log(`Chart init error: ${error}`, 'error');
        }
    }
}

function initializeIndicatorChart(): void {
    if (!indicatorChart) {
        try {
            indicatorChart = new IndicatorChart('indicator-chart-container');
            indicatorChart.initialize();
        } catch (error) {
            log(`Indicator chart init error: ${error}`, 'error');
        }
    }
}

// Symbol list
async function refreshSymbolList(): Promise<void> {
    try {
        log('Refreshing symbol list...', 'info');
        const symbols = await api.getSymbols();
        const list = document.getElementById('symbol-list');
        const sortSelect = document.getElementById('sort-select') as HTMLSelectElement;

        if (!list) return;

        if (symbols && symbols.length > 0) {
            // Sort
            const sortOption = sortSelect?.value || 'change-desc';
            symbols.sort((a, b) => {
                switch (sortOption) {
                    case 'change-desc': return b.change_percent - a.change_percent;
                    case 'change-asc': return a.change_percent - b.change_percent;
                    case 'price-desc': return b.price - a.price;
                    case 'price-asc': return a.price - b.price;
                    case 'symbol-asc': return a.symbol.localeCompare(b.symbol);
                    case 'symbol-desc': return b.symbol.localeCompare(a.symbol);
                    default: return b.change_percent - a.change_percent;
                }
            });

            list.innerHTML = symbols.map(s => {
                const changeColor = s.change_direction === 'up' ? 'price-up' :
                                   s.change_direction === 'down' ? 'price-down' : 'price-unchanged';
                const changeSign = s.change_percent >= 0 ? '+' : '';
                const arrow = s.change_direction === 'up' ? '▲' :
                             s.change_direction === 'down' ? '▼' : '';
                const favText = s.favorited ? '🌙' : '☽';
                const favClass = s.favorited ? 'favorited' : '';

                return `
                    <li class="symbol-item" data-symbol="${s.symbol}">
                        <div style="display: flex; align-items: center; gap: 8px;">
                            <button class="favorite-toggle ${favClass}" data-symbol="${s.symbol}" title="Toggle auto-refresh">${favText}</button>
                            <span class="symbol-ticker">${s.symbol}</span>
                        </div>
                        <div>
                            <span class="symbol-price ${changeColor}">$${s.price.toFixed(2)}</span>
                            <span class="${changeColor}" style="margin-left: 8px;">
                                ${arrow} ${changeSign}${s.change_percent.toFixed(2)}%
                            </span>
                        </div>
                    </li>
                `;
            }).join('');

            document.getElementById('symbol-count')!.textContent = symbols.length.toString();
            log(`Loaded ${symbols.length} symbols`, 'success');
        } else {
            list.innerHTML = '<li class="empty-state">No data loaded. Fetch some symbols to get started.</li>';
        }
    } catch (error) {
        log(`Error refreshing: ${error}`, 'error');
    }
}

// Fetch prices
async function fetchPrices(symbols: string, period: string): Promise<void> {
    try {
        log(`Fetching ${symbols} (${period})...`, 'info');
        const result = await api.fetchPrices(symbols, period);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);
        await refreshSymbolList();
    } catch (error) {
        log(`Error fetching: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

// Fetch FRED macro data
async function fetchFred(indicators: string): Promise<void> {
    try {
        log(`Fetching FRED indicators: ${indicators}...`, 'info');
        const result = await api.fetchFred(indicators);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);
        if (result.success) {
            await loadMacroData();
            switchTab('macro');
        }
    } catch (error) {
        log(`Error fetching FRED: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

// Load macro data
async function loadMacroData(): Promise<void> {
    try {
        log('Loading macro data...', 'info');
        const data = await api.getMacroData();
        const list = document.getElementById('macro-list');

        if (!list) return;

        if (data && data.length > 0) {
            const names: Record<string, string> = {
                'DFF': 'Fed Funds Rate',
                'UNRATE': 'Unemployment Rate',
                'GDP': 'Real GDP',
                'CPIAUCSL': 'CPI (Consumer Price Index)',
                'DGS10': '10-Year Treasury',
                'DGS2': '2-Year Treasury',
                'SP500': 'S&P 500',
                'VIXCLS': 'VIX Volatility',
                'PSAVERT': 'Personal Savings Rate',
                'INDPRO': 'Industrial Production',
            };

            list.innerHTML = data.map(d => {
                let formattedValue = d.value.toFixed(2);
                if (d.indicator === 'DFF' || d.indicator.includes('RATE')) {
                    formattedValue = d.value.toFixed(2) + '%';
                } else if (d.indicator === 'GDP') {
                    formattedValue = '$' + (d.value / 1000).toFixed(1) + 'T';
                }

                const displayName = names[d.indicator] || d.indicator;

                return `
                    <li class="symbol-item">
                        <div>
                            <span class="symbol-ticker">${d.indicator}</span>
                            <span style="color: var(--text-secondary); margin-left: 10px;">${displayName}</span>
                        </div>
                        <div>
                            <span class="symbol-price">${formattedValue}</span>
                            <span style="color: var(--text-secondary); font-size: 0.8rem; margin-left: 10px;">${d.date}</span>
                        </div>
                    </li>
                `;
            }).join('');
            log(`Loaded ${data.length} macro indicators`, 'success');
        } else {
            list.innerHTML = '<li class="empty-state">No macro data loaded. Click "FRED Macro" to fetch data.</li>';
        }
    } catch (error) {
        log(`Error loading macro data: ${error}`, 'error');
    }
}

// Alerts
async function loadAlerts(): Promise<void> {
    try {
        const alerts = await api.getAlerts(false);
        const list = document.getElementById('alerts-list');

        if (!list) return;

        if (alerts && alerts.length > 0) {
            list.innerHTML = alerts.map(a => `
                <li class="symbol-item">
                    <div>
                        <span class="symbol-ticker">${a.symbol}</span>
                        <span style="color: var(--text-secondary); margin-left: 10px;">
                            ${a.condition === 'above' ? '>=' : '<='} $${a.target_price.toFixed(2)}
                        </span>
                    </div>
                    <div>
                        <span style="color: ${a.triggered ? 'var(--success)' : 'var(--text-secondary)'};">
                            ${a.triggered ? 'TRIGGERED' : 'Active'}
                        </span>
                        <button class="btn-secondary delete-alert-btn" data-id="${a.id}" style="padding: 5px 10px; font-size: 0.8rem; margin-left: 10px;">
                            Delete
                        </button>
                    </div>
                </li>
            `).join('');
            log(`Loaded ${alerts.length} alerts`, 'success');
        } else {
            list.innerHTML = '<li class="empty-state">No alerts set. Add one to get started.</li>';
        }
    } catch (error) {
        log(`Error loading alerts: ${error}`, 'error');
    }
}

async function addAlert(): Promise<void> {
    const symbol = (document.getElementById('alert-symbol') as HTMLInputElement).value.trim();
    const price = parseFloat((document.getElementById('alert-price') as HTMLInputElement).value);
    const condition = (document.getElementById('alert-condition') as HTMLSelectElement).value;

    if (!symbol) {
        alert('Please enter a symbol');
        return;
    }

    if (isNaN(price) || price <= 0) {
        alert('Please enter a valid price');
        return;
    }

    try {
        log(`Adding alert: ${symbol} ${condition} $${price.toFixed(2)}...`, 'info');
        const result = await api.addAlert(symbol, price, condition);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);

        // Clear form and reload
        (document.getElementById('alert-symbol') as HTMLInputElement).value = '';
        (document.getElementById('alert-price') as HTMLInputElement).value = '';
        await loadAlerts();
    } catch (error) {
        log(`Error adding alert: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

// Portfolio
async function loadPortfolio(): Promise<void> {
    try {
        const portfolio = await api.getPortfolio();
        const list = document.getElementById('portfolio-list');

        if (!list) return;

        // Update summary
        document.getElementById('portfolio-total-value')!.textContent = `$${portfolio.total_value.toFixed(2)}`;

        const plEl = document.getElementById('portfolio-total-pl')!;
        const plColor = portfolio.total_profit_loss >= 0 ? 'var(--success)' : 'var(--error)';
        const plSign = portfolio.total_profit_loss >= 0 ? '+' : '';
        plEl.style.color = plColor;
        plEl.textContent = `${plSign}$${portfolio.total_profit_loss.toFixed(2)} (${plSign}${portfolio.total_profit_loss_percent.toFixed(2)}%)`;

        if (portfolio.positions && portfolio.positions.length > 0) {
            list.innerHTML = portfolio.positions.map(p => {
                const plColor = p.profit_loss >= 0 ? 'price-up' : 'price-down';
                const plSign = p.profit_loss >= 0 ? '+' : '';
                const arrow = p.profit_loss > 0 ? '▲' : p.profit_loss < 0 ? '▼' : '';
                const typeLabel = p.position_type === 'buy' ? 'LONG' : 'SHORT';
                const typeBadge = p.position_type === 'buy' ? 'badge-long' : 'badge-short';

                return `
                    <li class="symbol-item" style="flex-direction: column; align-items: stretch;">
                        <div style="display: flex; justify-content: space-between; align-items: center;">
                            <div>
                                <span class="symbol-ticker">${p.symbol}</span>
                                <span class="badge ${typeBadge}">${typeLabel}</span>
                                <span style="color: var(--text-secondary); margin-left: 10px; font-size: 0.85rem;">
                                    ${p.quantity} shares @ $${p.price.toFixed(2)}
                                </span>
                            </div>
                            <div>
                                <span>$${p.current_value.toFixed(2)}</span>
                                <span class="${plColor}" style="margin-left: 10px;">
                                    ${arrow} ${plSign}$${p.profit_loss.toFixed(2)} (${plSign}${p.profit_loss_percent.toFixed(2)}%)
                                </span>
                            </div>
                        </div>
                        <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 8px; font-size: 0.8rem; color: var(--text-secondary);">
                            <span>Bought: ${p.date} | Current: $${p.current_price.toFixed(2)}</span>
                            <button class="btn-secondary delete-position-btn" data-id="${p.id}" style="padding: 4px 8px; font-size: 0.75rem;">
                                Remove
                            </button>
                        </div>
                    </li>
                `;
            }).join('');
            log(`Loaded ${portfolio.positions.length} positions`, 'success');
        } else {
            list.innerHTML = '<li class="empty-state">No positions. Add your first trade to start tracking.</li>';
        }
    } catch (error) {
        log(`Error loading portfolio: ${error}`, 'error');
    }
}

async function addPosition(): Promise<void> {
    const symbol = (document.getElementById('position-symbol') as HTMLInputElement).value.trim();
    const quantity = parseFloat((document.getElementById('position-quantity') as HTMLInputElement).value);
    const price = parseFloat((document.getElementById('position-price') as HTMLInputElement).value);
    const positionType = (document.getElementById('position-type') as HTMLSelectElement).value;
    const date = (document.getElementById('position-date') as HTMLInputElement).value;
    const notes = (document.getElementById('position-notes') as HTMLInputElement).value.trim() || null;

    if (!symbol || isNaN(quantity) || quantity <= 0 || isNaN(price) || price <= 0 || !date) {
        alert('Please fill in all required fields');
        return;
    }

    try {
        log(`Adding ${positionType} position: ${quantity} x ${symbol} @ $${price.toFixed(2)}...`, 'info');
        const result = await api.addPosition(symbol, quantity, price, positionType, date, notes);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);

        // Clear form and reload
        (document.getElementById('position-symbol') as HTMLInputElement).value = '';
        (document.getElementById('position-quantity') as HTMLInputElement).value = '';
        (document.getElementById('position-price') as HTMLInputElement).value = '';
        (document.getElementById('position-notes') as HTMLInputElement).value = '';
        await loadPortfolio();
    } catch (error) {
        log(`Error adding position: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

// Toggle favorite button helper
async function toggleFavoriteButton(buttonId: string, symbol: string): Promise<void> {
    const btn = document.getElementById(buttonId);
    if (!btn) return;

    try {
        const newState = await api.toggleFavorite(symbol);
        btn.textContent = newState ? 'FAV ★' : 'FAV';
        btn.classList.toggle('favorited', newState);
        log(`${symbol} ${newState ? 'added to' : 'removed from'} auto-refresh`, 'info');
        // Refresh symbol list to update moon icons
        await refreshSymbolList();
    } catch (error) {
        log(`Error toggling favorite: ${error}`, 'error');
    }
}

// Update favorite button state when symbol input changes
async function updateFavoriteButtonState(inputId: string, buttonId: string): Promise<void> {
    const input = document.getElementById(inputId) as HTMLInputElement;
    const btn = document.getElementById(buttonId);
    if (!input || !btn) return;

    const symbol = input.value.trim().toUpperCase();
    if (!symbol) {
        btn.textContent = 'FAV';
        btn.classList.remove('favorited');
        return;
    }

    try {
        const favorites = await api.getFavoritedSymbols();
        const isFavorited = favorites.includes(symbol);
        btn.textContent = isFavorited ? 'FAV ★' : 'FAV';
        btn.classList.toggle('favorited', isFavorited);
    } catch {
        // Ignore errors
    }
}

// Search
async function searchCompany(query: string): Promise<void> {
    const resultsDiv = document.getElementById('search-results');
    if (!resultsDiv || query.length < 2) {
        if (resultsDiv) resultsDiv.innerHTML = '';
        return;
    }

    try {
        const symbols = await api.searchSymbol(query);
        if (symbols && symbols.length > 0) {
            resultsDiv.innerHTML = symbols.map(s =>
                `<button class="btn-secondary search-result" data-symbol="${s}">${s}</button>`
            ).join('');
        } else {
            resultsDiv.innerHTML = '<span style="color: var(--text-secondary);">No matches</span>';
        }
    } catch {
        resultsDiv.innerHTML = '';
    }
}

// Symbol Groups / Watchlists
async function loadGroups(): Promise<void> {
    try {
        log('Loading symbol groups...', 'info');
        const groups = await api.getAllWatchlists();
        const list = document.getElementById('groups-list');

        if (!list) return;

        if (groups && groups.length > 0) {
            list.innerHTML = groups.map(g => `
                <li class="symbol-item group-item" data-group="${g.name}">
                    <div>
                        <span class="symbol-ticker">${g.name}</span>
                        <span style="color: var(--text-secondary); margin-left: 10px; font-size: 0.85rem;">
                            ${g.symbol_count} symbols
                        </span>
                    </div>
                    <div style="color: var(--text-secondary); font-size: 0.8rem;">
                        ${g.description || ''}
                    </div>
                </li>
            `).join('');
            log(`Loaded ${groups.length} symbol groups`, 'success');
        } else {
            list.innerHTML = '<li class="empty-state">No groups created. Create your first symbol group.</li>';
        }
    } catch (error) {
        log(`Error loading groups: ${error}`, 'error');
    }
}

async function createGroup(): Promise<void> {
    const name = (document.getElementById('group-name') as HTMLInputElement).value.trim();
    const symbolsInput = (document.getElementById('group-symbols') as HTMLInputElement).value.trim();
    const description = (document.getElementById('group-description') as HTMLInputElement).value.trim() || null;

    if (!name) {
        alert('Please enter a group name');
        return;
    }

    const symbols = symbolsInput
        .split(',')
        .map(s => s.trim().toUpperCase())
        .filter(s => s.length > 0);

    if (symbols.length === 0) {
        alert('Please enter at least one symbol');
        return;
    }

    try {
        log(`Creating group "${name}" with ${symbols.length} symbols...`, 'info');
        const result = await api.createWatchlist(name, symbols, description);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);

        if (result.success) {
            // Clear form
            (document.getElementById('group-name') as HTMLInputElement).value = '';
            (document.getElementById('group-symbols') as HTMLInputElement).value = '';
            (document.getElementById('group-description') as HTMLInputElement).value = '';
            await loadGroups();
        }
    } catch (error) {
        log(`Error creating group: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

async function loadGroupDetail(groupName: string): Promise<void> {
    try {
        const detail = await api.getWatchlistDetail(groupName);
        const detailDiv = document.getElementById('group-detail');
        const symbolsList = document.getElementById('group-symbols-list');

        if (!detail || !detailDiv || !symbolsList) {
            if (detailDiv) detailDiv.style.display = 'none';
            return;
        }

        selectedGroupName = groupName;
        detailDiv.style.display = 'block';
        document.getElementById('group-detail-name')!.textContent = detail.name;
        document.getElementById('group-detail-desc')!.textContent = detail.description || '';

        if (detail.symbols.length > 0) {
            symbolsList.innerHTML = detail.symbols.map(s => `
                <li class="symbol-item group-symbol-item" data-symbol="${s}">
                    <span class="symbol-ticker">${s}</span>
                    <button class="btn-secondary remove-symbol-btn" data-symbol="${s}" style="padding: 4px 8px; font-size: 0.75rem; background: var(--error);">
                        Remove
                    </button>
                </li>
            `).join('');
        } else {
            symbolsList.innerHTML = '<li class="empty-state">No symbols in this group.</li>';
        }

        // Highlight selected group
        document.querySelectorAll('.group-item').forEach(item => {
            item.classList.toggle('selected', item.getAttribute('data-group') === groupName);
        });

        log(`Loaded group "${groupName}" with ${detail.symbols.length} symbols`, 'success');
    } catch (error) {
        log(`Error loading group detail: ${error}`, 'error');
    }
}

async function addSymbolToGroup(): Promise<void> {
    if (!selectedGroupName) {
        alert('Please select a group first');
        return;
    }

    const symbol = (document.getElementById('add-symbol-input') as HTMLInputElement).value.trim().toUpperCase();
    if (!symbol) {
        alert('Please enter a symbol');
        return;
    }

    try {
        log(`Adding ${symbol} to "${selectedGroupName}"...`, 'info');
        const result = await api.addSymbolToWatchlist(selectedGroupName, symbol);
        log(result.message, result.success ? 'success' : 'error');

        if (result.success) {
            (document.getElementById('add-symbol-input') as HTMLInputElement).value = '';
            await loadGroupDetail(selectedGroupName);
            await loadGroups(); // Update count
        } else {
            alert(result.message);
        }
    } catch (error) {
        log(`Error adding symbol: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

async function removeSymbolFromGroup(symbol: string): Promise<void> {
    if (!selectedGroupName) return;

    try {
        log(`Removing ${symbol} from "${selectedGroupName}"...`, 'info');
        const result = await api.removeSymbolFromWatchlist(selectedGroupName, symbol);
        log(result.message, result.success ? 'success' : 'error');

        if (result.success) {
            await loadGroupDetail(selectedGroupName);
            await loadGroups(); // Update count
        }
    } catch (error) {
        log(`Error removing symbol: ${error}`, 'error');
    }
}

async function deleteGroup(): Promise<void> {
    if (!selectedGroupName) return;

    if (!confirm(`Are you sure you want to delete the group "${selectedGroupName}"?`)) {
        return;
    }

    try {
        log(`Deleting group "${selectedGroupName}"...`, 'info');
        const result = await api.deleteWatchlist(selectedGroupName);
        log(result.message, result.success ? 'success' : 'error');

        if (result.success) {
            selectedGroupName = null;
            document.getElementById('group-detail')!.style.display = 'none';
            await loadGroups();
        } else {
            alert(result.message);
        }
    } catch (error) {
        log(`Error deleting group: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

async function fetchGroupPrices(): Promise<void> {
    if (!selectedGroupName) return;

    try {
        const detail = await api.getWatchlistDetail(selectedGroupName);
        if (!detail || detail.symbols.length === 0) {
            alert('No symbols in this group');
            return;
        }

        const period = (document.getElementById('period') as HTMLSelectElement).value;
        log(`Fetching prices for group "${selectedGroupName}" (${detail.symbols.length} symbols)...`, 'info');

        // Fetch in smaller batches for stability
        const batchSize = 5;
        for (let i = 0; i < detail.symbols.length; i += batchSize) {
            const batch = detail.symbols.slice(i, i + batchSize).join(',');
            try {
                await api.fetchPrices(batch, period);
                log(`Fetched batch ${Math.floor(i / batchSize) + 1}/${Math.ceil(detail.symbols.length / batchSize)}`, 'info');
            } catch (error) {
                log(`Error fetching batch: ${error}`, 'error');
            }
        }

        log(`Finished fetching ${detail.symbols.length} symbols for group "${selectedGroupName}"`, 'success');
        alert(`Fetched prices for ${detail.symbols.length} symbols in "${selectedGroupName}"`);
        await refreshSymbolList();
    } catch (error) {
        log(`Error fetching group prices: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

async function createPresetGroup(name: string, symbolsStr: string, description: string): Promise<void> {
    const symbols = symbolsStr.split(',').map(s => s.trim().toUpperCase());

    try {
        log(`Creating preset group "${name}"...`, 'info');
        const result = await api.createWatchlist(name, symbols, description);
        log(result.message, result.success ? 'success' : 'error');

        if (result.success) {
            await loadGroups();
            alert(`Created group "${name}" with ${symbols.length} symbols`);
        } else {
            alert(result.message);
        }
    } catch (error) {
        log(`Error creating preset group: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

// Fetch S&P 100
async function fetchSP100(): Promise<void> {
    const period = (document.getElementById('period') as HTMLSelectElement).value;
    log(`Fetching S&P 100 (${SP100_SYMBOLS.length} symbols)...`, 'info');

    // Fetch in smaller batches for stability
    const batchSize = 5;
    for (let i = 0; i < SP100_SYMBOLS.length; i += batchSize) {
        const batch = SP100_SYMBOLS.slice(i, i + batchSize).join(',');
        try {
            await api.fetchPrices(batch, period);
            log(`Fetched batch ${Math.floor(i / batchSize) + 1}/${Math.ceil(SP100_SYMBOLS.length / batchSize)}`, 'info');
        } catch (error) {
            log(`Error fetching batch: ${error}`, 'error');
        }
    }

    log(`S&P 100 fetch complete`, 'success');
    alert('S&P 100 symbols fetched!');
    await refreshSymbolList();
}

// Fetch ASX 100
async function fetchASX100(): Promise<void> {
    const period = (document.getElementById('period') as HTMLSelectElement).value;
    log(`Fetching ASX 100 (${ASX100_SYMBOLS.length} symbols)...`, 'info');

    // Fetch in smaller batches for stability
    const batchSize = 5;
    for (let i = 0; i < ASX100_SYMBOLS.length; i += batchSize) {
        const batch = ASX100_SYMBOLS.slice(i, i + batchSize).join(',');
        try {
            await api.fetchPrices(batch, period);
            log(`Fetched batch ${Math.floor(i / batchSize) + 1}/${Math.ceil(ASX100_SYMBOLS.length / batchSize)}`, 'info');
        } catch (error) {
            log(`Error fetching batch: ${error}`, 'error');
        }
    }

    log(`ASX 100 fetch complete`, 'success');
    alert('ASX 100 symbols fetched!');
    await refreshSymbolList();
}

// Auto-refresh
function updateLastRefreshTime(): void {
    const now = new Date();
    document.getElementById('last-refresh')!.textContent = now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function saveAutoRefreshState(enabled: boolean, interval: number): void {
    localStorage.setItem(STORAGE_KEYS.AUTO_REFRESH_ENABLED, JSON.stringify(enabled));
    localStorage.setItem(STORAGE_KEYS.AUTO_REFRESH_INTERVAL, JSON.stringify(interval));
}

function loadAutoRefreshState(): { enabled: boolean; interval: number } {
    const enabled = localStorage.getItem(STORAGE_KEYS.AUTO_REFRESH_ENABLED);
    const interval = localStorage.getItem(STORAGE_KEYS.AUTO_REFRESH_INTERVAL);

    return {
        enabled: enabled ? JSON.parse(enabled) : false,
        interval: interval ? JSON.parse(interval) : 300000, // default 5 min
    };
}

function toggleAutoRefresh(): void {
    const toggle = document.getElementById('auto-refresh-toggle') as HTMLInputElement;
    const intervalSelect = document.getElementById('refresh-interval') as HTMLSelectElement;
    const statusEl = document.getElementById('refresh-status')!;

    if (toggle.checked) {
        startAutoRefresh();
        statusEl.textContent = 'Auto-refresh on';
        statusEl.classList.add('active');
    } else {
        stopAutoRefresh();
        statusEl.textContent = 'Auto-refresh off';
        statusEl.classList.remove('active');
    }

    // Save state to localStorage
    saveAutoRefreshState(toggle.checked, parseInt(intervalSelect.value));
}

function startAutoRefresh(): void {
    const interval = parseInt((document.getElementById('refresh-interval') as HTMLSelectElement).value);
    log(`Auto-refresh started (every ${interval / 60000} min)`, 'info');

    autoRefreshPrices();
    autoRefreshTimer = window.setInterval(autoRefreshPrices, interval);
}

function stopAutoRefresh(): void {
    if (autoRefreshTimer) {
        clearInterval(autoRefreshTimer);
        autoRefreshTimer = null;
        log('Auto-refresh stopped', 'info');
    }
}

function restoreAutoRefreshState(): void {
    const state = loadAutoRefreshState();
    const toggle = document.getElementById('auto-refresh-toggle') as HTMLInputElement;
    const intervalSelect = document.getElementById('refresh-interval') as HTMLSelectElement;
    const statusEl = document.getElementById('refresh-status')!;

    // Restore interval selection
    intervalSelect.value = state.interval.toString();

    // Restore toggle state
    if (state.enabled) {
        toggle.checked = true;
        startAutoRefresh();
        statusEl.textContent = 'Auto-refresh on';
        statusEl.classList.add('active');
        log('Auto-refresh restored from saved settings', 'info');
    }
}

async function autoRefreshPrices(): Promise<void> {
    try {
        // Only refresh favorited symbols (marked with moon)
        const favoritedSymbols = await api.getFavoritedSymbols();
        if (!favoritedSymbols || favoritedSymbols.length === 0) {
            log('Auto-refresh: No favorited symbols (click ☆ to add)', 'info');
            updateLastRefreshTime();
            return;
        }

        const symbolList = favoritedSymbols.join(',');
        log(`Auto-refreshing ${favoritedSymbols.length} favorited symbols...`, 'info');

        const result = await api.fetchPrices(symbolList, '1d');
        log(`Auto-refresh: ${result.message}`, result.success ? 'success' : 'error');

        await refreshSymbolList();
        updateLastRefreshTime();

        // Check alerts
        const triggered = await api.checkAlerts();
        if (triggered && triggered.length > 0) {
            const messages = triggered.map(a =>
                `${a.symbol} ${a.condition === 'above' ? 'reached' : 'dropped to'} $${a.target_price.toFixed(2)}`
            ).join('\n');
            alert(`Alerts triggered!\n\n${messages}`);
            log(`${triggered.length} alerts triggered!`, 'success');
            await loadAlerts();
        }
    } catch (error) {
        log(`Auto-refresh error: ${error}`, 'error');
    }
}

// Load chart
async function loadChart(): Promise<void> {
    const symbol = (document.getElementById('chart-symbol') as HTMLInputElement).value.trim();
    if (!symbol) {
        alert('Please enter a symbol');
        return;
    }

    initializeChart();

    try {
        log(`Loading chart for ${symbol}...`, 'info');
        await tvChart?.loadSymbol(symbol);
        log(`Chart loaded for ${symbol}`, 'success');

        // Add selected indicators
        await updateChartIndicators();
    } catch (error) {
        log(`Error loading chart: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

async function updateChartIndicators(): Promise<void> {
    if (!tvChart) return;

    tvChart.clearIndicators();

    if ((document.getElementById('show-sma20') as HTMLInputElement).checked) {
        await tvChart.addIndicator('SMA_20', '#38bdf8');
    }
    if ((document.getElementById('show-sma50') as HTMLInputElement).checked) {
        await tvChart.addIndicator('SMA_50', '#f59e0b');
    }
    if ((document.getElementById('show-ema12') as HTMLInputElement).checked) {
        await tvChart.addIndicator('EMA_12', '#8b5cf6');
    }
    if ((document.getElementById('show-bb') as HTMLInputElement).checked) {
        await tvChart.addIndicator('BB_UPPER_20', '#ec4899');
        await tvChart.addIndicator('BB_MIDDLE_20', '#ec489980');
        await tvChart.addIndicator('BB_LOWER_20', '#ec4899');
    }
}

// Calculate indicators
async function calculateIndicators(): Promise<void> {
    const symbol = (document.getElementById('indicator-symbol') as HTMLInputElement).value.trim();
    if (!symbol) {
        alert('Please enter a symbol');
        return;
    }

    try {
        log(`Calculating indicators for ${symbol}...`, 'info');
        const result = await api.calculateIndicators(symbol);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);

        if (result.success) {
            await loadIndicatorList(symbol);
        }
    } catch (error) {
        log(`Error calculating: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

async function loadIndicatorList(symbol: string): Promise<void> {
    try {
        const indicators = await api.getIndicators(symbol);
        const list = document.getElementById('indicator-list');

        if (!list) return;

        if (indicators && indicators.length > 0) {
            list.innerHTML = indicators.map(ind => `
                <li class="symbol-item" data-indicator="${ind.name}">
                    <span class="symbol-ticker">${ind.name}</span>
                    <span class="symbol-price">${ind.value.toFixed(2)}</span>
                </li>
            `).join('');
            log(`Loaded ${indicators.length} indicators for ${symbol}`, 'success');
        } else {
            list.innerHTML = '<li class="empty-state">No indicators calculated. Click Calculate first.</li>';
        }
    } catch (error) {
        log(`Error loading indicators: ${error}`, 'error');
    }
}

async function showIndicatorChart(): Promise<void> {
    const symbol = (document.getElementById('indicator-symbol') as HTMLInputElement).value.trim();
    const indicatorName = (document.getElementById('indicator-select') as HTMLSelectElement).value;

    if (!symbol || !indicatorName) {
        alert('Please enter a symbol and select an indicator');
        return;
    }

    initializeIndicatorChart();

    try {
        log(`Loading ${indicatorName} chart for ${symbol}...`, 'info');
        await indicatorChart?.loadIndicator(symbol, indicatorName);
        log(`Indicator chart loaded`, 'success');
    } catch (error) {
        log(`Error loading indicator chart: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

// ============================================================================
// AI Search / Vector Database Functions
// ============================================================================

async function loadVectorStats(): Promise<void> {
    try {
        const stats = await api.getVectorStats();
        document.getElementById('vector-events-count')!.textContent = stats.events_count.toString();
        document.getElementById('vector-patterns-count')!.textContent = stats.patterns_count.toString();
        log(`Vector DB: ${stats.events_count} events, ${stats.patterns_count} patterns`, 'info');
    } catch (error) {
        log(`Error loading vector stats: ${error}`, 'error');
    }
}

async function performAISearch(): Promise<void> {
    const query = (document.getElementById('ai-search-query') as HTMLInputElement).value.trim();
    if (!query) {
        alert('Please enter a search query');
        return;
    }

    const resultsDiv = document.getElementById('ai-search-results')!;
    resultsDiv.innerHTML = '<p>Searching...</p>';

    try {
        log(`AI Search: "${query}"`, 'info');
        const results = await api.vectorSearch(query, 10);

        if (results.length === 0) {
            resultsDiv.innerHTML = '<p class="empty-state">No matching results found. Try a different query or add some events/patterns first.</p>';
            return;
        }

        // SECURITY: Escape all user-controlled content to prevent XSS
        resultsDiv.innerHTML = results.map(r => `
            <div class="ai-result-item" data-type="${escapeHtml(r.result_type)}">
                <div class="ai-result-header">
                    <span class="ai-result-type ${escapeHtml(r.result_type)}">${escapeHtml(r.result_type.replace('_', ' '))}</span>
                    <span class="ai-result-score">${sanitizeNumber(r.score * 100, 1)}% match</span>
                    ${r.symbol ? `<span class="ai-result-symbol">${escapeHtml(r.symbol)}</span>` : ''}
                </div>
                <div class="ai-result-content">${escapeHtml(r.content)}</div>
                ${r.date ? `<div class="ai-result-date">${escapeHtml(r.date)}</div>` : ''}
            </div>
        `).join('');

        log(`Found ${results.length} results for "${query}"`, 'success');
    } catch (error) {
        resultsDiv.innerHTML = `<p class="empty-state error">Error: ${escapeHtml(String(error))}</p>`;
        log(`AI Search error: ${error}`, 'error');
    }
}

async function addMarketEventUI(): Promise<void> {
    const symbol = (document.getElementById('event-symbol') as HTMLInputElement).value.trim().toUpperCase();
    const eventType = (document.getElementById('event-type') as HTMLSelectElement).value;
    const title = (document.getElementById('event-title') as HTMLInputElement).value.trim();
    const content = (document.getElementById('event-content') as HTMLTextAreaElement).value.trim();
    const date = (document.getElementById('event-date') as HTMLInputElement).value;
    const sentimentStr = (document.getElementById('event-sentiment') as HTMLInputElement).value;
    const sentiment = sentimentStr ? parseFloat(sentimentStr) : null;

    if (!symbol || !title || !content || !date) {
        alert('Please fill in symbol, title, content, and date');
        return;
    }

    try {
        log(`Adding market event for ${symbol}...`, 'info');
        const result = await api.addMarketEvent(symbol, eventType, title, content, date, sentiment);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);

        if (result.success) {
            // Clear form
            (document.getElementById('event-symbol') as HTMLInputElement).value = '';
            (document.getElementById('event-title') as HTMLInputElement).value = '';
            (document.getElementById('event-content') as HTMLTextAreaElement).value = '';
            (document.getElementById('event-date') as HTMLInputElement).value = '';
            (document.getElementById('event-sentiment') as HTMLInputElement).value = '';
            await loadVectorStats();
        }
    } catch (error) {
        log(`Error adding market event: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

async function addPricePatternUI(): Promise<void> {
    const symbol = (document.getElementById('pattern-symbol') as HTMLInputElement).value.trim().toUpperCase();
    const patternType = (document.getElementById('pattern-type') as HTMLSelectElement).value;
    const startDate = (document.getElementById('pattern-start-date') as HTMLInputElement).value;
    const endDate = (document.getElementById('pattern-end-date') as HTMLInputElement).value;
    const priceChange = parseFloat((document.getElementById('pattern-price-change') as HTMLInputElement).value) || 0;
    const volumeChange = parseFloat((document.getElementById('pattern-volume-change') as HTMLInputElement).value) || 0;
    const description = (document.getElementById('pattern-description') as HTMLTextAreaElement).value.trim();

    if (!symbol || !startDate || !endDate || !description) {
        alert('Please fill in symbol, dates, and description');
        return;
    }

    try {
        log(`Adding price pattern for ${symbol}...`, 'info');
        const result = await api.addPricePattern(symbol, patternType, startDate, endDate, priceChange, volumeChange, description);
        log(result.message, result.success ? 'success' : 'error');
        alert(result.message);

        if (result.success) {
            // Clear form
            (document.getElementById('pattern-symbol') as HTMLInputElement).value = '';
            (document.getElementById('pattern-start-date') as HTMLInputElement).value = '';
            (document.getElementById('pattern-end-date') as HTMLInputElement).value = '';
            (document.getElementById('pattern-price-change') as HTMLInputElement).value = '';
            (document.getElementById('pattern-volume-change') as HTMLInputElement).value = '';
            (document.getElementById('pattern-description') as HTMLTextAreaElement).value = '';
            await loadVectorStats();
        }
    } catch (error) {
        log(`Error adding price pattern: ${error}`, 'error');
        alert(`Error: ${error}`);
    }
}

// ============================================================================
// Claude AI Chat Functions
// ============================================================================

const CLAUDE_API_KEY_STORAGE = 'fp_claude_api_key';

function loadClaudeApiKey(): void {
    const savedKey = localStorage.getItem(CLAUDE_API_KEY_STORAGE);
    const keyInput = document.getElementById('claude-api-key') as HTMLInputElement;
    const statusSpan = document.getElementById('api-key-status')!;

    if (savedKey) {
        keyInput.value = savedKey;
        statusSpan.textContent = 'Key saved';
        statusSpan.style.color = 'var(--success)';
    }
}

function saveClaudeApiKey(): void {
    const keyInput = document.getElementById('claude-api-key') as HTMLInputElement;
    const statusSpan = document.getElementById('api-key-status')!;
    const key = keyInput.value.trim();

    if (key) {
        // SECURITY: Validate API key format before storing
        if (!key.startsWith('sk-ant-') || key.length < 50) {
            statusSpan.textContent = 'Invalid key format';
            statusSpan.style.color = 'var(--error)';
            log('Invalid API key format - must start with sk-ant-', 'error');
            return;
        }
        localStorage.setItem(CLAUDE_API_KEY_STORAGE, key);
        statusSpan.textContent = 'Key saved';
        statusSpan.style.color = 'var(--success)';
        log('Claude API key saved', 'success');
    } else {
        localStorage.removeItem(CLAUDE_API_KEY_STORAGE);
        statusSpan.textContent = 'Key cleared';
        statusSpan.style.color = 'var(--text-secondary)';
    }
}

// SECURITY: Rate limiting for API calls
let lastClaudeCallTime = 0;
const CLAUDE_RATE_LIMIT_MS = 2000; // Minimum 2 seconds between calls

async function sendClaudeChat(): Promise<void> {
    const query = (document.getElementById('claude-chat-query') as HTMLInputElement).value.trim();
    const apiKey = (document.getElementById('claude-api-key') as HTMLInputElement).value.trim();

    if (!apiKey) {
        alert('Please enter your Claude API key');
        return;
    }

    if (!query) {
        alert('Please enter a question');
        return;
    }

    // SECURITY: Rate limiting to prevent API abuse
    const now = Date.now();
    if (now - lastClaudeCallTime < CLAUDE_RATE_LIMIT_MS) {
        alert(`Please wait ${Math.ceil((CLAUDE_RATE_LIMIT_MS - (now - lastClaudeCallTime)) / 1000)} seconds before making another request`);
        return;
    }
    lastClaudeCallTime = now;

    // SECURITY: Validate query length to prevent abuse
    if (query.length > 10000) {
        alert('Query too long. Maximum 10,000 characters.');
        return;
    }

    const responseDiv = document.getElementById('claude-chat-response')!;
    const metaDiv = document.getElementById('claude-chat-meta')!;
    const chatBtn = document.getElementById('claude-chat-btn') as HTMLButtonElement;

    responseDiv.innerHTML = '<p style="color: var(--accent);">Asking Claude...</p>';
    metaDiv.style.display = 'none';
    chatBtn.disabled = true;

    try {
        log(`Claude query: "${query.substring(0, 50)}..."`, 'info');
        const result = await api.claudeChat(query, apiKey);

        // SECURITY: First escape HTML, THEN apply markdown formatting
        const sanitizedResponse = escapeHtml(result.response);
        const formattedResponse = sanitizedResponse
            .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
            .replace(/\n\n/g, '</p><p>')
            .replace(/\n/g, '<br>');

        responseDiv.innerHTML = `<div class="ai-result-item" style="border-left-color: var(--accent);">
            <div class="ai-result-content"><p>${formattedResponse}</p></div>
        </div>`;

        // Show metadata
        document.getElementById('claude-model')!.textContent = result.model;
        document.getElementById('claude-tokens')!.textContent = `${result.input_tokens} in / ${result.output_tokens} out`;
        metaDiv.style.display = 'block';

        log(`Claude response received (${result.input_tokens + result.output_tokens} tokens)`, 'success');

        // Clear query input
        (document.getElementById('claude-chat-query') as HTMLInputElement).value = '';

        // Refresh vector stats since the conversation was saved
        await loadVectorStats();
    } catch (error) {
        responseDiv.innerHTML = `<p class="empty-state" style="color: var(--error);">Error: ${escapeHtml(String(error))}</p>`;
        log(`Claude error: ${error}`, 'error');
    } finally {
        chatBtn.disabled = false;
    }
}

// ============================================================================
// Finnhub News Functions
// ============================================================================

const FINNHUB_API_KEY_STORAGE = 'fp_finnhub_api_key';

// Store fetched news for "Save All" functionality
let fetchedNewsItems: api.SimpleNewsItem[] = [];

// Store candle data for news date display (date -> {close, open, change%})
interface DayCandle {
    close: number;
    open: number;
    high: number;
    low: number;
    dailyChangePercent: number;  // (close - open) / open * 100
}
let fetchedCandlesByDate: Map<string, DayCandle> = new Map();

function loadFinnhubApiKey(): void {
    const savedKey = localStorage.getItem(FINNHUB_API_KEY_STORAGE);
    const keyInput = document.getElementById('finnhub-api-key') as HTMLInputElement;

    if (savedKey && keyInput) {
        keyInput.value = savedKey;
    }
}

function saveFinnhubApiKey(): void {
    const keyInput = document.getElementById('finnhub-api-key') as HTMLInputElement;
    const key = keyInput.value.trim();

    if (key) {
        localStorage.setItem(FINNHUB_API_KEY_STORAGE, key);
        log('Finnhub API key saved', 'success');
        alert('Finnhub API key saved!');
    } else {
        localStorage.removeItem(FINNHUB_API_KEY_STORAGE);
        log('Finnhub API key cleared', 'info');
    }
}

async function fetchNewsAndPopulateForm(): Promise<void> {
    const symbolInput = (document.getElementById('ai-search-query') as HTMLInputElement).value.trim().toUpperCase();
    const apiKey = localStorage.getItem(FINNHUB_API_KEY_STORAGE) || '';

    if (!apiKey) {
        alert('Please enter and save your Finnhub API key first.\nGet a free key at https://finnhub.io');
        return;
    }

    if (!symbolInput) {
        alert('Please enter a stock symbol (e.g., NVDA, AAPL)');
        return;
    }

    // Extract and sanitize the symbol (in case user entered a full name or extra text)
    const symbol = sanitizeSymbol(symbolInput.split(/\s+/)[0]);

    if (!symbol) {
        alert('Invalid symbol format');
        return;
    }

    const resultsDiv = document.getElementById('ai-search-results')!;
    resultsDiv.innerHTML = `<p>Fetching news for ${escapeHtml(symbol)}...</p>`;

    try {
        log(`Fetching news for ${symbol}...`, 'info');
        const response = await api.fetchNews(symbol, apiKey, 5);

        if (response.count === 0) {
            resultsDiv.innerHTML = `<p class="empty-state">No recent news found for ${escapeHtml(symbol)}.</p>`;
            fetchedNewsItems = [];
            fetchedCandlesByDate.clear();
            return;
        }

        // Store news items for Save All functionality
        fetchedNewsItems = response.news;

        // Fetch price data for news dates - try local Yahoo data first, then Finnhub
        fetchedCandlesByDate.clear();
        const dates = response.news.map(n => n.date).sort();

        try {
            resultsDiv.innerHTML = `<p>Fetching price data for ${escapeHtml(symbol)}...</p>`;

            // Try local price history first (Yahoo data already fetched)
            const localPrices = await api.getPriceHistory(symbol);
            if (localPrices && localPrices.length > 0) {
                log(`Using local Yahoo price data (${localPrices.length} days)`, 'info');
                for (const p of localPrices) {
                    const dailyChangePercent = p.open > 0 ? ((p.close - p.open) / p.open) * 100 : 0;
                    fetchedCandlesByDate.set(p.date, {
                        close: p.close,
                        open: p.open,
                        high: p.high,
                        low: p.low,
                        dailyChangePercent
                    });
                }
                log(`Loaded ${fetchedCandlesByDate.size} days of local price data`, 'info');
            } else {
                // Fall back to Finnhub candles if no local data
                log(`No local data, trying Finnhub candles...`, 'info');
                const minDate = dates[0];
                const maxDate = dates[dates.length - 1];
                const candles = await api.fetchCandles(symbol, minDate, maxDate, apiKey);
                for (let i = 0; i < candles.dates.length; i++) {
                    const open = candles.open[i];
                    const close = candles.close[i];
                    const dailyChangePercent = open > 0 ? ((close - open) / open) * 100 : 0;
                    fetchedCandlesByDate.set(candles.dates[i], {
                        close,
                        open,
                        high: candles.high[i],
                        low: candles.low[i],
                        dailyChangePercent
                    });
                }
                log(`Loaded ${candles.dates.length} days from Finnhub`, 'info');
            }
        } catch (priceErr) {
            log(`Could not fetch price data: ${priceErr}`, 'info');
            // Continue without price data - it's optional
        }

        // Common symbol to company name mappings
        const symbolNames: Record<string, string[]> = {
            'NVDA': ['nvidia', 'nvda'],
            'AAPL': ['apple', 'aapl', 'iphone', 'ipad', 'mac'],
            'GOOGL': ['google', 'googl', 'alphabet', 'youtube', 'android'],
            'GOOG': ['google', 'goog', 'alphabet', 'youtube', 'android'],
            'MSFT': ['microsoft', 'msft', 'windows', 'azure', 'xbox'],
            'AMZN': ['amazon', 'amzn', 'aws', 'prime'],
            'META': ['meta', 'facebook', 'instagram', 'whatsapp'],
            'TSLA': ['tesla', 'tsla', 'elon musk'],
            'AMD': ['amd', 'advanced micro'],
            'INTC': ['intel', 'intc'],
        };

        // Helper to check if news mentions the symbol or company name
        const mentionsSymbol = (item: api.SimpleNewsItem, sym: string): boolean => {
            const text = (item.headline + ' ' + item.summary).toLowerCase();
            const symLower = sym.toLowerCase();

            // Check for symbol itself
            if (text.includes(symLower)) return true;

            // Check for known company names
            const names = symbolNames[sym.toUpperCase()];
            if (names) {
                for (const name of names) {
                    if (text.includes(name)) return true;
                }
            }

            return false;
        };

        // Render news items (called on initial load and when filter changes)
        const renderNewsItems = (filterSymbolOnly: boolean) => {
            const newsToShow = filterSymbolOnly
                ? response.news.filter(item => mentionsSymbol(item, symbol))
                : response.news;

            const newsListHtml = newsToShow.map((item, index) => {
                // Get price data for this news date
                const candle = fetchedCandlesByDate.get(item.date);
                const priceHtml = candle
                    ? (() => {
                        const change = candle.dailyChangePercent;
                        const arrow = change > 0 ? '▲' : change < 0 ? '▼' : '–';
                        const color = change > 0 ? '#22c55e' : change < 0 ? '#ef4444' : 'var(--text-secondary)';
                        const sentimentLabel = change >= 2 ? 'BULLISH' : change <= -2 ? 'BEARISH' : 'NEUTRAL';
                        const sentimentColor = change >= 2 ? '#22c55e' : change <= -2 ? '#ef4444' : '#888';
                        return `
                            <span class="news-price-badge" style="margin-left: 8px; font-size: 0.85em;">
                                <span style="color: var(--text-secondary);">$${candle.close.toFixed(2)}</span>
                                <span style="color: ${color}; font-weight: 500;">${arrow}${Math.abs(change).toFixed(1)}%</span>
                                <span class="sentiment-badge" style="background: ${sentimentColor}; color: white; padding: 1px 6px; border-radius: 3px; font-size: 0.75em; margin-left: 4px;">${sentimentLabel}</span>
                            </span>`;
                    })()
                    : '';

                // Check if this is symbol-specific or general market news
                const isSpecific = mentionsSymbol(item, symbol);
                const relevanceBadge = !isSpecific
                    ? '<span style="background: #666; color: white; padding: 1px 5px; border-radius: 3px; font-size: 0.7em; margin-left: 4px;">MARKET</span>'
                    : '';

                return `
                <div class="ai-result-item news-item" data-index="${index}" style="border-left-color: var(--accent);">
                    <div class="ai-result-header" style="cursor: pointer;">
                        <span class="ai-result-type news">news</span>
                        <span class="ai-result-date">${escapeHtml(item.date)}</span>
                        <span class="ai-result-symbol">${escapeHtml(item.symbol)}</span>
                        ${relevanceBadge}
                        ${priceHtml}
                    </div>
                    <div class="ai-result-content" style="cursor: pointer;"><strong>${escapeHtml(item.headline)}</strong></div>
                    <div class="news-summary" data-index="${index}" style="font-size: 0.9em; color: var(--text-secondary);">
                        <span class="summary-preview">${escapeHtml(item.summary.substring(0, 200))}${item.summary.length > 200 ? '...' : ''}</span>
                        <span class="summary-full" style="display: none;">${escapeHtml(item.summary)}</span>
                    </div>
                    <div style="font-size: 0.8em; color: var(--text-secondary); margin-top: 4px;">
                        Source: ${escapeHtml(item.source)} |
                        <a href="#" class="read-more-link" data-index="${index}" style="color: var(--accent);">Read more ▼</a>
                        ${item.url ? ` | <a href="#" class="open-article-link" data-index="${index}" data-url="${escapeHtml(item.url)}" data-title="${escapeHtml(item.headline)}" style="color: var(--accent); cursor: pointer;">Open article ↗</a>` : ''}
                    </div>
                </div>
                `;
            }).join('');

            return { html: newsListHtml, count: newsToShow.length };
        };

        // Initial render
        const initialRender = renderNewsItems(false);

        // Show news items in results with Save All button and filter options
        resultsDiv.innerHTML = `
            <div style="margin-bottom: 12px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap;">
                <button id="save-all-news-btn" style="background: var(--accent); color: white; padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer;">
                    Save All <span id="news-count">${response.count}</span> News Items
                </button>
                <label style="display: flex; align-items: center; gap: 4px; font-size: 0.9em; cursor: pointer;" title="Fetches price data around each news date and creates linked patterns showing price reaction">
                    <input type="checkbox" id="auto-link-pattern" checked style="cursor: pointer;">
                    <span>Auto-link patterns</span>
                </label>
                <label style="display: flex; align-items: center; gap: 4px; font-size: 0.9em; cursor: pointer;" title="Only show news that specifically mentions ${escapeHtml(symbol)}">
                    <input type="checkbox" id="filter-symbol-only" style="cursor: pointer;">
                    <span>${escapeHtml(symbol)}-specific only</span>
                </label>
            </div>
            <div id="news-items-container">${initialRender.html}</div>
        `;

        // Add filter toggle handler
        document.getElementById('filter-symbol-only')?.addEventListener('change', (e) => {
            const filterOn = (e.target as HTMLInputElement).checked;
            const rendered = renderNewsItems(filterOn);
            document.getElementById('news-items-container')!.innerHTML = rendered.html;
            document.getElementById('news-count')!.textContent = rendered.count.toString();
            // Re-attach event handlers
            attachNewsEventHandlers();
            log(`Showing ${rendered.count} news items (filter: ${filterOn ? 'symbol-specific' : 'all'})`, 'info');
        });

        // Function to attach event handlers to news items
        const attachNewsEventHandlers = () => {
            // Click handlers for news items (to populate form)
            resultsDiv.querySelectorAll('.news-item .ai-result-header, .news-item .ai-result-content').forEach((elem) => {
                elem.addEventListener('click', (e) => {
                    const newsItem = (e.currentTarget as HTMLElement).closest('.news-item');
                    const index = parseInt(newsItem?.getAttribute('data-index') || '0');
                    populateEventFormFromNews(response.news[index]);
                });
            });

            // Read More handlers
            resultsDiv.querySelectorAll('.read-more-link').forEach((link) => {
                link.addEventListener('click', (e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    const index = (e.currentTarget as HTMLElement).getAttribute('data-index');
                    const summaryDiv = resultsDiv.querySelector(`.news-summary[data-index="${index}"]`);
                    const preview = summaryDiv?.querySelector('.summary-preview') as HTMLElement;
                    const full = summaryDiv?.querySelector('.summary-full') as HTMLElement;
                    const linkElem = e.currentTarget as HTMLElement;
                    if (preview && full) {
                        if (full.style.display === 'none') {
                            preview.style.display = 'none';
                            full.style.display = 'inline';
                            linkElem.textContent = 'Show less ▲';
                        } else {
                            preview.style.display = 'inline';
                            full.style.display = 'none';
                            linkElem.textContent = 'Read more ▼';
                        }
                    }
                });
            });

            // Open Article handlers
            resultsDiv.querySelectorAll('.open-article-link').forEach((link) => {
                link.addEventListener('click', async (e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    const url = (e.currentTarget as HTMLElement).getAttribute('data-url');
                    const title = (e.currentTarget as HTMLElement).getAttribute('data-title');
                    if (url && title) {
                        try {
                            await api.openArticleWindow(url, title);
                        } catch (err) {
                            log(`Failed to open article: ${err}`, 'error');
                            window.open(url, '_blank');
                        }
                    }
                });
            });
        };

        // Initial event handler attachment
        attachNewsEventHandlers();

        // Add Save All button handler
        document.getElementById('save-all-news-btn')?.addEventListener('click', saveAllFetchedNews);

        log(`Found ${response.count} news items for ${symbol}. Click one to populate the form, or Save All.`, 'success');
    } catch (err) {
        resultsDiv.innerHTML = `<p class="error">Error fetching news: ${err}</p>`;
        log(`Failed to fetch news: ${err}`, 'error');
    }
}


function populateEventFormFromNews(news: api.SimpleNewsItem): void {
    // Populate the market event form fields
    (document.getElementById('event-symbol') as HTMLInputElement).value = news.symbol;
    (document.getElementById('event-type') as HTMLSelectElement).value = 'news';
    (document.getElementById('event-title') as HTMLInputElement).value = news.headline;
    (document.getElementById('event-content') as HTMLTextAreaElement).value = news.summary;
    (document.getElementById('event-date') as HTMLInputElement).value = news.date;
    // Leave sentiment empty for user to assess
    (document.getElementById('event-sentiment') as HTMLInputElement).value = '';

    log(`Form populated with: "${news.headline}"`, 'info');
}

async function saveAllFetchedNews(): Promise<void> {
    if (fetchedNewsItems.length === 0) {
        alert('No news items to save. Fetch news first.');
        return;
    }

    const saveBtn = document.getElementById('save-all-news-btn') as HTMLButtonElement;
    const autoLinkCheckbox = document.getElementById('auto-link-pattern') as HTMLInputElement;
    const linkPattern = autoLinkCheckbox?.checked ?? false;

    // Get Finnhub API key for pattern linking
    const apiKey = linkPattern ? localStorage.getItem(FINNHUB_API_KEY_STORAGE) : null;

    // Debug logging
    log(`Pattern linking: checkbox=${!!autoLinkCheckbox}, checked=${linkPattern}, hasKey=${!!apiKey}`, 'info');

    if (saveBtn) {
        saveBtn.disabled = true;
        saveBtn.textContent = linkPattern && apiKey ? 'Saving with patterns...' : 'Saving...';
    }

    let savedCount = 0;
    let patternsLinked = 0;
    let errorCount = 0;

    for (const news of fetchedNewsItems) {
        try {
            if (linkPattern && apiKey) {
                // Use enhanced save with pattern linking
                const result = await api.addMarketEventWithPattern(
                    news.symbol,
                    'news',
                    news.headline,
                    news.summary,
                    news.date,
                    null, // No sentiment auto-assigned
                    apiKey,
                    true, // Link pattern
                    3     // 3 days window
                );
                if (result.success) {
                    savedCount++;
                    if (result.pattern_id) {
                        patternsLinked++;
                        log(`${news.symbol}: ${result.price_change_percent?.toFixed(2)}% price change`, 'info');
                    } else {
                        // Pattern linking failed - show actual error
                        const errorDetail = result.pattern_error || result.message;
                        log(`${news.symbol}: Event saved, pattern failed: ${errorDetail}`, 'info');
                    }
                } else {
                    errorCount++;
                    log(`Failed to save: ${news.headline.substring(0, 50)}...`, 'error');
                }
            } else {
                // Use basic save without pattern linking
                const result = await api.addMarketEvent(
                    news.symbol,
                    'news',
                    news.headline,
                    news.summary,
                    news.date,
                    null
                );
                if (result.success) {
                    savedCount++;
                } else {
                    errorCount++;
                    log(`Failed to save: ${news.headline.substring(0, 50)}...`, 'error');
                }
            }
        } catch (error) {
            errorCount++;
            log(`Error saving news: ${error}`, 'error');
        }
    }

    // Update button state
    if (saveBtn) {
        const btnText = patternsLinked > 0
            ? `Saved ${savedCount} + ${patternsLinked} patterns`
            : `Saved ${savedCount}/${fetchedNewsItems.length}`;
        saveBtn.textContent = btnText;
        saveBtn.style.background = savedCount > 0 ? 'var(--success)' : 'var(--error)';
    }

    // Refresh vector stats
    await loadVectorStats();

    if (errorCount === 0) {
        const msg = patternsLinked > 0
            ? `Saved ${savedCount} news items with ${patternsLinked} price reaction patterns!`
            : `Successfully saved all ${savedCount} news items to the database!`;
        log(msg, 'success');
    } else {
        log(`Saved ${savedCount} items, ${errorCount} failed.`, 'error');
    }

    // Clear the stored items to prevent double-saving
    fetchedNewsItems = [];
}

// ============================================================================
// AI TRADER FUNCTIONS
// ============================================================================

let aiPerformanceChart: any = null;

async function loadAiTrader(): Promise<void> {
    try {
        // Load status
        const status = await api.aiTraderGetStatus();
        updateAiTraderStatus(status);

        // Load decisions
        const decisions = await api.aiTraderGetDecisions(undefined, undefined, 50);
        updateAiDecisionLog(decisions);

        // Load forecast
        try {
            const forecast = await api.aiTraderGetCompoundingForecast();
            updateAiForecast(forecast);
        } catch (e) {
            // Forecast may fail if no data yet
        }

        // Load accuracy
        try {
            const accuracy = await api.aiTraderGetPredictionAccuracy();
            updateAiAccuracy(accuracy);
        } catch (e) {
            // Accuracy may fail if no predictions yet
        }

        // Load benchmark comparison
        try {
            const benchmark = await api.aiTraderGetBenchmarkComparison();
            updateAiBenchmark(benchmark);
        } catch (e) {
            // Benchmark may fail if no data yet
        }

        log('AI Trader data loaded', 'success');
    } catch (error) {
        log(`Error loading AI Trader: ${error}`, 'error');
    }
}

function updateAiTraderStatus(status: api.AiTraderStatus): void {
    const container = document.querySelector('.ai-trader-container');
    if (container) {
        container.classList.toggle('bankrupt', status.is_bankrupt);
    }

    // Update values
    const portfolioEl = document.getElementById('ai-portfolio-value');
    const changeEl = document.getElementById('ai-portfolio-change');
    const cashEl = document.getElementById('ai-cash');
    const positionsEl = document.getElementById('ai-positions-value');
    const sessionEl = document.getElementById('ai-session-status');
    const sessionsCountEl = document.getElementById('ai-sessions-count');
    const decisionsCountEl = document.getElementById('ai-decisions-count');
    const tradesCountEl = document.getElementById('ai-trades-count');

    if (portfolioEl) portfolioEl.textContent = `$${status.portfolio_value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
    if (cashEl) cashEl.textContent = `$${status.cash.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
    if (positionsEl) positionsEl.textContent = `$${status.positions_value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;

    // Calculate change from $1M
    const pnlPercent = ((status.portfolio_value - 1000000) / 1000000) * 100;
    if (changeEl) {
        const sign = pnlPercent >= 0 ? '+' : '';
        changeEl.textContent = `${sign}${pnlPercent.toFixed(2)}%`;
        changeEl.className = 'ai-stat-change ' + (pnlPercent >= 0 ? 'positive' : 'negative');
    }

    // Session status
    if (sessionEl) {
        if (status.is_bankrupt) {
            sessionEl.textContent = 'BANKRUPT';
            sessionEl.style.color = 'var(--error)';
        } else if (status.is_running) {
            sessionEl.textContent = 'Active';
            sessionEl.style.color = 'var(--success)';
        } else {
            sessionEl.textContent = 'Inactive';
            sessionEl.style.color = 'var(--text-secondary)';
        }
    }

    // Stats
    if (sessionsCountEl) sessionsCountEl.textContent = status.sessions_completed.toString();
    if (decisionsCountEl) decisionsCountEl.textContent = status.total_decisions.toString();
    if (tradesCountEl) tradesCountEl.textContent = status.total_trades.toString();

    // Update button states
    const startBtn = document.getElementById('ai-start-session-btn') as HTMLButtonElement;
    const endBtn = document.getElementById('ai-end-session-btn') as HTMLButtonElement;
    const runBtn = document.getElementById('ai-run-cycle-btn') as HTMLButtonElement;

    if (startBtn && endBtn && runBtn) {
        startBtn.disabled = status.is_running || status.is_bankrupt;
        endBtn.disabled = !status.is_running;
        runBtn.disabled = !status.is_running;
    }
}

function updateAiDecisionLog(decisions: api.AiTradeDecision[]): void {
    const logEl = document.getElementById('ai-decision-log');
    if (!logEl) return;

    if (decisions.length === 0) {
        logEl.innerHTML = '<div class="empty-state">No decisions yet. Start a session and run a cycle.</div>';
        return;
    }

    logEl.innerHTML = decisions.map(d => {
        const actionClass = d.action.toLowerCase();
        const time = new Date(d.timestamp).toLocaleTimeString();
        const date = new Date(d.timestamp).toLocaleDateString();
        const qty = d.quantity ? d.quantity.toFixed(0) : '';
        const price = d.price_at_decision ? `@$${d.price_at_decision.toFixed(2)}` : '';
        const prediction = d.predicted_direction && d.predicted_price_target
            ? `Prediction: ${d.predicted_direction} to $${d.predicted_price_target.toFixed(2)} in ${d.predicted_timeframe_days}d`
            : '';
        const accuracyBadge = d.prediction_accurate !== null
            ? `<span style="color: ${d.prediction_accurate ? 'var(--success)' : 'var(--error)'};">${d.prediction_accurate ? 'Correct' : 'Wrong'}</span>`
            : '';

        return `
            <div class="ai-decision-item">
                <div class="ai-decision-header">
                    <div>
                        <span class="ai-decision-action ${actionClass}">${escapeHtml(d.action)}</span>
                        <span class="ai-decision-symbol">${escapeHtml(d.symbol)}</span>
                        <span class="ai-decision-details">${qty} ${price}</span>
                    </div>
                    <div class="ai-decision-confidence">Conf: ${(d.confidence * 100).toFixed(0)}%</div>
                </div>
                <div class="ai-decision-reasoning">${escapeHtml(d.reasoning.substring(0, 300))}${d.reasoning.length > 300 ? '...' : ''}</div>
                ${prediction ? `<div class="ai-decision-prediction">${escapeHtml(prediction)} ${accuracyBadge}</div>` : ''}
                <div class="ai-decision-timestamp">${date} ${time} | ${escapeHtml(d.model_used)}</div>
            </div>
        `;
    }).join('');
}

function updateAiForecast(forecast: api.AiCompoundingForecast): void {
    const dailyEl = document.getElementById('ai-daily-return');
    const winRateEl = document.getElementById('ai-win-rate');
    const proj30El = document.getElementById('ai-proj-30d');
    const proj90El = document.getElementById('ai-proj-90d');
    const proj1yEl = document.getElementById('ai-proj-1y');
    const doubleEl = document.getElementById('ai-time-to-double');
    const bankruptEl = document.getElementById('ai-bankruptcy-risk');

    if (dailyEl) {
        const sign = forecast.current_daily_return >= 0 ? '+' : '';
        dailyEl.textContent = `${sign}${(forecast.current_daily_return * 100).toFixed(3)}%`;
        dailyEl.style.color = forecast.current_daily_return >= 0 ? 'var(--success)' : 'var(--error)';
    }
    if (winRateEl) winRateEl.textContent = `${(forecast.current_win_rate * 100).toFixed(0)}%`;
    if (proj30El) proj30El.textContent = `$${forecast.projected_30_days.toLocaleString('en-US', { maximumFractionDigits: 0 })}`;
    if (proj90El) proj90El.textContent = `$${forecast.projected_90_days.toLocaleString('en-US', { maximumFractionDigits: 0 })}`;
    if (proj1yEl) proj1yEl.textContent = `$${forecast.projected_365_days.toLocaleString('en-US', { maximumFractionDigits: 0 })}`;
    if (doubleEl) doubleEl.textContent = forecast.time_to_double ? `${forecast.time_to_double} days` : 'N/A';
    if (bankruptEl) bankruptEl.textContent = forecast.time_to_bankruptcy ? `${forecast.time_to_bankruptcy} days` : 'Low';
}

function updateAiAccuracy(accuracy: api.AiPredictionAccuracy): void {
    const pctEl = document.getElementById('ai-accuracy-pct');
    const countEl = document.getElementById('ai-predictions-count');
    const correctEl = document.getElementById('ai-correct-count');

    if (pctEl) pctEl.textContent = `${accuracy.accuracy_percent.toFixed(0)}%`;
    if (countEl) countEl.textContent = `${accuracy.total_predictions} predictions evaluated`;
    if (correctEl) correctEl.textContent = `${accuracy.accurate_predictions} correct`;
}

function updateAiBenchmark(benchmark: api.AiBenchmarkComparison): void {
    const alphaEl = document.getElementById('ai-alpha');
    if (alphaEl) {
        const sign = benchmark.alpha >= 0 ? '+' : '';
        alphaEl.textContent = `${sign}${benchmark.alpha.toFixed(2)}%`;
        alphaEl.style.color = benchmark.alpha >= 0 ? 'var(--success)' : 'var(--error)';
    }

    // Update performance chart if we have data
    if (benchmark.tracking_data && benchmark.tracking_data.length > 0) {
        updateAiPerformanceChart(benchmark.tracking_data);
    }
}

function updateAiPerformanceChart(data: [string, number, number][]): void {
    const chartEl = document.getElementById('ai-performance-chart');
    if (!chartEl) return;

    // For now, just show text summary. Full chart would use lightweight-charts
    if (data.length < 2) {
        chartEl.innerHTML = '<div class="empty-state" style="padding: 100px;">Not enough data for chart. Run more cycles to see performance.</div>';
        return;
    }

    const latest = data[data.length - 1];
    const portfolioReturn = ((latest[1] - 1000000) / 1000000 * 100).toFixed(2);
    const benchmarkReturn = ((latest[2] - 1000000) / 1000000 * 100).toFixed(2);

    chartEl.innerHTML = `
        <div style="padding: 20px; text-align: center;">
            <div style="display: flex; justify-content: center; gap: 40px; margin-bottom: 20px;">
                <div>
                    <div style="font-size: 0.8rem; color: var(--text-secondary);">Portfolio Return</div>
                    <div style="font-size: 1.5rem; font-weight: 700; color: ${parseFloat(portfolioReturn) >= 0 ? 'var(--success)' : 'var(--error)'};">${parseFloat(portfolioReturn) >= 0 ? '+' : ''}${portfolioReturn}%</div>
                </div>
                <div>
                    <div style="font-size: 0.8rem; color: var(--text-secondary);">SPY Benchmark</div>
                    <div style="font-size: 1.5rem; font-weight: 700; color: ${parseFloat(benchmarkReturn) >= 0 ? 'var(--success)' : 'var(--error)'};">${parseFloat(benchmarkReturn) >= 0 ? '+' : ''}${benchmarkReturn}%</div>
                </div>
            </div>
            <div style="font-size: 0.85rem; color: var(--text-secondary);">${data.length} data points | Latest: ${new Date(latest[0]).toLocaleString()}</div>
        </div>
    `;
}

async function aiStartSession(): Promise<void> {
    try {
        log('Starting AI trading session...', 'info');
        const session = await api.aiTraderStartSession();
        log(`AI session ${session.id} started`, 'success');
        await loadAiTrader();
    } catch (error) {
        log(`Error starting session: ${error}`, 'error');
    }
}

async function aiEndSession(): Promise<void> {
    try {
        log('Ending AI trading session...', 'info');
        const session = await api.aiTraderEndSession();
        if (session) {
            log(`AI session ${session.id} ended`, 'success');
        }
        await loadAiTrader();
    } catch (error) {
        log(`Error ending session: ${error}`, 'error');
    }
}

async function aiRunCycle(): Promise<void> {
    const runBtn = document.getElementById('ai-run-cycle-btn') as HTMLButtonElement;
    const modelStatus = document.getElementById('ai-model-status');

    try {
        if (runBtn) runBtn.disabled = true;
        if (modelStatus) modelStatus.textContent = 'Running AI cycle...';

        log('Running AI trading cycle...', 'info');
        const decisions = await api.aiTraderRunCycle();
        log(`AI cycle completed: ${decisions.length} decisions made`, 'success');

        if (modelStatus) {
            if (decisions.length > 0) {
                modelStatus.textContent = `Last: ${decisions[0].model_used}`;
            } else {
                modelStatus.textContent = 'Cycle complete - no actions';
            }
        }

        await loadAiTrader();
    } catch (error) {
        log(`Error running cycle: ${error}`, 'error');
        if (modelStatus) modelStatus.textContent = `Error: ${error}`;
    } finally {
        // Re-enable based on status
        const status = await api.aiTraderGetStatus();
        if (runBtn) runBtn.disabled = !status.is_running;
    }
}

async function aiReset(): Promise<void> {
    if (!confirm('Reset AI trading? This will clear all sessions, decisions, and reset to $1,000,000.')) {
        return;
    }

    try {
        log('Resetting AI trading...', 'info');
        await api.aiTraderReset(1000000);
        log('AI trading reset to $1,000,000', 'success');
        await loadAiTrader();
    } catch (error) {
        log(`Error resetting: ${error}`, 'error');
    }
}

async function aiEvaluatePredictions(): Promise<void> {
    try {
        log('Evaluating pending predictions...', 'info');
        const count = await api.aiTraderEvaluatePredictions();
        log(`Evaluated ${count} predictions`, 'success');
        await loadAiTrader();
    } catch (error) {
        log(`Error evaluating predictions: ${error}`, 'error');
    }
}

// ============================================================================
// DC TRADER FUNCTIONS
// ============================================================================

async function loadDcTrader(): Promise<void> {
    try {
        // Load DC balance
        const balance = await api.getDcBalance();
        updateDcBalance(balance);

        // Load positions
        const positions = await api.getDcPositions();
        updateDcPositions(positions);

        // Load trades
        const trades = await api.getDcTrades(50);
        updateDcTrades(trades);

        // Load competition stats
        const stats = await api.getCompetitionStats();
        updateCompetitionStats(stats);

        // Load team configs
        await loadTeamConfigs();

        log('DC Trader data loaded', 'success');
    } catch (error) {
        log(`Error loading DC Trader: ${error}`, 'error');
    }
}

function updateDcBalance(balance: api.DcWalletBalance): void {
    const portfolioEl = document.getElementById('dc-portfolio-value');
    const changeEl = document.getElementById('dc-portfolio-change');
    const cashEl = document.getElementById('dc-cash');
    const positionsEl = document.getElementById('dc-positions-value');

    if (portfolioEl) portfolioEl.textContent = `$${balance.total_equity.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
    if (cashEl) cashEl.textContent = `$${balance.cash.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
    if (positionsEl) positionsEl.textContent = `$${balance.positions_value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;

    if (changeEl) {
        const sign = balance.total_pnl_percent >= 0 ? '+' : '';
        changeEl.textContent = `${sign}${balance.total_pnl_percent.toFixed(2)}%`;
        changeEl.className = 'ai-stat-change ' + (balance.total_pnl_percent >= 0 ? 'positive' : 'negative');
    }
}

function updateDcPositions(positions: api.DcPosition[]): void {
    const container = document.getElementById('dc-positions-list');
    if (!container) return;

    if (positions.length === 0) {
        container.innerHTML = '<div class="empty-state">No positions. Execute a trade to get started.</div>';
        return;
    }

    container.innerHTML = positions.map(pos => {
        const pnlClass = pos.unrealized_pnl >= 0 ? 'positive' : 'negative';
        const sign = pos.unrealized_pnl >= 0 ? '+' : '';
        return `
            <div class="ai-decision-item" style="display: flex; justify-content: space-between; align-items: center;">
                <div>
                    <strong>${pos.symbol}</strong>
                    <span style="color: var(--text-secondary); margin-left: 8px;">${pos.quantity} shares @ $${pos.entry_price.toFixed(2)}</span>
                </div>
                <div style="text-align: right;">
                    <span>$${pos.current_value.toFixed(2)}</span>
                    <span class="${pnlClass}" style="margin-left: 8px;">${sign}$${pos.unrealized_pnl.toFixed(2)}</span>
                    <button class="btn-secondary dc-quick-sell" data-symbol="${pos.symbol}" data-qty="${pos.quantity}" style="margin-left: 12px; padding: 4px 8px;">SELL</button>
                </div>
            </div>
        `;
    }).join('');

    // Add quick sell handlers
    container.querySelectorAll('.dc-quick-sell').forEach(btn => {
        btn.addEventListener('click', async () => {
            const symbol = btn.getAttribute('data-symbol');
            const qty = parseFloat(btn.getAttribute('data-qty') || '0');
            if (symbol && qty > 0) {
                try {
                    await api.executeDcTrade(symbol, 'SELL', qty);
                    log(`Sold ${qty} ${symbol}`, 'success');
                    await loadDcTrader();
                } catch (error) {
                    log(`Error selling: ${error}`, 'error');
                }
            }
        });
    });
}

function updateDcTrades(trades: api.DcTrade[]): void {
    const container = document.getElementById('dc-trades-list');
    const todayEl = document.getElementById('dc-trades-today');
    if (!container) return;

    // Count today's trades
    const today = new Date().toISOString().split('T')[0];
    const todayTrades = trades.filter(t => t.timestamp.startsWith(today)).length;
    if (todayEl) todayEl.textContent = todayTrades.toString();

    if (trades.length === 0) {
        container.innerHTML = '<div class="empty-state">No trades yet.</div>';
        return;
    }

    container.innerHTML = trades.map(trade => {
        const time = new Date(trade.timestamp).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
        const actionClass = trade.action === 'BUY' ? 'positive' : 'negative';
        const pnlText = trade.pnl ? ` (${trade.pnl >= 0 ? '+' : ''}$${trade.pnl.toFixed(2)})` : '';
        const notesText = trade.notes ? ` - "${trade.notes}"` : '';
        return `
            <div class="ai-decision-item">
                <span style="color: var(--text-secondary);">${time}</span>
                <span class="${actionClass}" style="margin-left: 8px;">${trade.action}</span>
                <strong style="margin-left: 8px;">${trade.symbol}</strong>
                <span style="margin-left: 8px;">${trade.quantity} @ $${trade.price.toFixed(2)}${pnlText}</span>
                <span style="color: var(--text-secondary); margin-left: 8px;">${notesText}</span>
            </div>
        `;
    }).join('');
}

function updateCompetitionStats(stats: api.CompetitionStats): void {
    // KALIC stats
    const kalicTotal = document.getElementById('dc-kalic-total');
    const kalicPnl = document.getElementById('dc-kalic-pnl');
    const kalicTrades = document.getElementById('dc-kalic-trades');

    if (kalicTotal) kalicTotal.textContent = `$${stats.kalic_total.toLocaleString('en-US', { minimumFractionDigits: 0, maximumFractionDigits: 0 })}`;
    if (kalicPnl) {
        const sign = stats.kalic_pnl_pct >= 0 ? '+' : '';
        kalicPnl.textContent = `${sign}${stats.kalic_pnl_pct.toFixed(2)}%`;
        kalicPnl.className = 'ai-stat-change ' + (stats.kalic_pnl_pct >= 0 ? 'positive' : 'negative');
    }
    if (kalicTrades) kalicTrades.textContent = `${stats.kalic_trades} trades`;

    // DC stats
    const dcTotal = document.getElementById('dc-dc-total');
    const dcPnl = document.getElementById('dc-dc-pnl');
    const dcTrades = document.getElementById('dc-dc-trades');

    if (dcTotal) dcTotal.textContent = `$${stats.dc_total.toLocaleString('en-US', { minimumFractionDigits: 0, maximumFractionDigits: 0 })}`;
    if (dcPnl) {
        const sign = stats.dc_pnl_pct >= 0 ? '+' : '';
        dcPnl.textContent = `${sign}${stats.dc_pnl_pct.toFixed(2)}%`;
        dcPnl.className = 'ai-stat-change ' + (stats.dc_pnl_pct >= 0 ? 'positive' : 'negative');
    }
    if (dcTrades) dcTrades.textContent = `${stats.dc_trades} trades`;

    // Leader
    const leaderEl = document.getElementById('dc-leader-text');
    if (leaderEl) {
        if (stats.leader === 'TIE') {
            leaderEl.textContent = 'TIE';
        } else {
            leaderEl.textContent = `LEADER: ${stats.leader} (+$${stats.lead_amount.toLocaleString('en-US', { minimumFractionDigits: 0, maximumFractionDigits: 0 })})`;
        }
    }
}

async function loadTeamConfigs(): Promise<void> {
    try {
        const configs = await api.listTeamConfigs();
        const select = document.getElementById('dc-config-select') as HTMLSelectElement;
        if (select) {
            select.innerHTML = '<option value="">-- Select Config --</option>' +
                configs.map(c => `<option value="${c.name}">${c.name}</option>`).join('');
        }
    } catch (error) {
        // Configs may not exist yet
    }
}

async function executeDcTrade(): Promise<void> {
    const symbolEl = document.getElementById('dc-symbol') as HTMLInputElement;
    const quantityEl = document.getElementById('dc-quantity') as HTMLInputElement;
    const priceEl = document.getElementById('dc-price') as HTMLInputElement;
    const notesEl = document.getElementById('dc-notes') as HTMLInputElement;
    const actionEl = document.querySelector('input[name="dc-action"]:checked') as HTMLInputElement;

    const symbol = symbolEl?.value?.trim().toUpperCase();
    const quantity = parseFloat(quantityEl?.value || '0');
    const price = priceEl?.value ? parseFloat(priceEl.value) : undefined;
    const notes = notesEl?.value?.trim() || undefined;
    const action = actionEl?.value as 'BUY' | 'SELL';

    if (!symbol || !quantity || quantity <= 0) {
        log('Please enter a valid symbol and quantity', 'error');
        return;
    }

    try {
        const trade = await api.executeDcTrade(symbol, action, quantity, price, notes);
        log(`DC Trade: ${action} ${quantity} ${symbol} @ $${trade.price.toFixed(2)}`, 'success');

        // Clear form
        symbolEl.value = '';
        quantityEl.value = '';
        priceEl.value = '';
        notesEl.value = '';

        await loadDcTrader();
    } catch (error) {
        log(`Trade error: ${error}`, 'error');
    }
}

async function lookupDcPrice(): Promise<void> {
    const symbolEl = document.getElementById('dc-symbol') as HTMLInputElement;
    const priceEl = document.getElementById('dc-price') as HTMLInputElement;
    const symbol = symbolEl?.value?.trim().toUpperCase();

    if (!symbol) {
        log('Enter a symbol first', 'error');
        return;
    }

    try {
        const price = await api.lookupCurrentPrice(symbol);
        if (priceEl) priceEl.value = price.toFixed(2);
        log(`${symbol} current price: $${price.toFixed(2)}`, 'success');
    } catch (error) {
        log(`Price lookup failed: ${error}`, 'error');
    }
}

async function importDcTradesCsv(): Promise<void> {
    const contentEl = document.getElementById('dc-import-content') as HTMLTextAreaElement;
    const resultEl = document.getElementById('dc-import-result');
    const content = contentEl?.value?.trim();

    if (!content) {
        log('Paste CSV content first', 'error');
        return;
    }

    try {
        const result = await api.importDcTradesCsv(content);
        const msg = `Imported ${result.success_count} trades, ${result.error_count} errors`;
        if (resultEl) resultEl.textContent = msg;
        log(msg, result.error_count > 0 ? 'error' : 'success');
        if (result.errors.length > 0) {
            result.errors.forEach(err => log(err, 'error'));
        }
        contentEl.value = '';
        await loadDcTrader();
    } catch (error) {
        log(`Import error: ${error}`, 'error');
    }
}

async function importDcTradesJson(): Promise<void> {
    const contentEl = document.getElementById('dc-import-content') as HTMLTextAreaElement;
    const resultEl = document.getElementById('dc-import-result');
    const content = contentEl?.value?.trim();

    if (!content) {
        log('Paste JSON content first', 'error');
        return;
    }

    try {
        const result = await api.importDcTradesJson(content);
        const msg = `Imported ${result.success_count} trades, ${result.error_count} errors`;
        if (resultEl) resultEl.textContent = msg;
        log(msg, result.error_count > 0 ? 'error' : 'success');
        if (result.errors.length > 0) {
            result.errors.forEach(err => log(err, 'error'));
        }
        contentEl.value = '';
        await loadDcTrader();
    } catch (error) {
        log(`Import error: ${error}`, 'error');
    }
}

async function resetDcAccount(): Promise<void> {
    try {
        await api.resetDcAccount(1000000);
        log('DC account reset to $1,000,000', 'success');
        await loadDcTrader();
    } catch (error) {
        log(`Reset error: ${error}`, 'error');
    }
}

async function saveTeamConfig(): Promise<void> {
    const nameEl = document.getElementById('dc-config-name') as HTMLInputElement;
    const descEl = document.getElementById('dc-config-desc') as HTMLInputElement;
    const name = nameEl?.value?.trim();
    const description = descEl?.value?.trim() || undefined;

    if (!name) {
        log('Enter a config name', 'error');
        return;
    }

    try {
        await api.saveTeamConfig(name, description);
        log(`Config "${name}" saved`, 'success');
        await loadTeamConfigs();
        nameEl.value = '';
        descEl.value = '';
    } catch (error) {
        log(`Save error: ${error}`, 'error');
    }
}

async function recordDcSnapshot(): Promise<void> {
    try {
        await api.recordPortfolioSnapshot('DC');
        await api.recordPortfolioSnapshot('KALIC');
        log('Recorded snapshots for DC and KALIC', 'success');
    } catch (error) {
        log(`Snapshot error: ${error}`, 'error');
    }
}

async function syncDcPrices(): Promise<void> {
    try {
        // Add DC positions to auto-refresh favorites
        const result = await api.favoriteDcPositions();
        log(result.message, 'success');

        // Also add KALIC positions
        const result2 = await api.favoritePaperPositions();
        log(result2.message, 'success');

        // Now fetch fresh prices for all favorited symbols
        const favorites = await api.getFavoritedSymbols();
        if (favorites.length > 0) {
            const symbolList = favorites.join(',');
            log(`Fetching prices for ${favorites.length} symbols...`, 'info');
            const fetchResult = await api.fetchPrices(symbolList, '1d');
            log(fetchResult.message, fetchResult.success ? 'success' : 'error');
        }

        // Refresh DC trader display
        await loadDcTrader();
    } catch (error) {
        log(`Sync error: ${error}`, 'error');
    }
}

// Event listeners
function setupEventListeners(): void {
    // Tab switching
    document.querySelectorAll('.tab').forEach(tab => {
        tab.addEventListener('click', () => {
            const tabName = tab.getAttribute('data-tab');
            if (tabName) switchTab(tabName);
        });
    });

    // Fetch form
    document.getElementById('fetch-form')?.addEventListener('submit', (e) => {
        e.preventDefault();
        const symbols = (document.getElementById('symbols') as HTMLInputElement).value;
        const period = (document.getElementById('period') as HTMLSelectElement).value;
        if (symbols) fetchPrices(symbols, period);
    });

    // Quick actions
    document.querySelectorAll('.quick-action').forEach(btn => {
        btn.addEventListener('click', () => {
            const symbols = btn.getAttribute('data-symbols');
            const fred = btn.getAttribute('data-fred');
            const period = (document.getElementById('period') as HTMLSelectElement).value;

            if (symbols) fetchPrices(symbols, period);
            if (fred) fetchFred(fred);
        });
    });

    // Refresh button
    document.getElementById('refresh-btn')?.addEventListener('click', refreshSymbolList);

    // Sort select
    document.getElementById('sort-select')?.addEventListener('change', refreshSymbolList);

    // Search
    document.getElementById('search-input')?.addEventListener('input', (e) => {
        searchCompany((e.target as HTMLInputElement).value);
    });

    // Search results (event delegation)
    document.getElementById('search-results')?.addEventListener('click', (e) => {
        const target = e.target as HTMLElement;
        if (target.classList.contains('search-result')) {
            const symbol = target.getAttribute('data-symbol');
            if (symbol) {
                (document.getElementById('search-input') as HTMLInputElement).value = '';
                document.getElementById('search-results')!.innerHTML = '';
                const period = (document.getElementById('period') as HTMLSelectElement).value;
                fetchPrices(symbol, period);
            }
        }
    });

    // Auto-refresh
    document.getElementById('auto-refresh-toggle')?.addEventListener('change', toggleAutoRefresh);
    document.getElementById('refresh-interval')?.addEventListener('change', () => {
        const toggle = document.getElementById('auto-refresh-toggle') as HTMLInputElement;
        const intervalSelect = document.getElementById('refresh-interval') as HTMLSelectElement;

        // Save the new interval
        saveAutoRefreshState(toggle.checked, parseInt(intervalSelect.value));

        if (toggle.checked) {
            stopAutoRefresh();
            startAutoRefresh();
        }
    });

    // S&P 100 and ASX 100 buttons
    document.getElementById('sp100-btn')?.addEventListener('click', fetchSP100);
    document.getElementById('asx100-btn')?.addEventListener('click', fetchASX100);

    // Chart controls
    document.getElementById('load-chart-btn')?.addEventListener('click', loadChart);
    document.querySelectorAll('.chart-indicators input').forEach(checkbox => {
        checkbox.addEventListener('change', updateChartIndicators);
    });

    // Chart favorite button
    document.getElementById('chart-favorite-btn')?.addEventListener('click', async () => {
        const symbol = (document.getElementById('chart-symbol') as HTMLInputElement).value.trim().toUpperCase();
        if (!symbol) { alert('Enter a symbol first'); return; }
        await toggleFavoriteButton('chart-favorite-btn', symbol);
    });
    document.getElementById('chart-symbol')?.addEventListener('change', () => {
        updateFavoriteButtonState('chart-symbol', 'chart-favorite-btn');
    });

    // Indicators
    document.getElementById('calc-indicators-btn')?.addEventListener('click', calculateIndicators);
    document.getElementById('show-indicator-chart-btn')?.addEventListener('click', showIndicatorChart);

    // Indicator favorite button
    document.getElementById('indicator-favorite-btn')?.addEventListener('click', async () => {
        const symbol = (document.getElementById('indicator-symbol') as HTMLInputElement).value.trim().toUpperCase();
        if (!symbol) { alert('Enter a symbol first'); return; }
        await toggleFavoriteButton('indicator-favorite-btn', symbol);
    });
    document.getElementById('indicator-symbol')?.addEventListener('change', () => {
        updateFavoriteButtonState('indicator-symbol', 'indicator-favorite-btn');
    });

    // Indicator list click - show chart for clicked indicator
    document.getElementById('indicator-list')?.addEventListener('click', async (e) => {
        const target = (e.target as HTMLElement).closest('.symbol-item');
        if (target) {
            const indicatorName = target.getAttribute('data-indicator');
            const symbol = (document.getElementById('indicator-symbol') as HTMLInputElement).value.trim();
            if (indicatorName && symbol) {
                // Update dropdown to match clicked indicator
                const select = document.getElementById('indicator-select') as HTMLSelectElement;
                if (select) {
                    select.value = indicatorName;
                }
                // Show the chart
                initializeIndicatorChart();
                try {
                    log(`Loading ${indicatorName} chart for ${symbol}...`, 'info');
                    await indicatorChart?.loadIndicator(symbol, indicatorName);
                    log(`Indicator chart loaded`, 'success');
                } catch (error) {
                    log(`Error loading indicator chart: ${error}`, 'error');
                }
            }
        }
    });

    // Alerts
    document.getElementById('add-alert-btn')?.addEventListener('click', addAlert);

    // Alert favorite button
    document.getElementById('alert-favorite-btn')?.addEventListener('click', async () => {
        const symbol = (document.getElementById('alert-symbol') as HTMLInputElement).value.trim().toUpperCase();
        if (!symbol) { alert('Enter a symbol first'); return; }
        await toggleFavoriteButton('alert-favorite-btn', symbol);
    });
    document.getElementById('alert-symbol')?.addEventListener('change', () => {
        updateFavoriteButtonState('alert-symbol', 'alert-favorite-btn');
    });
    document.getElementById('check-alerts-btn')?.addEventListener('click', async () => {
        try {
            log('Checking alerts...', 'info');
            const triggered = await api.checkAlerts();
            if (triggered && triggered.length > 0) {
                const messages = triggered.map(a =>
                    `${a.symbol} ${a.condition === 'above' ? 'reached' : 'dropped to'} $${a.target_price.toFixed(2)}`
                ).join('\n');
                alert(`Alerts triggered!\n\n${messages}`);
                log(`${triggered.length} alerts triggered!`, 'success');
            } else {
                alert('No alerts triggered.');
                log('No alerts triggered', 'info');
            }
            await loadAlerts();
        } catch (error) {
            log(`Error checking alerts: ${error}`, 'error');
        }
    });

    // Alert deletion (event delegation)
    document.getElementById('alerts-list')?.addEventListener('click', async (e) => {
        const target = e.target as HTMLElement;
        if (target.classList.contains('delete-alert-btn')) {
            const id = parseInt(target.getAttribute('data-id') || '0');
            if (id) {
                try {
                    const result = await api.deleteAlert(id);
                    log(result.message, result.success ? 'success' : 'error');
                    await loadAlerts();
                } catch (error) {
                    log(`Error deleting alert: ${error}`, 'error');
                }
            }
        }
    });

    // Portfolio
    document.getElementById('add-position-btn')?.addEventListener('click', addPosition);

    // Position deletion (event delegation)
    document.getElementById('portfolio-list')?.addEventListener('click', async (e) => {
        const target = e.target as HTMLElement;
        if (target.classList.contains('delete-position-btn')) {
            if (!confirm('Remove this position?')) return;
            const id = parseInt(target.getAttribute('data-id') || '0');
            if (id) {
                try {
                    const result = await api.deletePosition(id);
                    log(result.message, result.success ? 'success' : 'error');
                    await loadPortfolio();
                } catch (error) {
                    log(`Error deleting position: ${error}`, 'error');
                }
            }
        }
    });

    // Symbol list click - toggle favorite or view in chart
    document.getElementById('symbol-list')?.addEventListener('click', async (e) => {
        const target = e.target as HTMLElement;

        // Check if clicked on favorite toggle (moon icon)
        if (target.classList.contains('favorite-toggle')) {
            e.stopPropagation();
            const symbol = target.getAttribute('data-symbol');
            if (symbol) {
                try {
                    const newState = await api.toggleFavorite(symbol);
                    target.textContent = newState ? '🌙' : '☽';
                    target.classList.toggle('favorited', newState);
                    log(`${symbol} ${newState ? 'added to' : 'removed from'} auto-refresh`, 'info');
                } catch (error) {
                    log(`Error toggling favorite: ${error}`, 'error');
                }
            }
            return;
        }

        // Otherwise, view in chart
        const item = target.closest('.symbol-item');
        if (item) {
            const symbol = item.getAttribute('data-symbol');
            if (symbol) {
                (document.getElementById('chart-symbol') as HTMLInputElement).value = symbol;
                (document.getElementById('indicator-symbol') as HTMLInputElement).value = symbol;
                switchTab('chart');
                loadChart();
            }
        }
    });

    // Groups tab
    document.getElementById('create-group-btn')?.addEventListener('click', createGroup);
    document.getElementById('add-symbol-btn')?.addEventListener('click', addSymbolToGroup);
    document.getElementById('delete-group-btn')?.addEventListener('click', deleteGroup);
    document.getElementById('fetch-group-btn')?.addEventListener('click', fetchGroupPrices);

    // Group favorite button
    document.getElementById('group-favorite-btn')?.addEventListener('click', async () => {
        const symbol = (document.getElementById('add-symbol-input') as HTMLInputElement).value.trim().toUpperCase();
        if (!symbol) { alert('Enter a symbol first'); return; }
        await toggleFavoriteButton('group-favorite-btn', symbol);
    });

    // Groups list click - load detail
    document.getElementById('groups-list')?.addEventListener('click', (e) => {
        const target = (e.target as HTMLElement).closest('.group-item');
        if (target) {
            const groupName = target.getAttribute('data-group');
            if (groupName) loadGroupDetail(groupName);
        }
    });

    // Group symbols list - remove symbol
    document.getElementById('group-symbols-list')?.addEventListener('click', async (e) => {
        const target = e.target as HTMLElement;
        if (target.classList.contains('remove-symbol-btn')) {
            const symbol = target.getAttribute('data-symbol');
            if (symbol) await removeSymbolFromGroup(symbol);
        }
    });

    // Preset groups
    document.querySelectorAll('.preset-group').forEach(btn => {
        btn.addEventListener('click', () => {
            const name = btn.getAttribute('data-name');
            const symbols = btn.getAttribute('data-symbols');
            const desc = btn.getAttribute('data-desc');
            if (name && symbols) createPresetGroup(name, symbols, desc || '');
        });
    });

    // AI Search
    document.getElementById('ai-search-btn')?.addEventListener('click', performAISearch);
    document.getElementById('ai-search-query')?.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') performAISearch();
    });
    document.getElementById('add-event-btn')?.addEventListener('click', addMarketEventUI);
    document.getElementById('add-pattern-btn')?.addEventListener('click', addPricePatternUI);

    // Finnhub News
    document.getElementById('fetch-news-btn')?.addEventListener('click', fetchNewsAndPopulateForm);
    document.getElementById('save-finnhub-key-btn')?.addEventListener('click', saveFinnhubApiKey);
    loadFinnhubApiKey();

    // Claude AI Chat
    document.getElementById('save-api-key-btn')?.addEventListener('click', saveClaudeApiKey);
    document.getElementById('claude-chat-btn')?.addEventListener('click', sendClaudeChat);
    document.getElementById('claude-chat-query')?.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') sendClaudeChat();
    });
    // Load saved API key on startup
    loadClaudeApiKey();

    // AI Trader
    document.getElementById('ai-start-session-btn')?.addEventListener('click', aiStartSession);
    document.getElementById('ai-end-session-btn')?.addEventListener('click', aiEndSession);
    document.getElementById('ai-run-cycle-btn')?.addEventListener('click', aiRunCycle);
    document.getElementById('ai-reset-btn')?.addEventListener('click', aiReset);
    document.getElementById('ai-evaluate-btn')?.addEventListener('click', aiEvaluatePredictions);

    // DC Trader
    document.getElementById('dc-execute-trade-btn')?.addEventListener('click', executeDcTrade);
    document.getElementById('dc-lookup-price-btn')?.addEventListener('click', lookupDcPrice);
    document.getElementById('dc-import-csv-btn')?.addEventListener('click', importDcTradesCsv);
    document.getElementById('dc-import-json-btn')?.addEventListener('click', importDcTradesJson);
    document.getElementById('dc-reset-account-btn')?.addEventListener('click', resetDcAccount);
    document.getElementById('dc-save-config-btn')?.addEventListener('click', saveTeamConfig);
    document.getElementById('dc-sync-prices-btn')?.addEventListener('click', syncDcPrices);
    document.getElementById('dc-snapshot-btn')?.addEventListener('click', recordDcSnapshot);
}

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    log('Financial Pipeline UI loaded', 'success');
    setupEventListeners();
    refreshSymbolList();
    updateLastRefreshTime();

    // Set default date for position form
    const dateInput = document.getElementById('position-date') as HTMLInputElement;
    if (dateInput) {
        dateInput.valueAsDate = new Date();
    }

    // Restore auto-refresh state from localStorage
    restoreAutoRefreshState();
});
