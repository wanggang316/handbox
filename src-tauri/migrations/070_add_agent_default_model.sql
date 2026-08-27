-- The model a session starts on, moved from an app-wide setting onto the agent
-- definition.
--
-- Every session is instantiated from a definition (`createSessionFromDefinition`
-- is the only creation path), so the definition is where "what does this run on"
-- belongs: one setting for the whole app could not express "the coding agent
-- runs on a big model, the quick translator on a cheap one".
--
-- Stored as a PAIR, like the setting it replaces: the same model id can exist
-- under several providers, so neither half identifies a model on its own. Both
-- nullable — an agent with no model creates a model-less session and the
-- composer asks, exactly as an unset default did before.
ALTER TABLE agents ADD COLUMN default_model_id TEXT;
ALTER TABLE agents ADD COLUMN default_provider_id TEXT;
