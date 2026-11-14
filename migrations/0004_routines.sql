-- M5 routine templates and immutable daily-run step snapshots.
CREATE TABLE IF NOT EXISTS routines (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 2000),
    schedule_type TEXT NOT NULL DEFAULT 'daily'
        CHECK (schedule_type IN ('daily', 'weekdays')),
    weekday_mask INTEGER CHECK (weekday_mask IS NULL OR weekday_mask BETWEEN 1 AND 127),
    start_date TEXT NOT NULL CHECK (
        length(start_date) = 10
        AND strftime('%Y-%m-%d', start_date) IS NOT NULL
        AND strftime('%Y-%m-%d', start_date) = start_date
    ),
    end_date TEXT CHECK (
        end_date IS NULL OR (
            length(end_date) = 10
            AND strftime('%Y-%m-%d', end_date) IS NOT NULL
            AND strftime('%Y-%m-%d', end_date) = end_date
        )
    ),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'paused', 'archived')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (end_date IS NULL OR end_date >= start_date),
    CHECK (
        (schedule_type = 'daily' AND weekday_mask IS NULL)
        OR (schedule_type = 'weekdays' AND weekday_mask IS NOT NULL)
    )
);

CREATE TABLE IF NOT EXISTS routine_steps (
    id INTEGER PRIMARY KEY,
    routine_id INTEGER NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 160),
    details TEXT NOT NULL DEFAULT '' CHECK (length(details) <= 1000),
    step_type TEXT NOT NULL DEFAULT 'action'
        CHECK (step_type IN ('action', 'habit')),
    linked_habit_id INTEGER REFERENCES habits(id) ON DELETE RESTRICT,
    is_required INTEGER NOT NULL DEFAULT 1 CHECK (is_required IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (step_type = 'action' AND linked_habit_id IS NULL)
        OR (step_type = 'habit' AND linked_habit_id IS NOT NULL)
    )
);

CREATE TABLE IF NOT EXISTS routine_runs (
    id INTEGER PRIMARY KEY,
    routine_id INTEGER NOT NULL REFERENCES routines(id) ON DELETE RESTRICT,
    local_date TEXT NOT NULL CHECK (
        length(local_date) = 10
        AND strftime('%Y-%m-%d', local_date) IS NOT NULL
        AND strftime('%Y-%m-%d', local_date) = local_date
    ),
    status TEXT NOT NULL DEFAULT 'in_progress'
        CHECK (status IN ('in_progress', 'completed')),
    note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 2000),
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    UNIQUE (routine_id, local_date)
);

-- Run rows snapshot their step title/type/order; step_id is intentionally not a FK.
-- Editing/removing a template step must never rewrite already-started routine history.
CREATE TABLE IF NOT EXISTS routine_run_steps (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES routine_runs(id) ON DELETE CASCADE,
    step_id INTEGER NOT NULL,
    title_snapshot TEXT NOT NULL CHECK (length(trim(title_snapshot)) BETWEEN 1 AND 160),
    step_type_snapshot TEXT NOT NULL CHECK (step_type_snapshot IN ('action', 'habit')),
    linked_habit_id_snapshot INTEGER,
    is_required INTEGER NOT NULL CHECK (is_required IN (0, 1)),
    sort_order INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'completed', 'skipped')),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (run_id, step_id)
);

CREATE INDEX IF NOT EXISTS idx_routines_status_sort
    ON routines(status, sort_order, title);
CREATE INDEX IF NOT EXISTS idx_routine_steps_routine_sort
    ON routine_steps(routine_id, sort_order, id);
CREATE INDEX IF NOT EXISTS idx_routine_runs_routine_date
    ON routine_runs(routine_id, local_date DESC);
CREATE INDEX IF NOT EXISTS idx_routine_run_steps_run_status
    ON routine_run_steps(run_id, status, sort_order);
