-- Migration: Add 2FA fields to users table
-- Created: 2024

-- Add 2FA fields to users table
ALTER TABLE users
ADD COLUMN IF NOT EXISTS two_factor_enabled BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS two_factor_method VARCHAR(10) CHECK (two_factor_method IN ('totp', 'email', NULL)),
ADD COLUMN IF NOT EXISTS totp_secret VARCHAR(255),
ADD COLUMN IF NOT EXISTS recovery_codes TEXT,
ADD COLUMN IF NOT EXISTS recovery_codes_used TEXT DEFAULT '[]';

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS idx_users_two_factor_enabled ON users(two_factor_enabled);
