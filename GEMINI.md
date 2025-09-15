# Gemini Code Assistant Context

This document provides context for the Gemini code assistant to understand the HandBox project.

## Project Overview

HandBox is a locally-focused, privacy-conscious, all-in-one multi-model AI workbench. It is built using Tauri 2, SvelteKit 5, and TypeScript. The application provides features such as multi-model conversations, reusable chat configurations (Artifacts), MCP integration, history search, and local persistence.

The project is structured as a Tauri application, with a Rust backend and a SvelteKit frontend.

- **Backend (`src-tauri`)**: Written in Rust, it handles business logic, database interactions (via `sqlx` with SQLite), and communication with external AI model APIs (using `reqwest` and a local `openai-rust` library).
- **Frontend (`src`)**: Built with SvelteKit 5 and TypeScript, it provides the user interface for the application.

## Building and Running

The project uses `npm` for package management and running scripts.

- **Development**: To run the application in development mode with hot-reloading, use the following command:
  ```bash
  npm run tauri dev
  ```

- **Building**: To build the application for production, use the following command:
  ```bash
  npm run tauri build
  ```

- **Type Checking**: To check for TypeScript errors, use the following command:
  ```bash
  npm run check
  ```

## Development Conventions

- **Code Style**: The project uses TypeScript for the frontend and Rust for the backend. Adhere to the existing code style and formatting.
- **Testing**: While no testing framework is explicitly configured in `package.json`, the presence of `chat_test.rs` and `message_test.rs` in the `src-tauri/src/models` and `src-tauri/src/services` directories suggests that backend tests are written using Rust's built-in testing framework.
- **Commits**: Follow standard commit message conventions.
- **Contributions**: Contributions are welcome via Pull Requests. Before submitting, ensure that `npm run check` passes.

## Key Files

- `README.md`: The main README file for the project, providing a high-level overview.
- `package.json`: Defines the project's dependencies and scripts.
- `src-tauri/tauri.conf.json`: The main configuration file for the Tauri application.
- `src-tauri/Cargo.toml`: Defines the Rust dependencies for the backend.
- `src/routes`: Contains the SvelteKit routes for the application.
- `src-tauri/src/main.rs`: The main entry point for the Rust backend.
