-- Add reasoning column to chats table
-- reasoning stores per-chat reasoning/thinking configuration as JSON

ALTER TABLE chats ADD COLUMN reasoning TEXT;
