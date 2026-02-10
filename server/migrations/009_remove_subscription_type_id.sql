-- Migration 009: Remove subscription_type_id column from users table
-- This migration removes the subscription_type_id column which is no longer used

-- =============================================================================
-- REMOVE SUBSCRIPTION_TYPE_ID FROM USERS
-- =============================================================================

-- Step 1: Drop subscription_type_id column from users
ALTER TABLE users DROP COLUMN IF EXISTS subscription_type_id;
