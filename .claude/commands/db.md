# Database Query Command

Quick database inspection commands for HandBox SQLite database.

## Description

Fast access to common database queries without typing full sqlite3 commands. Useful for debugging and development.

## Usage

### Show tables
```
/db tables
```

### Show table schema
```
/db schema <table_name>
```

### Query table data
```
/db query <table_name> [limit]
```

### Check migrations
```
/db migrations
```

### Foreign key check
```
/db fk-check [table_name]
```

### Count records
```
/db count <table_name>
```

### Integrity check
```
/db check
```

### Custom SQL query
```
/db sql "SELECT * FROM models WHERE enabled = 1"
```

## Implementation

{{args}}

```bash
# Database path (adjust for OS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    DB_PATH="$HOME/Library/Application Support/com.gumpw.handbox/handbox.db"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    DB_PATH="$HOME/.local/share/com.gumpw.handbox/handbox.db"
else
    DB_PATH="$APPDATA/com.gumpw.handbox/handbox.db"
fi

# Check if database exists
if [ ! -f "$DB_PATH" ]; then
    echo "❌ Database not found at: $DB_PATH"
    echo ""
    echo "Make sure HandBox has been run at least once."
    exit 1
fi

# Parse command
SUBCOMMAND="${1:-help}"
ARG1="${2:-}"
ARG2="${3:-}"

# Execute based on subcommand
case "$SUBCOMMAND" in
    tables)
        echo "## HandBox Database Tables"
        echo ""
        sqlite3 "$DB_PATH" ".tables"
        echo ""
        echo "Use '/db schema <table_name>' to see table structure"
        ;;

    schema)
        if [ -z "$ARG1" ]; then
            echo "Error: Table name required"
            echo "Usage: /db schema <table_name>"
            exit 1
        fi

        echo "## Schema for table: $ARG1"
        echo ""
        sqlite3 "$DB_PATH" ".schema $ARG1"
        echo ""
        echo "## Column Info"
        sqlite3 "$DB_PATH" -header -column "PRAGMA table_info($ARG1);"
        ;;

    query)
        if [ -z "$ARG1" ]; then
            echo "Error: Table name required"
            echo "Usage: /db query <table_name> [limit]"
            exit 1
        fi

        LIMIT="${ARG2:-10}"

        echo "## Data from table: $ARG1 (limit: $LIMIT)"
        echo ""
        sqlite3 "$DB_PATH" -header -box "SELECT * FROM $ARG1 LIMIT $LIMIT;"
        echo ""

        # Show total count
        TOTAL=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM $ARG1;")
        echo "Total records: $TOTAL"
        ;;

    count)
        if [ -z "$ARG1" ]; then
            echo "Error: Table name required"
            echo "Usage: /db count <table_name>"
            exit 1
        fi

        TOTAL=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM $ARG1;")
        echo "Table '$ARG1' has $TOTAL records"
        ;;

    migrations)
        echo "## Migration History"
        echo ""
        sqlite3 "$DB_PATH" -header -box \
            "SELECT version, description, installed_on, success
             FROM _sqlx_migrations
             ORDER BY version DESC
             LIMIT 20;"
        echo ""

        # Show total
        TOTAL=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM _sqlx_migrations;")
        echo "Total migrations: $TOTAL"

        # Show any failed
        FAILED=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 0;")
        if [ "$FAILED" -gt 0 ]; then
            echo ""
            echo "⚠️ Failed migrations: $FAILED"
            sqlite3 "$DB_PATH" -header -box \
                "SELECT * FROM _sqlx_migrations WHERE success = 0;"
        fi
        ;;

    fk-check)
        echo "## Foreign Key Check"
        echo ""

        if [ -n "$ARG1" ]; then
            # Check specific table
            echo "Checking table: $ARG1"
            echo ""
            RESULT=$(sqlite3 "$DB_PATH" "PRAGMA foreign_key_check($ARG1);")
        else
            # Check all tables
            echo "Checking all tables..."
            echo ""
            RESULT=$(sqlite3 "$DB_PATH" "PRAGMA foreign_key_check;")
        fi

        if [ -z "$RESULT" ]; then
            echo "✅ No foreign key violations found"
        else
            echo "❌ Foreign key violations detected:"
            echo ""
            echo "$RESULT"
        fi

        echo ""
        echo "Foreign keys enabled: $(sqlite3 "$DB_PATH" "PRAGMA foreign_keys;")"
        ;;

    check)
        echo "## Database Integrity Check"
        echo ""

        echo "Running quick check..."
        QUICK_CHECK=$(sqlite3 "$DB_PATH" "PRAGMA quick_check;")

        if [ "$QUICK_CHECK" = "ok" ]; then
            echo "✅ Quick check: OK"
        else
            echo "❌ Quick check failed:"
            echo "$QUICK_CHECK"
        fi

        echo ""
        echo "Running full integrity check..."
        INTEGRITY=$(sqlite3 "$DB_PATH" "PRAGMA integrity_check;")

        if [ "$INTEGRITY" = "ok" ]; then
            echo "✅ Integrity check: OK"
        else
            echo "❌ Integrity issues found:"
            echo "$INTEGRITY"
        fi
        ;;

    sql)
        if [ -z "$ARG1" ]; then
            echo "Error: SQL query required"
            echo "Usage: /db sql \"SELECT * FROM table\""
            exit 1
        fi

        echo "## Running custom query"
        echo ""
        echo "Query: $ARG1"
        echo ""

        sqlite3 "$DB_PATH" -header -box "$ARG1"
        ;;

    stats)
        echo "## Database Statistics"
        echo ""

        # File size
        if [[ "$OSTYPE" == "darwin"* ]]; then
            SIZE=$(ls -lh "$DB_PATH" | awk '{print $5}')
        else
            SIZE=$(du -h "$DB_PATH" | cut -f1)
        fi
        echo "Database size: $SIZE"
        echo ""

        # Table counts
        echo "## Record Counts"
        echo ""
        sqlite3 "$DB_PATH" -header -column \
            "SELECT
                'providers' as table_name,
                (SELECT COUNT(*) FROM providers) as count
             UNION ALL
             SELECT 'models', (SELECT COUNT(*) FROM models)
             UNION ALL
             SELECT 'chats', (SELECT COUNT(*) FROM chats)
             UNION ALL
             SELECT 'messages', (SELECT COUNT(*) FROM messages)
             UNION ALL
             SELECT 'artifacts', (SELECT COUNT(*) FROM artifacts)
             UNION ALL
             SELECT 'mcp_servers', (SELECT COUNT(*) FROM mcp_servers);"
        ;;

    help|*)
        echo "Database Query Command - Quick access to HandBox SQLite database"
        echo ""
        echo "Usage: /db <command> [args]"
        echo ""
        echo "Commands:"
        echo "  tables              - List all tables"
        echo "  schema <table>      - Show table schema and columns"
        echo "  query <table> [N]   - Query table (default limit 10)"
        echo "  count <table>       - Count records in table"
        echo "  migrations          - Show migration history"
        echo "  fk-check [table]    - Check foreign key constraints"
        echo "  check               - Run integrity checks"
        echo "  sql \"query\"         - Execute custom SQL"
        echo "  stats               - Show database statistics"
        echo ""
        echo "Examples:"
        echo "  /db tables"
        echo "  /db schema models"
        echo "  /db query providers"
        echo "  /db count models"
        echo "  /db migrations"
        echo "  /db fk-check models"
        echo "  /db sql \"SELECT name FROM models WHERE enabled = 1\""
        echo ""
        echo "Database: $DB_PATH"
        ;;
esac
```

