-- Decouple the model from AgentDefinition: the model is now chosen per session
-- (AgentSession.model_id / provider_id), never seeded from the agent. Drop the
-- now-unused `model` column from `agents`. Not indexed, so a plain DROP COLUMN.
ALTER TABLE agents DROP COLUMN model;
