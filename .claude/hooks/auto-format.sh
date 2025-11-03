#!/bin/bash
# Auto-format hook for Claude Code
# Runs after Write/Edit tools to ensure code quality

set -e

# Get environment variables from Claude Code
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
TOOL_NAME="${CLAUDE_TOOL_NAME:-}"
TOOL_INPUT="${CLAUDE_TOOL_INPUT:-}"

cd "$PROJECT_DIR"

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    exit 0
fi

# Extract file path from tool input (if available)
FILE_PATH=""
if [ -n "$TOOL_INPUT" ]; then
    # Try to extract file_path from JSON input
    FILE_PATH=$(echo "$TOOL_INPUT" | grep -o '"file_path":"[^"]*"' | cut -d'"' -f4 || echo "")
fi

# Determine what to format based on file extension
format_file() {
    local file="$1"

    if [ ! -f "$file" ]; then
        return 0
    fi

    case "$file" in
        *.rs)
            # Rust files - use cargo fmt
            echo "📝 Formatting Rust file: $file"
            if command -v cargo &> /dev/null; then
                cargo fmt --all -- --check &> /dev/null || cargo fmt --all
                echo "✅ Rust formatted"
            fi
            ;;
        *.ts|*.js|*.svelte|*.json|*.css)
            # Frontend files - use prettier
            echo "📝 Formatting frontend file: $file"
            if command -v npx &> /dev/null; then
                npx prettier --write "$file" &> /dev/null || true
                echo "✅ Frontend formatted"
            fi
            ;;
        *.md)
            # Markdown - use prettier
            if command -v npx &> /dev/null; then
                npx prettier --write "$file" &> /dev/null || true
            fi
            ;;
    esac
}

# Run linters based on file type
lint_file() {
    local file="$1"

    if [ ! -f "$file" ]; then
        return 0
    fi

    case "$file" in
        *.rs)
            # Rust - run clippy on the file's crate
            if command -v cargo &> /dev/null; then
                # Only show warnings, don't fail
                echo "🔍 Checking Rust code..."
                cargo clippy --all-targets --all-features -- -D warnings 2>&1 | head -20 || true
            fi
            ;;
        *.ts|*.svelte)
            # TypeScript - run type check (but don't block)
            if command -v npm &> /dev/null; then
                echo "🔍 Type checking..."
                npm run check 2>&1 | grep -E "(error|warning)" | head -10 || true
            fi
            ;;
    esac
}

# Main logic
if [ -n "$FILE_PATH" ]; then
    # Format specific file
    format_file "$FILE_PATH"

    # Optional: Run linter (non-blocking)
    # lint_file "$FILE_PATH"
else
    # No specific file, check all modified files
    MODIFIED_FILES=$(git diff --name-only --cached 2>/dev/null || git diff --name-only 2>/dev/null || echo "")

    if [ -n "$MODIFIED_FILES" ]; then
        echo "📝 Auto-formatting modified files..."

        # Check if any Rust files were modified
        if echo "$MODIFIED_FILES" | grep -q "\.rs$"; then
            if command -v cargo &> /dev/null; then
                cargo fmt --all &> /dev/null || true
                echo "✅ Rust files formatted"
            fi
        fi

        # Check if any frontend files were modified
        if echo "$MODIFIED_FILES" | grep -qE "\.(ts|js|svelte|json|css)$"; then
            if command -v npx &> /dev/null; then
                echo "$MODIFIED_FILES" | grep -E "\.(ts|js|svelte|json|css)$" | while read -r file; do
                    npx prettier --write "$file" &> /dev/null || true
                done
                echo "✅ Frontend files formatted"
            fi
        fi
    fi
fi

# Optional: Run quick sanity checks
# Uncomment if you want automatic test running (may be slow)
# if [ -n "$FILE_PATH" ] && [[ "$FILE_PATH" == *.rs ]]; then
#     echo "🧪 Running related tests..."
#     cargo test --lib 2>&1 | tail -5 || true
# fi

exit 0
