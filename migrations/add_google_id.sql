-- Add google_id column to events table if it doesn't exist
ALTER TABLE events ADD COLUMN IF NOT EXISTS google_id TEXT;
