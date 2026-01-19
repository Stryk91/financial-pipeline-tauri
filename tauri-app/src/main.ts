// Main entry point
import './styles.css';
import * as api from './api';
import { TradingViewChart, IndicatorChart } from './chart';

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

        resultsDiv.innerHTML = results.map(r => `
            <div class="ai-result-item" data-type="${r.result_type}">
                <div class="ai-result-header">
                    <span class="ai-result-type ${r.result_type}">${r.result_type.replace('_', ' ')}</span>
                    <span class="ai-result-score">${(r.score * 100).toFixed(1)}% match</span>
                    ${r.symbol ? `<span class="ai-result-symbol">${r.symbol}</span>` : ''}
                </div>
                <div class="ai-result-content">${r.content}</div>
                ${r.date ? `<div class="ai-result-date">${r.date}</div>` : ''}
            </div>
        `).join('');

        log(`Found ${results.length} results for "${query}"`, 'success');
    } catch (error) {
        resultsDiv.innerHTML = `<p class="empty-state error">Error: ${error}</p>`;
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

    const responseDiv = document.getElementById('claude-chat-response')!;
    const metaDiv = document.getElementById('claude-chat-meta')!;
    const chatBtn = document.getElementById('claude-chat-btn') as HTMLButtonElement;

    responseDiv.innerHTML = '<p style="color: var(--accent);">Asking Claude...</p>';
    metaDiv.style.display = 'none';
    chatBtn.disabled = true;

    try {
        log(`Claude query: "${query.substring(0, 50)}..."`, 'info');
        const result = await api.claudeChat(query, apiKey);

        // Format the response with markdown-like styling
        const formattedResponse = result.response
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
        responseDiv.innerHTML = `<p class="empty-state" style="color: var(--error);">Error: ${error}</p>`;
        log(`Claude error: ${error}`, 'error');
    } finally {
        chatBtn.disabled = false;
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

    // Claude AI Chat
    document.getElementById('save-api-key-btn')?.addEventListener('click', saveClaudeApiKey);
    document.getElementById('claude-chat-btn')?.addEventListener('click', sendClaudeChat);
    document.getElementById('claude-chat-query')?.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') sendClaudeChat();
    });
    // Load saved API key on startup
    loadClaudeApiKey();
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
