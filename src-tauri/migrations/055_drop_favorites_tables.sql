-- Remove the favorites feature storage.
--
-- The favorites feature (sidebar "收藏" entry, the per-message favorite button,
-- favorited text-range highlights, and external/image captures) has been removed
-- from the app, so these tables are no longer read or written.
--
-- Drop the junction table first (it references both `favorites` and `tags`),
-- then the two base tables. No other table references any of these three, so the
-- drops are self-contained — message/chat persistence is unaffected. Indexes are
-- dropped implicitly with their tables.
DROP TABLE IF EXISTS favorite_tags;
DROP TABLE IF EXISTS favorites;
DROP TABLE IF EXISTS tags;
