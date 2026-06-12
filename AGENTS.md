# Repository Guidelines

## Project Structure & Module Organization

The SvelteKit frontend lives in `src/`:
- UI components in `src/lib/components` (PascalCase.svelte)
- State management in `src/lib/states` (xxx.svelte.ts using Svelte 5 runes)
- API helpers in `src/lib/api` (index.ts with named exports)
- Route groups in `src/routes/(app)` and `src/routes/settings`
- Shared styles in `src/app.css`
- Provider assets in `static/`

The Tauri backend sits in `src-tauri/`:
- IPC entry points in `src/commands`
- Business logic in `src/services` and `src/storage`
- SQLx migrations in `src-tauri/migrations`
- Crates in `src-tauri/crates/`

Reference docs under `docs/`; treat `handbox.db` as disposable dev data.

## Build, Test, and Development Commands

**Frontend (from root):**
- `npm install` – install SvelteKit and Tauri frontend dependencies
- `npm run dev` – launch web preview on Vite with hot module reload
- `npm run tauri dev` – run the desktop shell (requires `@tauri-apps/cli`)
- `npm run check` – synchronize kit metadata and run `svelte-check` for type/accessibility
- `npm run build` / `npm run preview` – produce and inspect production SPA bundle

**Backend (from src-tauri/):**
- `cargo fmt -- --check` – enforce Rust formatting (repo-wide; currently red on legacy files)
- `rustfmt --edition 2021 --check --config skip_children=true <files>` – the only safe **per-file** format check; never run bare `rustfmt` or `cargo fmt -- <file>` on `lib.rs`-reachable files, as rustfmt follows `mod` declarations and reformats the whole crate
- `cargo test` – run all backend tests
- `cargo test test_name` – run single test by name
- `cargo test -- --ignored` – run ignored tests (useful when migrations change storage)
- `cargo test --lib` – run library unit tests only

## Coding Style & Naming Conventions

**TypeScript/Svelte:**
- Use `lang="ts"` in all `<script>` blocks
- Two-space indentation
- Svelte components: `PascalCase.svelte`
- State files: `xxx.svelte.ts` (e.g., `chat.svelte.ts`)
- Exports and stores: `camelCase` (e.g., `chatState`, `chatActions`)
- Constants: `CAPITALIZED_SNAKE_CASE`

**Rust:**
- Modules and functions: `snake_case`
- Types: `PascalCase`
- Constants: `CAPITALIZED_SNAKE_CASE`

**State Management Pattern (Svelte 5):**
```typescript
// xxx.svelte.ts
let stateXxx = $state<T>(initialValue);
export const stateXxx = { get value() { return stateXxx }, set value(v) { stateXxx = v } };
export const actionsXxx = { /* async methods */ };
```

**Imports:**
- Use `$lib/` alias for internal imports
- API modules: `import * as chatApi from "$lib/api/chat"`
- Types: `import type { Chat, UUID } from "$lib/types"`
- Group imports by category (std → external → internal)

**Styling:**
- Prefer Tailwind utility classes for layout
- Use component `<style>` blocks only for bespoke animations
- CSS variables: `var(--primary)`, `var(--base-content)`

**Error Handling:**
- Rust: Return `Result<T, AppError>` from commands; use `thiserror` for error types
- Frontend: Use `normalizeError()` and `showAppError()` from `$lib/utils/error`
- Always catch and log async errors: `.catch((error) => console.error(...))`

## Testing Guidelines

**Frontend:**
- Tests in `src/lib/__tests__/` (Vitest or Playwright)
- Keep `npm run check` clean for all contributions

**Backend:**
- Tests in `src-tauri/crates/*/tests/` with `mod name_should_do_thing` naming
- Use `#[test]` attributes for unit tests
- Use `#[tokio::test]` for async tests
- Follow existing patterns in `test_supported_methods.rs`

**PR Requirements:**
- Run `npm run check` and `cargo fmt -- --check` before committing
- Document manual testing steps if automation isn't feasible

## Commit & Pull Request Guidelines

Follow Conventional Commits: `feat:`, `refactor:`, `fix:`, `docs:`, `chore:`
- Write imperative, present-tense summaries under 70 characters
- Each PR should describe intent, testing evidence, and link to relevant docs
- Include before/after screenshots for user-facing changes

## Security & Configuration Tips

- Never commit provider secrets; keep API keys in OS keychain or environment
- Treat `src-tauri/llm_config.json` as a template, not a source of real credentials
- Rust layer depends on sibling `openai-rust` and `google-genai-rust` crates
- Redact `tracing` logs before sharing externally
- Use `dotenvy` for local dev (non-fatal if `.env` missing)

## Upstream Dependency: hand-ai

HandBox's model catalog, capability metadata, and chat dispatch run entirely on the
sibling `hand-ai` project (the `hand-ai-model` crate from `wanggang316/hand-ai`). The
model list shown in the UI comes **100% from hand-ai's static catalog** — HandBox does
**no** live `/v1/models` polling. This is a deliberate single-source-of-truth design;
see `docs/exec-plans/dissolve-handbox-llm.md`.

**When you hit a hand-ai problem, file an issue upstream — do not work around it in
HandBox.** Catalog drift (a model the provider doesn't actually serve), missing
capability metadata, or a bug inside `hand_ai_model` are hand-ai's responsibility;
patching around them here re-introduces the divergence the dissolve migration removed.

How to file:

```bash
gh issue create --repo wanggang316/hand-ai \
  --label bug \                 # or: enhancement
  --title "[catalog] <one-line>" \
  --body-file /tmp/issue.md     # heredoc; include repro + ground-truth verification
```

- Lead with a reproducible case and a ground-truth check (e.g. `curl …/v1/models` vs.
  the catalog) so the maintainer can confirm without guessing.
- Log every hand-ai ask in `docs/proposals/hand-ai-reasoning-capabilities.md` (the
  running ledger). Examples already filed: #94 (Cerebras catalog drift), #95 (reasoning
  capability metadata).
- HandBox-side processing is limited to disabling the affected model or surfacing a
  clearer error — never add live polling or per-account filtering to compensate.
