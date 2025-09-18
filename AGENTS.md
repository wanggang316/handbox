# Repository Guidelines

## Project Structure & Module Organization
The SvelteKit frontend lives in `src/`: UI components in `src/lib/components`, stores in `src/lib/stores`, API helpers in `src/lib/api`, and route groups in `src/routes/(app)` plus `src/routes/settings`. Shared styles stay in `src/app.css`; provider assets live in `static/`. The Tauri backend sits in `src-tauri/`, with IPC entry points in `src/commands`, business logic in `src/services` and `src/storage`, and SQLx migrations in `src-tauri/migrations`. Reference docs are under `docs/`; treat `handbox.db` as disposable dev data.

## Build, Test, and Development Commands
- `npm install` – install SvelteKit and Tauri frontend dependencies.
- `npm run dev` – launch the web preview on Vite with hot module reload.
- `npm run tauri dev` – run the desktop shell (requires `@tauri-apps/cli`).
- `npm run check` – synchronize kit metadata and run `svelte-check` for type and accessibility issues.
- `npm run build` / `npm run preview` – produce and inspect the production SPA bundle.
- `cargo fmt -- --check` & `cargo test` (run in `src-tauri/`) – enforce Rust formatting and execute backend tests.

## Coding Style & Naming Conventions
Use TypeScript everywhere (`lang="ts"` in Svelte blocks) with two-space indentation. Name Svelte components in `PascalCase.svelte`, exports and stores in `camelCase`, and Rust modules/functions in `snake_case`. Prefer Tailwind utility classes for layout; fall back to component `<style>` blocks for bespoke animations. Run `Prettier` and `rustfmt` before committing.

## Testing Guidelines
Frontend contributions must keep `npm run check` clean; add Playwright or Vitest coverage in `src/lib/__tests__` when UI logic grows. Backend specs belong in `src-tauri/tests/` with `mod name_should_do_thing` naming. Run `cargo test -- --ignored` when migrations change storage, and outline manual checks in the PR if automation is not feasible.

## Commit & Pull Request Guidelines
Follow Conventional Commits (`feat:`, `refactor:`, `fix:`) as seen in recent history, and write imperative, present-tense summaries under 70 characters. Each pull request should describe the intent and testing evidence, link to any relevant issue or doc page in `docs/`, and include before/after screenshots or terminal output for user-facing changes.

## Security & Configuration Tips
Never commit provider secrets; keep API keys in the OS keychain or environment, and treat `src-tauri/llm_config.json` as a template, not a source of real credentials. The Rust layer depends on the sibling `openai-rust` crate—verify the relative path before builds. Redact `tracing` logs before sharing externally.
