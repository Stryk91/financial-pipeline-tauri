# Financial Pipeline Dev Log

> **Location:** `X:\dev\financial-pipeline-rs\DEV_LOG.md`  
> **Purpose:** Living document of changes, errors, solutions. DC reads via project files. KALIC writes during work.

---

## Format Guide

### Changes
```
## [YYYY-MM-DD] Brief Title
**Author:** KALIC | DC | STRYK
**Files:** path/to/file.rs, another/file.ts
**Summary:** What changed and why

- Bullet points of specific changes
- Keep it concise
```

### Errors & Solutions
```
### âŒ ERROR: Brief description
**When:** What triggered it
**Fix:** How it was solved
**Prevention:** How to avoid in future (optional)
```

---

## Log Entries

## [2026-01-21] PATH Self-Healing System
**Author:** DC
**Files:** `X:\dev\tools\kalic-path-hook.ps1`, PhiSHRI T45WIN_ENV_BOOTSTRAP
**Summary:** Created permanent fix for Windows PATH inheritance issues across WSL boundary

- Created `kalic-path-hook.ps1` with canonical paths for npm, node, cargo, git
- Functions: `Repair-PathForTool`, `Get-FullToolPath`, `Initialize-KalicEnv`
- PhiSHRI door T45WIN_ENV_BOOTSTRAP documents all known PATH issues
- KALIC should run `Initialize-KalicEnv` at session start

### âŒ ERROR: npm not recognized in WSL->PowerShell
**When:** Every time KALIC runs npm from WSL bash calling PowerShell
**Fix:** `. "X:\dev\tools\kalic-path-hook.ps1"; Repair-PathForTool "npm"`
**Prevention:** Run `Initialize-KalicEnv` at session start

---

## [2026-01-21] PhiSHRI Path Correction
**Author:** DC
**Files:** `claude_desktop_config.json`
**Summary:** MCP was pointing to stale PhiSHRI location with 567 doors instead of production 802

- Changed `PHISHRI_PATH` from `C:\Users\Stryker\.phishri\knowledge` to `C:\Dev\PhiSHRI\PhiSHRI`
- Changed `PHISHRI_SESSION_ROOT` to `C:\Dev\PhiSHRI`
- Requires Claude Desktop restart to take effect

---

## [2026-01-21] Tauri devUrl localhost fix
**Author:** KALIC
**Files:** `tauri-app/src-tauri/tauri.conf.json`
**Summary:** Fixed hardcoded IP that broke every rebuild

- Changed devUrl from `10.0.134.178:1420` to `localhost:1420`
- Applied `git update-index --skip-worktree` to prevent tracking local changes
- No more editing this file every compile

---

## [2026-01-21] Finnhub API Expansion - News Pattern Linking
**Author:** KALIC
**Files:** `src/finnhub.rs`, `tauri-app/src-tauri/src/lib.rs`, `tauri-app/src/api.ts`
**Summary:** Auto-link price patterns to news events

- Quote, Candles, PriceReaction structs implemented
- `add_market_event_with_pattern` Tauri command
- UI checkbox "Auto-link price patterns" + Save All button
- Fetches Â±3 day candles around news event date

---


---

## [2026-01-21] News Cards Enhanced with Price + Sentiment
**Author:** KALIC
**Files:** `src-tauri/src/lib.rs,src/api.ts,src/main.ts,src/chart.ts,src-tauri/tauri.conf.json`
**Summary:** News cards now show price at date, daily % change, and outcome-based sentiment

- Added `fetch_candles` Tauri command returning raw OHLCV data with dates
- News cards display: `$142.50 ▲2.3% BULLISH` on header row
- Sentiment is outcome-based (actual price movement): ≥+2% = BULLISH, ≤-2% = BEARISH
- Fixed CSP: added `http://ipc.localhost` to `connect-src`
- Fixed chart "Value is null" error in time scale sync
- Used `npm run tauri build` (not just `cargo build`) to bundle frontend


### âŒ ERROR: Finnhub candles 403 Forbidden
**When:** Fetching price data for news cards
**Fix:** Use local Yahoo price history instead of Finnhub /stock/candle
**Prevention:** Finnhub free tier restricts candle data; always try local data first

---

## [2026-01-21] Paper Trading Simulator - Backend Complete
**Author:** KALIC
**Files:** `src/models.rs`, `src/db.rs`, `src/lib.rs`, `tauri-app/src-tauri/src/lib.rs`, `tauri-app/src/api.ts`
**Summary:** Implemented paper trading system per TRADING_SIM_SPEC.md

**Database:**
- `paper_wallet` table - singleton with $100k starting cash
- `paper_positions` table - open positions with entry price/date
- `paper_trades` table - full trade history with P&L

**Rust Models:**
- `PaperWallet`, `PaperPosition`, `PaperTrade`, `PaperTradeAction` structs

**Database Methods:**
- `get_paper_wallet()` - get cash balance
- `get_paper_positions()` / `get_paper_position(symbol)` - get positions
- `execute_paper_trade(symbol, action, qty, price, ...)` - BUY/SELL with validation
- `get_paper_trades(symbol, limit)` - trade history
- `reset_paper_account(starting_cash)` - reset to clean state
- `get_paper_portfolio_value()` - (cash, positions_value, total_equity)

**Tauri Commands:**
- `get_paper_balance` - wallet summary with P&L
- `get_paper_positions` - positions with current prices and unrealized P&L
- `execute_paper_trade` - execute trade (validates cash/shares)
- `get_paper_trades` - trade history
- `reset_paper_account` - reset account