## Examples

### Common Workflows

**Debug foreign key error:**
```bash
# 1. Check which table has the issue
/db fk-check

# 2. Look at the table structure
/db schema models

# 3. Check if referenced data exists
/db query providers

# 4. See recent migrations
/db migrations
```

**Verify migration:**
```bash
# 1. Check migration was applied
/db migrations

# 2. Verify column exists
/db schema models

# 3. Check data
/db query models 5
```

**Find data issues:**
```bash
# 1. Check integrity
/db check

# 2. Look at table counts
/db stats

# 3. Query specific data
/db sql "SELECT * FROM models WHERE supported_parameters IS NULL"
```

**Inspect relationships:**
```bash
# 1. See model and provider relationship
/db sql "SELECT m.name as model, p.name as provider
         FROM models m JOIN providers p ON m.provider_id = p.id
         LIMIT 10"

# 2. Count models per provider
/db sql "SELECT p.name, COUNT(m.id) as count
         FROM providers p LEFT JOIN models m ON p.id = m.provider_id
         GROUP BY p.id"
```

## Integration with Subagent

This command provides quick access for simple queries. For complex database investigations, use the `database-inspector` subagent:

```
You: "调查为什么模型插入失败"

Claude: [Uses database-inspector subagent]
- Checks schema
- Verifies foreign keys
- Analyzes migration history
- Provides detailed report
```

**Use /db when:**
- Quick schema check
- Verify table exists
- Count records
- Simple data query

**Use database-inspector subagent when:**
- Debugging complex issues
- Need detailed analysis
- Multiple related queries
- Root cause investigation

## Notes

- Read-only by default (no UPDATE/DELETE)
- Database path auto-detected by OS
- Uses box format for pretty output
- Limited to 10 records by default (override with limit parameter)
- For complex queries, use `/db sql "..."`

## Safety

All queries are read-only SELECT statements. To modify data:
1. Create a migration (recommended)
2. Or manually with sqlite3 (backup first!)

```bash
# Backup first
cp "$DB_PATH" "$DB_PATH.backup"

# Then modify
sqlite3 "$DB_PATH" "UPDATE ..."
```
