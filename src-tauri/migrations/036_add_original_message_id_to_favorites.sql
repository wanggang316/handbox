-- 添加 original_message_id 字段到 favorites 表
ALTER TABLE favorites ADD COLUMN original_message_id TEXT;
