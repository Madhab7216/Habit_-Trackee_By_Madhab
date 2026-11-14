-- Initial local-first schema. Apply in a transaction and record version 1.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) BETWEEN 1 AND 80),
    color TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS habits (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '',
    tracking_type TEXT NOT NULL DEFAULT 'binary'
        CHECK (tracking_type IN ('binary', 'numeric', 'duration', 'count', 'limit', 'frequency')),
    direction TEXT NOT NULL DEFAULT 'at_least'
        CHECK (direction IN ('at_least', 'at_most')),
    target_value REAL CHECK (target_value IS NULL OR target_value >= 0),
    unit TEXT,
    schedule_type TEXT NOT NULL DEFAULT 'daily'
        CHECK (schedule_type IN ('daily', 'weekdays', 'every_n_days', 'times_per_week')),
    weekday_mask INTEGER CHECK (weekday_mask IS NULL OR weekday_mask BETWEEN 1 AND 127),
    interval_days INTEGER CHECK (interval_days IS NULL OR interval_days >= 1),
    target_per_period INTEGER CHECK (target_per_period IS NULL OR target_per_period >= 1),
    period_unit TEXT NOT NULL DEFAULT 'week' CHECK (period_unit IN ('day', 'week', 'month')),
    time_zone_id TEXT NOT NULL DEFAULT 'local',
    start_date TEXT NOT NULL DEFAULT (date('now', 'localtime')),
    end_date TEXT,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    archived_at TEXT,
    CHECK (end_date IS NULL OR end_date >= start_date),
    CHECK (schedule_type != 'weekdays' OR weekday_mask IS NOT NULL),
    CHECK (schedule_type != 'every_n_days' OR interval_days IS NOT NULL),
    CHECK (schedule_type != 'times_per_week' OR (target_per_period IS NOT NULL AND period_unit = 'week'))
);

CREATE TABLE IF NOT EXISTS habit_entries (
    id INTEGER PRIMARY KEY,
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE RESTRICT,
    local_date TEXT NOT NULL CHECK (date(local_date) IS NOT NULL AND date(local_date) = local_date),
    status TEXT NOT NULL CHECK (status IN ('completed', 'partial', 'skipped')),
    value REAL CHECK (value IS NULL OR value >= 0),
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (habit_id, local_date)
);

CREATE TABLE IF NOT EXISTS habit_tags (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) BETWEEN 1 AND 40),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS habit_tag_links (
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES habit_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (habit_id, tag_id)
);

CREATE TABLE IF NOT EXISTS goals (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '',
    target_date TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed', 'paused', 'archived')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS goal_habits (
    goal_id INTEGER NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE RESTRICT,
    PRIMARY KEY (goal_id, habit_id)
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_habits_active_sort ON habits(archived_at, sort_order, name);
CREATE INDEX IF NOT EXISTS idx_habit_entries_habit_date ON habit_entries(habit_id, local_date);
CREATE INDEX IF NOT EXISTS idx_habit_entries_date_status ON habit_entries(local_date, status);
CREATE INDEX IF NOT EXISTS idx_goals_status_date ON goals(status, target_date);
