---
description: Run Rust backend tests
argument-hint: [test_name] [--verbose]
---

Run Rust tests for the HandBox backend.

Test target: $ARGUMENTS

Please run tests in the `src-tauri/` directory:

1. **If no arguments**: Run all tests
   ```bash
   cd src-tauri && cargo test
   ```

2. **If test name provided**: Run specific test
   ```bash
   cd src-tauri && cargo test $1
   ```

3. **If --verbose flag**: Show test output
   ```bash
   cd src-tauri && cargo test $1 -- --nocapture
   ```

4. **Show results**: Report pass/fail status clearly

**Examples**:
- `/test-run` - Run all tests
- `/test-run test_send_message` - Run specific test
- `/test-run services::chat` - Run module tests
- `/test-run --verbose` - Run with output

For test coverage analysis, use the `rust-test-writer` subagent.
