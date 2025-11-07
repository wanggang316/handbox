---
name: sqlite-query
description: Query HandBox SQLite database located in Tauri sandbox. Use when user asks to check database contents, query chats, messages, models, providers, or any database-related information. Requires sqlite3 command-line tool.
allowed-tools: Bash
---

# SQLite Database Query Skill

This skill helps query the HandBox SQLite database stored in the Tauri application sandbox directory.

## Database Location

The HandBox database is located at:
```
~/Library/Application Support/com.handbox.app/handbox.db
```

## Common Queries

### List all tables
```bash
sqlite3 ~/Library/Application\ Support/com.handbox.app/handbox.db ".tables"
```

### Show table schema
```bash
sqlite3 ~/Library/Application\ Support/com.handbox.app/handbox.db ".schema TABLE_NAME"
```

### Query data
```bash
sqlite3 ~/Library/Application\ Support/com.handbox.app/handbox.db "SELECT * FROM table_name LIMIT 10"
```

## Instructions

When the user asks to query the database:

1. **Determine the query type**:
   - List tables: Use `.tables`
   - Show schema: Use `.schema TABLE_NAME`
   - Query data: Use appropriate SELECT statement
   - Count records: Use `SELECT COUNT(*) FROM table_name`

2. **Format the output**:
   - Use `-header` flag to show column headers
   - Use `-column` mode for readable output
   - Use `-json` mode for JSON output when appropriate

3. **Common table structures**:
   - **chats**: id, name, temperature, top_p, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers, turn_count, created_at, updated_at
   - **messages**: id, chat_id, role, content, reasoning, config, tools, attachments, input_tokens, output_tokens, total_tokens, created_at, updated_at
   - **models**: id, provider_id, name, context_length, output_max_tokens, supported_features, supported_parameters, default_parameters, max_parameters, enabled, favorite
   - **providers**: id, name, provider_type, base_url, api_key, enabled, created_at, updated_at

4. **Safety guidelines**:
   - Never modify the database (no INSERT, UPDATE, DELETE)
   - Use read-only queries only
   - Limit results to reasonable numbers (e.g., LIMIT 50)
   - Handle sensitive data carefully (e.g., api_key should be masked)

## Examples

### Example 1: List all chats
```bash
sqlite3 -header -column ~/Library/Application\ Support/com.handbox.app/handbox.db \
  "SELECT id, name, model_id, created_at FROM chats ORDER BY updated_at DESC LIMIT 10"
```

### Example 2: Check messages in a specific chat
```bash
sqlite3 -header -column ~/Library/Application\ Support/com.handbox.app/handbox.db \
  "SELECT role, substr(content, 1, 50) as content_preview, created_at
   FROM messages
   WHERE chat_id = 'CHAT_ID'
   ORDER BY created_at DESC"
```

### Example 3: View message config
```bash
sqlite3 -json ~/Library/Application\ Support/com.handbox.app/handbox.db \
  "SELECT id, config FROM messages WHERE config IS NOT NULL LIMIT 5"
```

### Example 4: Check enabled models
```bash
sqlite3 -header -column ~/Library/Application\ Support/com.handbox.app/handbox.db \
  "SELECT provider_id, name, context_length, output_max_tokens
   FROM models
   WHERE enabled = 1
   ORDER BY provider_id, name"
```

### Example 5: Count messages by role
```bash
sqlite3 -header -column ~/Library/Application\ Support/com.handbox.app/handbox.db \
  "SELECT role, COUNT(*) as count
   FROM messages
   GROUP BY role"
```

## Tips

- Always quote the database path if it contains spaces
- Use LIMIT to prevent large result sets
- Use `substr()` for long text fields to keep output readable
- JSON mode (`-json`) is useful for structured data analysis
- Use `\n` in queries for better readability in multi-line SQL
