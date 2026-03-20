-- Migration 038: Add announcement_sent_at to events table

ALTER TABLE events ADD COLUMN IF NOT EXISTS announcement_sent_at TIMESTAMPTZ;
