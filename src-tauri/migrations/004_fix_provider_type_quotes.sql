-- Fix provider_type values that were incorrectly stored with quotes
-- This migration removes the surrounding quotes from provider_type values

UPDATE providers 
SET provider_type = CASE 
    WHEN provider_type LIKE '"%"' THEN SUBSTR(provider_type, 2, LENGTH(provider_type) - 2)
    ELSE provider_type 
END
WHERE provider_type LIKE '"%"';