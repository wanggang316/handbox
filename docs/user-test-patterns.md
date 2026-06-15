# User-Test Patterns

> Project-wide conventions for writing user-level tests. Authored during the first run of `/hs-test-spec` (Step 0: Bootstrap). Read this before writing any individual user-test case; case files live in `docs/user-tests/`.

## Status

**Status:** Draft
**Last updated:** 2026-05-25

## Platforms in Scope

- **Tauri 2 desktop app (macOS)** — the only user-facing surface today. WKWebView frontend + Rust backend via Tauri IPC. Runs locally; no cloud service.

Mobile, web SaaS, CLI are explicitly out of scope.

## Tooling per Platform

### Tauri desktop app

- **Primary:** manual operator-driven probe — the developer follows the case's numbered steps in the running app and asserts the named observable. Each case names exactly what to inspect (visible label, IPC return shape, log line, file contents).
- **Fallback (Rust-internal probes):** `cargo test --manifest-path src-tauri/Cargo.toml --lib <pattern>` for assertions that can be checked against in-process state without launching the UI. Used when an assertion is provable from a unit-level harness with no observable user impact.
- **Invocation:**
  - Start the app: `npm run tauri dev` from the repo root. Wait until the title-bar window appears and the dev server logs `ready in NNNms`.
  - Inspect Tauri logs: tail stdout of `npm run tauri dev`; backend logs use `tracing` — grep on `[ServiceName::method_name]` prefix.
  - Inspect SQLite DB: `sqlite3 ~/Library/Application\ Support/com.gumpw.handbox/handbox.db "<SQL>"` (path may vary per Tauri 2 `app_data_dir`).
  - Inspect IPC payloads: the frontend's `apiCall` wrapper logs request/response to the browser console; open WKWebView devtools via the in-app menu.
- **Ready signal:** the app's main window renders the sidebar AND the Tauri stdout contains `Successfully loaded LLM config from`.
- **Base URL discovery:** N/A — app launches its own window.

### Frontend UI / component gallery (web preview)

For pure-frontend work (UI consistency, component refactors, theming, accessibility) that needs no Tauri IPC, the vite web preview is a cheaper, deterministic surface than `npm run tauri dev`.

- **Primary:** `npm run dev` → http://localhost:1420/ (vite, **strictPort** — port 1420 must be free) → navigate to the live component gallery at **`/settings/components`** (demos ~24 of 26 `ui/` components with real `$state` bindings). Probe with Chrome MCP (`mcp__claude-in-chrome__*`) or computer-use screenshots.
- **Theme toggle (runtime):** inject `document.documentElement.setAttribute("data-theme", "dark" | "light")` — this is exactly what the `[data-theme="dark"]` CSS selectors match; most reliable, fewest side effects. Capture both themes for any visual assertion.
- **Static-check diff:** `npm run check` (= `svelte-kit sync && svelte-check`). It is the only frontend static gate (no prettier/eslint configured). Assertions compare against a **recorded baseline** (state the baseline error/warning count + files in the case); any failure beyond baseline = FAIL.
- **Structural guards (grep):** for invariants with no visible surface (an anti-pattern removed, a token present/absent), `grep` the source as terminal-output evidence — e.g. `grep -rn "var(--color-blue-500)" src` returns empty. These are `(guard: ...)` assertions, not user-observable behavior.
- **Ready signal:** vite stdout logs `ready in NNNms` AND the gallery page renders its "UI 组件测试" heading with no console errors.
- **Coverage gap:** the gallery does NOT demo `MenuButton` or `TitleBar`; assert those separately (extend the gallery, or inspect in `tauri dev`).

### HTTP API / Web / Mobile

Not in scope for HandBox.

## Case Dimensions

| Dimension | Required? | What to check |
|---|---|---|
| Happy path | Mandatory | The primary success flow for this case |
| Error path | Optional (Mandatory for write paths) | At least one declared failure mode if the case touches a write or external call |
| Edge values | Mandatory for input-handling cases | Empty / boundary / unicode / large-attachment inputs |
| Accessibility | Optional | Tauri WKWebView; revisit when we ship to broader audiences |
| Performance budget | Optional | Use only when the spec named a target |
| Regression (no-change-after-refactor) | Mandatory for refactor cases | The behavior the refactor must not break, with the exact observable that proves it |
| Security | Mandatory for auth / data / shell cases | AuthN/AuthZ boundary, API keys not in logs, shell argument escaping |

