# Test Command

Quick test execution and coverage commands for HandBox Rust backend.

## Description

Fast access to common testing operations without typing full cargo commands. Supports running tests, checking coverage, and filtering test execution.

## Usage

### Run all tests
```
/test
/test all
```

### Run specific test
```
/test <test_name>
```

### Run tests in a module
```
/test mod <module_path>
```

### Run tests with output
```
/test verbose
/test <test_name> verbose
```

### Run coverage report
```
/test coverage
/test cov
```

### Run specific test file
```
/test file <file_path>
```

### Watch mode (run tests on file changes)
```
/test watch
```

## Implementation

{{args}}

```bash
# Change to Tauri directory
cd src-tauri

# Parse command
SUBCOMMAND="${1:-all}"
ARG1="${2:-}"
ARG2="${3:-}"

# Color output helpers
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

case "$SUBCOMMAND" in
    all|"")
        echo -e "${BLUE}## Running All Tests${NC}"
        echo ""
        cargo test

        if [ $? -eq 0 ]; then
            echo ""
            echo -e "${GREEN}✅ All tests passed!${NC}"
        else
            echo ""
            echo -e "${RED}❌ Some tests failed${NC}"
            exit 1
        fi
        ;;

    verbose|v)
        echo -e "${BLUE}## Running Tests (Verbose)${NC}"
        echo ""
        cargo test -- --nocapture --test-threads=1
        ;;

    coverage|cov)
        echo -e "${BLUE}## Generating Coverage Report${NC}"
        echo ""

        # Check if cargo-tarpaulin is installed
        if ! command -v cargo-tarpaulin &> /dev/null; then
            echo -e "${YELLOW}Installing cargo-tarpaulin...${NC}"
            cargo install cargo-tarpaulin
        fi

        echo "Running tests with coverage..."
        echo ""

        # Generate coverage
        cargo tarpaulin --out Html --output-dir ../coverage

        if [ $? -eq 0 ]; then
            echo ""
            echo -e "${GREEN}✅ Coverage report generated!${NC}"
            echo ""
            echo "Open: coverage/index.html"
            echo ""

            # Show summary
            cargo tarpaulin --out Stdout | tail -10
        else
            echo -e "${RED}❌ Coverage generation failed${NC}"
            exit 1
        fi
        ;;

    mod|module)
        if [ -z "$ARG1" ]; then
            echo -e "${RED}Error: Module path required${NC}"
            echo "Usage: /test mod <module_path>"
            echo ""
            echo "Examples:"
            echo "  /test mod services::chat"
            echo "  /test mod commands"
            exit 1
        fi

        echo -e "${BLUE}## Running Tests in Module: $ARG1${NC}"
        echo ""
        cargo test "$ARG1"
        ;;

    file|f)
        if [ -z "$ARG1" ]; then
            echo -e "${RED}Error: File path required${NC}"
            echo "Usage: /test file <file_path>"
            echo ""
            echo "Example:"
            echo "  /test file tests/chat_service_tests.rs"
            exit 1
        fi

        # Extract test name from file path
        TEST_NAME=$(basename "$ARG1" .rs)

        echo -e "${BLUE}## Running Tests in File: $ARG1${NC}"
        echo ""
        cargo test --test "$TEST_NAME"
        ;;

    watch|w)
        echo -e "${BLUE}## Watch Mode - Tests will re-run on file changes${NC}"
        echo ""

        # Check if cargo-watch is installed
        if ! command -v cargo-watch &> /dev/null; then
            echo -e "${YELLOW}Installing cargo-watch...${NC}"
            cargo install cargo-watch
        fi

        echo "Watching for changes... (Ctrl+C to stop)"
        echo ""
        cargo watch -x test
        ;;

    quick|q)
        echo -e "${BLUE}## Quick Test (lib only, no integration)${NC}"
        echo ""
        cargo test --lib
        ;;

    integration|i)
        echo -e "${BLUE}## Running Integration Tests${NC}"
        echo ""
        cargo test --test '*'
        ;;

    unit|u)
        echo -e "${BLUE}## Running Unit Tests${NC}"
        echo ""
        cargo test --lib
        ;;

    failed|f)
        echo -e "${BLUE}## Re-running Failed Tests${NC}"
        echo ""
        cargo test -- --failed
        ;;

    bench|b)
        echo -e "${BLUE}## Running Benchmarks${NC}"
        echo ""
        cargo bench
        ;;

    doc)
        echo -e "${BLUE}## Running Doc Tests${NC}"
        echo ""
        cargo test --doc
        ;;

    clean)
        echo -e "${BLUE}## Cleaning Test Artifacts${NC}"
        echo ""
        cargo clean
        rm -rf ../coverage
        echo -e "${GREEN}✅ Cleaned${NC}"
        ;;

    help|h)
        echo "Test Command - Quick test execution for HandBox"
        echo ""
        echo "Usage: /test [command] [args]"
        echo ""
        echo "Commands:"
        echo "  all              - Run all tests (default)"
        echo "  <test_name>      - Run specific test"
        echo "  verbose          - Run with output (--nocapture)"
        echo "  coverage         - Generate HTML coverage report"
        echo "  mod <path>       - Run tests in module"
        echo "  file <path>      - Run tests in file"
        echo "  watch            - Watch mode (re-run on changes)"
        echo "  quick            - Quick test (lib only)"
        echo "  integration      - Integration tests only"
        echo "  unit             - Unit tests only"
        echo "  failed           - Re-run failed tests"
        echo "  bench            - Run benchmarks"
        echo "  doc              - Run doc tests"
        echo "  clean            - Clean test artifacts"
        echo ""
        echo "Examples:"
        echo "  /test"
        echo "  /test coverage"
        echo "  /test test_send_message"
        echo "  /test mod services::chat"
        echo "  /test verbose"
        echo "  /test watch"
        echo ""
        echo "Coverage Reports:"
        echo "  Generated in: coverage/index.html"
        echo "  Target: ≥ 80% coverage"
        ;;

    *)
        # Treat as specific test name
        if [ "$SUBCOMMAND" = "verbose" ] || [ "$ARG1" = "verbose" ]; then
            echo -e "${BLUE}## Running Test: $SUBCOMMAND (Verbose)${NC}"
            echo ""
            cargo test "$SUBCOMMAND" -- --nocapture
        else
            echo -e "${BLUE}## Running Test: $SUBCOMMAND${NC}"
            echo ""
            cargo test "$SUBCOMMAND"
        fi
        ;;
esac

# Return to original directory
cd ..
```

