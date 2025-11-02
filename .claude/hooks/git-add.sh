#!/bin/bash
# Git auto-add hook for Claude Code
# Automatically stages all changes before sending a message

set -e

# Get the project directory
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"

cd "$PROJECT_DIR"

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "Not a git repository, skipping git add"
    exit 0
fi

# Add all changes
git add . 2>&1

# Output success message
echo "Changes staged successfully"

exit 0
