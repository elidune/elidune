-- Migration 010: Stats export schema
-- Adds sex + staff fields to users, visitor counting, schedules, equipment, events

-- 1.1 - Sex column on users
ALTER TABLE users ADD COLUMN IF NOT EXISTS sex SMALLINT DEFAULT 85;

-- 1.2 - Staff columns on users
ALTER TABLE users ADD COLUMN IF NOT EXISTS staff_type SMALLINT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS hours_per_week REAL;
ALTER TABLE users ADD COLUMN IF NOT EXISTS staff_start_date DATE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS staff_end_date DATE;

-- 1.3 - Visitor counts table
CREATE TABLE IF NOT EXISTS visitor_counts (
    id SERIAL PRIMARY KEY,
    count_date DATE NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    source VARCHAR(50) DEFAULT 'manual',
    notes VARCHAR,
    crea_date TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_visitor_counts_date ON visitor_counts(count_date);

-- 1.4 - Schedule tables (periods, slots, closures)
CREATE TABLE IF NOT EXISTS schedule_periods (
    id SERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    notes VARCHAR,
    crea_date TIMESTAMPTZ DEFAULT NOW(),
    modif_date TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS schedule_slots (
    id SERIAL PRIMARY KEY,
    period_id INTEGER NOT NULL REFERENCES schedule_periods(id) ON DELETE CASCADE,
    day_of_week SMALLINT NOT NULL CHECK (day_of_week BETWEEN 0 AND 6),
    open_time TIME NOT NULL,
    close_time TIME NOT NULL,
    crea_date TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_schedule_slots_period ON schedule_slots(period_id);

CREATE TABLE IF NOT EXISTS schedule_closures (
    id SERIAL PRIMARY KEY,
    closure_date DATE NOT NULL,
    reason VARCHAR,
    crea_date TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_schedule_closures_date ON schedule_closures(closure_date);

-- 1.5 - Equipment table
CREATE TABLE IF NOT EXISTS equipment (
    id SERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    equipment_type SMALLINT NOT NULL DEFAULT 0,
    has_internet BOOLEAN DEFAULT FALSE,
    is_public BOOLEAN DEFAULT TRUE,
    quantity INTEGER DEFAULT 1,
    status SMALLINT DEFAULT 0,
    notes VARCHAR,
    crea_date TIMESTAMPTZ DEFAULT NOW(),
    modif_date TIMESTAMPTZ
);

-- 1.6 - Events table
CREATE TABLE IF NOT EXISTS events (
    id SERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    event_type SMALLINT NOT NULL DEFAULT 0,
    event_date DATE NOT NULL,
    start_time TIME,
    end_time TIME,
    attendees_count INTEGER DEFAULT 0,
    target_public SMALLINT,
    school_name VARCHAR,
    class_name VARCHAR,
    students_count INTEGER,
    partner_name VARCHAR,
    description VARCHAR,
    notes VARCHAR,
    crea_date TIMESTAMPTZ DEFAULT NOW(),
    modif_date TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_events_date ON events(event_date);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