## Examples

### Basic Testing Workflows

**Run all tests:**
```bash
/test
```

**Run specific test:**
```bash
/test test_send_message
/test test_validate_provider_name
```

**Run with output (see println! statements):**
```bash
/test verbose
/test test_send_message verbose
```

### Module Testing

**Test entire module:**
```bash
/test mod services
/test mod commands::chat
/test mod storage::model_repository
```

### Coverage Workflows

**Generate coverage report:**
```bash
/test coverage

# Opens coverage/index.html
# Shows line-by-line coverage
# Identifies untested code
```

**Quick coverage check:**
```bash
/test coverage | grep "Total:"
# Example output: Total: 75/90 (83.33%)
```

### Development Workflows

**Watch mode (TDD):**
```bash
/test watch

# Tests re-run automatically when files change
# Great for TDD workflow
```

**Quick iteration:**
```bash
# 1. Write test
/test test_new_feature

# 2. Watch it fail (Red)
# 3. Implement feature
# 4. Watch it pass (Green)
# 5. Refactor

/test test_new_feature verbose  # See detailed output
```

### Targeted Testing

**Run only unit tests:**
```bash
/test unit
```

**Run only integration tests:**
```bash
/test integration
```

**Re-run failed tests:**
```bash
/test failed
```

### Coverage-Driven Development

**1. Check current coverage:**
```bash
/test coverage
```

**2. Identify gaps (open coverage/index.html):**
- Red lines = not covered
- Need tests for those paths

**3. Write tests for uncovered code:**
```bash
/test test_uncovered_function verbose
```