## Selector and Assertion Rules

### Allowed selectors (Tauri WKWebView)

- ARIA role + accessible name when announced (`button` with label "添加供应商")
- Visible text by user-facing language (Chinese strings on the actual UI: "供应商", "模型", "发送", etc.)
- Stable Tauri command names (`provider_list`, `hand_ai_list_providers`)
- SQLite query: `SELECT count(*) FROM providers WHERE provider_type = 'openai'`
- File presence: `test -f static/logo-150.png`
- Log substring: `grep -F '[MessageService::call_llm_api_stream]' <log>`

### Forbidden selectors

- CSS class names (`.text-base-content`) — Tailwind class churn breaks them
- File paths inside `src/` or `src-tauri/src/` — those are implementation, not user surface
- DOM position (`div:nth-child(2) > span`) — Svelte reorders
- Internal-only IDs (Svelte component instance ids, store keys) — couples test to source layout

### Allowed assertions

- Binary: PASS / FAIL, no "looks roughly right"
- Concrete: name the expected literal value
- Independent: one observable per assertion, no compound ("and the response was fast")

## State Isolation

- **DB reset:** developer can reset the DB by deleting `~/Library/Application Support/com.gumpw.handbox/handbox.db` and restarting the app; the app will recreate a fresh schema. For tests that must avoid wiping personal data, the case names a backup-restore protocol explicitly.
- **No cross-case state:** a case states its exact preconditions; cases must not assume another case has run.
- **External services:** chat cases that need a live provider use the operator's actual API keys configured in HandBox's settings; never hardcode keys in cases.

## Personas Registry

**Location:** `docs/user-tests/_shared/personas.yaml`

**Schema:**

```yaml
personas:
  - id: <persona-id>
    description: <one-line summary>
    role: <user / admin / developer-operator>
    fixtures: <list of fixture file paths required>
    notes: <optional>
```

This project today has one persona: `developer_operator` — the developer running HandBox locally, with access to logs, the DB file, and the Tauri stdout.

## Fixtures and Test Data

- **Location:** `docs/user-tests/<feature>/fixtures/` for feature-local; `docs/user-tests/_shared/fixtures/` for cross-feature
- **Naming:** `<purpose>.<ext>` — e.g. `tiny-provider-list.sql`, `single-image-attachment.json`
- **DB seeds:** SQL files that can be piped into sqlite3 against a fresh DB

## Artifacts on FAIL

- **Screenshot:** macOS Cmd+Shift+5 region capture → `~/Downloads/handbox-ut-fail-<case-id>-<timestamp>.png`
- **Logs:** redirect `npm run tauri dev` output to `/tmp/handbox-ut-<case-id>.log` when probing
- **DB dump:** `sqlite3 <db> .dump > /tmp/handbox-ut-<case-id>.db.sql` after a failed write-path case
- Retention: developer keeps until the case re-passes; no auto-cleanup yet

## Failure-Reproduction Expectation

A FAIL must ship with: exact step where the assertion broke, observed value vs. expected, full artifact bundle (per Artifacts on FAIL), and the commit SHA the case was run against.

## Anti-Patterns

- **Probing private state.** A case must not grep `src/`, must not call an internal Rust function. The case can only see what an IPC return, the rendered UI, or the DB / log surface.
- **Hallucinated coverage.** A case must trace to a spec AC, an ExecPlan task, or a regression observation declared in a docs/exec-plans/ file. No invented assertions.
- **Selector tied to Tailwind class.** Tailwind class churn (e.g. `text-base-100 → text-base-200`) must not break a case.
- **Compound assertions.** "It works fast and correctly" is two assertions; split them.
- **Grader gaming.** A case that hides its assertion behind a vague predicate ("system functions as expected") is impossible to falsify; rewrite with concrete observables.
- **Case depends on another case having run.** Tests must be runnable in any order; preconditions must be self-contained.
