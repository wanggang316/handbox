-- 重构收藏数据结构
-- 1. 添加 context 字段
ALTER TABLE favorites ADD COLUMN context TEXT;

-- 2. 删除不需要的字段
ALTER TABLE favorites DROP COLUMN selected_text;
ALTER TABLE favorites DROP COLUMN title;
ALTER TABLE favorites DROP COLUMN original_message_id;