**TypeScript API (api.ts):**
- Full type definitions and async functions for all commands

**Build Status:** Compiles (both lib and tauri-app)

**Next:** Frontend UI (sidebar panel for trading)

---

## [2026-01-21] AI Trader Guardrails & Circuit Breaker
**Author:** KALIC
**Files:** `src/ai_trader.rs`, `src/db.rs`, `src/models.rs`, `src/ollama.rs`, `tauri-app/src-tauri/src/lib.rs`
**Summary:** Implemented autonomous trading safety system per DC spec

**Trading Modes (TradingMode enum):**
- `Aggressive` - 33% max position, 20 trades/day, no confluence required (STRYK override only)
- `Normal` - 10% max position, 10 trades/day, confluence required (default)
- `Conservative` - 5% max position, 5 trades/day, strict confluence (circuit breaker fallback)
- `Paused` - No new trades, position management only

**Circuit Breaker:**
- -10% daily loss threshold → auto-switch to conservative
- 5 consecutive losses → 1 hour trading pause
- Auto-conservative on trigger (configurable)
- All triggers logged to `circuit_breaker_events` table

**TradeResult Enum (Audit Trail):**
- `Executed { trade_id, symbol, action, quantity, price, value, timestamp }`
- `Queued { reason, review_by, proposed_trade }`
- `Rejected { reason, rule_triggered, proposed_trade }`

**Override Escape Hatch:**
- Time-limited elevated permissions for STRYK
- `Override::timed(hours, max_pct, reason)` with auto-expiry
- Audit logged with reason

**Database:**
- `trade_rejections` table - every rejected trade with reason + rule
- `circuit_breaker_events` table - trigger history
- Extended `ai_trader_config` with mode, CB settings, guardrails
- Migration support for existing databases

**Tauri Commands:**
- `ai_trader_get_mode` / `ai_trader_switch_mode`
- `ai_trader_get_circuit_breaker` / `ai_trader_update_circuit_breaker`
- `ai_trader_get_rejections` / `ai_trader_get_circuit_breaker_events`

**Git:** Merged to main via PR #2

---

## Pending / TODO

- [x] Add price display to news cards (symbol price at time of news)
- [x] Integrate Finnhub `/news-sentiment` endpoint for bullish/bearish scores (used outcome-based instead)
- [x] Paper trading backend (db, Tauri commands, TypeScript API)
- [x] AI Trader guardrails & circuit breaker
- [ ] Paper trading frontend UI (sidebar panel)
- [ ] Guardrails mode switcher UI in AI Trader tab
- [ ] Ollama tool calling integration
- [ ] Vector learning hooks capturing KALIC tool executions

---

## Quick Reference

| Issue | Solution |
|-------|----------|
| npm not found | `. "X:\dev\tools\kalic-path-hook.ps1"; Repair-PathForTool "npm"` |
| cargo not found | `Repair-PathForTool "cargo"` |
| devUrl wrong IP | Already fixed + skip-worktree applied |
| PhiSHRI wrong count | Restart Claude Desktop (config updated) |

| localhost:1420 refused | Run `npm run tauri dev` (starts Vite + Tauri together), OR `npm run build` first if using debug exe |
| Frontend not bundled | Use `npm run tauri build`, NOT just `cargo build --release` |


---

## [2026-01-21] npm.ps1 Wrapper Broken in PowerShell
**Author:** DC
**Files:** C:\Program Files\nodejs\npm.ps1
**Summary:** Node.js npm.ps1 wrapper script fails with $LASTEXITCODE not set error

- npm.ps1 line 17 and 50 reference $LASTEXITCODE before it's initialized
- Affects ALL pwsh sessions trying to run npm commands
- KALIC was stuck in rebuild loop hitting this wall
- **FIX:** Use cmd.exe shell instead of pwsh for npm/node commands

### ❌ ERROR: npm fails in PowerShell
**When:** Running 
pm run dev or any npm command in pwsh
**Fix:** Use cmd shell: cmd /c "npm run dev" OR create cmd-based terminal session
**Prevention:** KALIC should use cmd shells for Node.js projects, not pwsh


---

## [2026-01-21] KALIC Report & Log Analysis Toolkit
**Author:** DC
**Files:** `tools\kalic_*.py`, `tools\kalic_*.bat`, `knowledge\KALIC_*.md`
**Summary:** Complete PDF report generation and AI decision log analysis system

**Scripts (X:\dev\financial-pipeline-rs\tools\):**
- `kalic_log_analyzer.py` - Parses ai_decisions/*.jsonl, calculates metrics
- `kalic_report_regen_v2.py` - PDF generator with AI performance section
- `kalic_report_export.py` - Static snapshot PDF (data baked in)
- `kalic_full_report.bat` - One-click: analyzer + PDF + JSON export
- `kalic_regen.bat` / `kalic_export.bat` - Individual launchers

**Documentation (X:\dev\financial-pipeline-rs\knowledge\):**
- `KALIC_TOOLKIT_README.md` - Full toolkit documentation
- `KALIC_REPORTS_README.md` - Original report docs

**Features:**
- 3-page PDF: Portfolio, Projections/Risk, AI Performance
- Log analysis: action breakdown, confidence distribution, model stats
- Markdown digest generation for daily review
- JSON export for programmatic integration
- Reads live from logs/ai_decisions/*.jsonl and reports/ai_trader_report_*.md

**Dependency:** `pip install reportlab`

