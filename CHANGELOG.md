# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Removed


## [0.2.4] - 2026-06-18

### Added
- Switch the interface language between Chinese and English at any time — the
  change applies instantly across the whole app, with no restart.
- Agent sessions have a new "Open in…" button in the header that opens the
  session's working folder in your editor, terminal, or Finder, and remembers
  the app you choose.

### Changed
- Cleaner message input: focusing the Chat or Agent composer no longer shows a
  colored outline, and the divider above the Agent input is gone.
- The slash-command menu supports Ctrl/Cmd+N/P to move through items and keeps
  the highlighted one in view; the skill you pick now appears inline in the
  input as `/name` instead of a chip below it.

### Removed
- The unused Wordbook and Components pages have been removed from Settings.


## [0.2.3] - 2026-06-17

### Added
- Scheduled Jobs: a new Jobs workspace where you can schedule a saved chat,
  agent, or prompt to run automatically on a recurring schedule.
- A schedule editor with quick presets and an advanced cron mode that shows
  the timing in plain language.
- Each job keeps a timeline of its past runs, and you can trigger any job
  immediately with Run now.
- Jobs automatically retry failed runs with a back-off delay, time out runs
  that take too long, and notify you when a job keeps failing.


## [0.2.2] - 2026-06-17

### Added
- Automatic update checks: HandBox now checks for a new version on launch and
  shows a dialog to download and install it. You can turn auto-checking off or
  check manually from the About page.
- A branded splash screen while the app launches.

### Fixed
- Provider logos are now legible in dark theme.


## [0.2.1] - 2026-06-16

### Added
- The agent can now create and edit files and run shell commands, in addition
  to reading, listing, and searching — a full coding toolset.
- Tool approval: before the agent writes a file or runs a command, HandBox pauses
  and asks you to allow it once, always allow it for the session, or deny it.
- Settings → Agent Tools: choose which tools new agent sessions start with.

### Changed
- The agent's file and command tools are now confined to each session's working
  directory.
- The agent composer's per-tool toggles are collapsed into a single tools icon
  with a popover.

### Removed
- The agent's web-page fetch tool.


## [0.2.0] - 2026-06-12

### Added
- Agent mode: a new Agent workspace alongside Chat (toggle in the sidebar) for
  multi-turn agent runs with streaming replies, thinking blocks, and token usage.
- Agent tools — the agent can read files, list directories, and fetch web pages,
  sandboxed to a per-session working directory (web fetches are SSRF-guarded).
- Image attachments in agent messages.
- Mid-run steering: send extra guidance to an agent while it's still running.
- Per-session system prompt editor, opened from the session header.
- Agent sessions grouped by project in the sidebar, with project create / rename /
  delete and zero-dialog session creation from the project header.
- Skills: a slash-command popover in the agent composer, a Settings → Skills page,
  and per-session skill enablement.
- Custom providers: manual model entry and custom-endpoint chat for providers that
  aren't in the catalog.
- Favorites: collapsible tag filter that detects overflow.

### Changed
- Provider catalog is now sourced live from the models.dev catalog with runtime
  refresh, fixing stale / drifting OpenRouter and Cerebras model lists; provider
  icons also come from models.dev instead of bundled assets.
- Reworked the new-chat flow and made session title generation reliable.
- Moved catalog sync off the startup critical path for faster launch.

### Fixed
- API keys are now redacted from debug and log output.

## [0.1.3] - 2026-05-24

### Added
- Release v0.1.3

### Fixed
- Load `llm_config.json` from the Tauri resource directory instead of the current working directory, so packaged builds no longer fall back to an empty config and fail chat requests with `Unknown provider type` (and lose provider icons).

## [0.1.2] - 2026-05-24

### Added
- Linear-inspired design system: dual-mode surface ladder, hairline borders, and a 3-tier radius hierarchy (button 6 / card 8 / panel 12).
- Geist Variable + Geist Mono Variable as the default UI fonts.
- `docs/ui-design.md` capturing the Linear design system and HandBox deviations.

### Changed
- Refactored chat, settings, edge routes (words/selection/agent/favorite), and modals to the new Linear surface ladder.
- Inverted sidebar/main surface roles to match the Linear pattern; main card bleeds to all four window edges and fills the viewport when the sidebar is closed.
- Tightened typography and spacing across sidebar, title bar, message bubbles, and modals; aligned all card radii to `rounded-xl` (12px).
- Switched base inputs and modals to `bg-base-300` inset with hairline borders and removed semibold weight.
- Remapped `@theme` tokens to a Linear-inspired palette with tighter primary contrast and a real blue tint on dark surfaces.

### Fixed
- Layout gap between sidebar and main content card when the sidebar is closed.
- Modal surface lift moved into `Modal.svelte` to remove the redundant wrapper in `AddProviderModal`.
- A11y warnings and dead code surfaced at dev startup.

### Removed
- Dropped the unused `TextSelectionMenu` wrapper from message bubbles.
- Removed Windows from the release matrix.

## [0.1.1] - 2026-05-06

### Added
- System tray (menu bar) icon with Open / Do Something / Quit menu.
- In-app updater wired through `tauri-plugin-updater` with a Settings page check/install flow.
- Release script `scripts/release.sh` and GitHub Actions release workflow.

### Changed
- Replaced local path crate dependencies (`openai-rust`, `google-genai-rust`) with remote git references.
- Hide main window on close instead of destroying it, so the tray Open command can always restore it.

### Fixed
- Corrected misleading error message reporting `OSSEndpoint` when `OSSRegion` was missing from environment.

## [0.1.0] - 2026-05-06

### Added

- Initial baseline release of handbox.
