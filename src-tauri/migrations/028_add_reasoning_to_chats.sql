-- Add reasoning and supported_parameters columns to chats table
-- reasoning stores per-chat reasoning/thinking configuration as JSON
-- supported_parameters caches the model's supported parameter list for availability checks

ALTER TABLE chats ADD COLUMN reasoning TEXT;
ALTER TABLE chats ADD COLUMN supported_parameters TEXT;

-- Backfill supported_parameters from models table when possible so existing chats inherit model capabilities
UPDATE chats
SET supported_parameters = (
    SELECT supported_parameters
    FROM models
    WHERE models.id = chats.model_id
      AND (models.provider_id = chats.provider_id OR chats.provider_id IS NULL)
)
WHERE supported_parameters IS NULL
  AND model_id IS NOT NULL;
