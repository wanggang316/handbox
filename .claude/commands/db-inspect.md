---
description: Inspect HandBox SQLite database for debugging
argument-hint: [table_name] [query]
---

Inspect the HandBox database located at:
- macOS: `~/Library/Application Support/com.gumpw.handbox/handbox.db`
- Linux: `~/.local/share/com.gumpw.handbox/handbox.db`
- Windows: `%APPDATA%/com.gumpw.handbox/handbox.db`

Query: $ARGUMENTS

Please help me investigate the database:

1. **If no arguments**: Show all tables and basic statistics
   ```bash
   sqlite3 "$DB_PATH" ".tables"
   sqlite3 "$DB_PATH" "SELECT name, (SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=m.name) as count FROM sqlite_master m WHERE type='table';"
   ```

2. **If table name provided**: Show table schema and sample data
   ```bash
   sqlite3 "$DB_PATH" ".schema $1"
   sqlite3 "$DB_PATH" -header -box "SELECT * FROM $1 LIMIT 10;"
   ```

3. **If query provided**: Execute the SQL query (read-only)
   ```bash
   sqlite3 "$DB_PATH" -header -box "$ARGUMENTS"
   ```

**Common queries**:
- Check migrations: `SELECT * FROM _sqlx_migrations ORDER BY version DESC LIMIT 10`
- Check foreign keys: `PRAGMA foreign_key_check;`
- Count records: `SELECT COUNT(*) FROM <table>`
- Show table info: `PRAGMA table_info(<table>);`

For complex database investigations, consider using the `database-inspector` subagent for detailed analysis.