**4. Verify coverage improved:**
```bash
/test coverage
```

## Integration with rust-test-writer Subagent

### When to Use Command vs Subagent

**Use `/test` command when:**
- ✅ Running existing tests
- ✅ Checking coverage quickly
- ✅ Debugging test failures
- ✅ TDD iteration (write → run → refactor)

**Use `rust-test-writer` subagent when:**
- ✅ Writing new tests
- ✅ Improving test coverage
- ✅ Need test patterns/examples
- ✅ Comprehensive test suite creation

### Combined Workflow

```
1. User: "为 chat_service 编写单元测试"

2. Claude: [使用 rust-test-writer subagent]
   - 分析 chat_service.rs 代码
   - 识别需要测试的函数
   - 编写全面的测试套件
   - 包含成功路径、错误处理、边界情况

3. User: "运行测试看看"

4. Claude: /test mod services::chat

5. User: "检查覆盖率"

6. Claude: /test coverage
   报告: chat_service.rs: 85/90 (94.4%) ✅

7. User: "还有哪些没覆盖？"

8. Claude: [分析 coverage/index.html]
   - 第 45-47 行未覆盖（错误处理分支）
   - 建议添加测试

9. Claude: [使用 rust-test-writer]
   - 添加缺失的测试

10. User: "再跑一次测试"

11. Claude: /test coverage
    报告: chat_service.rs: 90/90 (100%) ✅✅
```

## Coverage Goals for HandBox

### Target Coverage by Component

```
✅ Critical Security Code: 100%
   - API key storage/retrieval
   - Input validation
   - Authentication

✅ Business Logic: ≥ 90%
   - Chat services
   - Model management
   - Provider operations

✅ Database Layer: ≥ 85%
   - Repositories
   - Migrations (manual testing)

✅ Commands (IPC): ≥ 80%
   - Command handlers
   - State management

✅ Utilities: ≥ 75%
   - Helper functions
   - Formatters
```

### Monitoring Coverage

```bash
# Quick check
/test coverage | grep -A 5 "Total:"

# Full report
/test coverage
open coverage/index.html

# Per-file breakdown
/test coverage | grep "src/"
```

## Tips for Fast Testing

### Speed Up Tests

**1. Run only what changed:**
```bash
# After editing chat_service.rs
/test mod services::chat
```

**2. Use quick tests during development:**
```bash
# Skip integration tests
/test quick
```

**3. Run specific test:**
```bash
# Instead of all tests
/test test_specific_function
```

**4. Use watch mode:**
```bash
# Auto-run on save
/test watch
```

### Parallel Execution

```bash
# Tests run in parallel by default
# To control threads:
cargo test -- --test-threads=4
```

## Debugging Failed Tests

### Show Output

```bash
# See println! and dbg! output
/test test_name verbose
```

### Run Single Test

```bash
# Focus on one failing test
/test test_that_fails verbose
```

### Check Test Code

```bash
# Read the test implementation
grep -A 20 "fn test_that_fails" src/**/*.rs
```

## Continuous Integration

### Pre-commit Checks

```bash
# Run before committing
/test
/test coverage

# Ensure ≥ 80% coverage
# Fix any failures
```

### CI Pipeline Commands

```bash
# In GitHub Actions / CI
cargo test --all-features
cargo tarpaulin --out Xml
```

## Notes

- All commands run from project root
- Coverage reports saved to `coverage/`
- Watch mode requires `cargo-watch`
- Coverage requires `cargo-tarpaulin`
- Tests run in `src-tauri/` directory

## Remember

- **Test First** - Write tests before implementation (TDD)
- **Fast Feedback** - Use `/test watch` for rapid iteration
- **Coverage Goal** - Maintain ≥ 80% coverage
- **Green Build** - All tests must pass before committing
- **Test Quality** - Coverage number is guide, not goal itself

Good tests are:
- ✅ Fast (< 1s each)
- ✅ Isolated (no shared state)
- ✅ Deterministic (same result every time)
- ✅ Readable (clear intent)
- ✅ Maintainable (easy to update)
