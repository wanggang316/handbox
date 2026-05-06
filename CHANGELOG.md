# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- System tray (menu bar) icon with Open / Do Something / Quit menu.
- In-app updater wired through `tauri-plugin-updater` with a Settings page check/install flow.
- Release script `scripts/release.sh` and GitHub Actions release workflow.

### Changed

- Replaced local path crate dependencies (`openai-rust`, `google-genai-rust`) with remote git references.
- Hide main window on close instead of destroying it, so the tray Open command can always restore it.

### Fixed

- Corrected misleading error message reporting `OSSEndpoint` when `OSSRegion` was missing from environment.

### Removed

## [0.1.0] - 2026-05-06

### Added

- Initial baseline release of handbox.
