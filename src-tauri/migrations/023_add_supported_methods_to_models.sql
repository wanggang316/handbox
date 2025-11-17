-- Add supported_methods column to models table
-- This field stores the supported methods/endpoints for each model
-- Format: JSON string array, e.g., ["openai_chat_completions", "google_generate_content"]
ALTER TABLE models
ADD COLUMN supported_methods TEXT;
