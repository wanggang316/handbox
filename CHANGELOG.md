# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Removed


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
