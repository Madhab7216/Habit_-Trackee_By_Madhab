-- M5 additive goal milestone storage. Existing goal rows are preserved.
CREATE TABLE IF NOT EXISTS goal_milestones (
    id INTEGER PRIMARY KEY,
    goal_id INTEGER NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 120),
    description TEXT NOT NULL DEFAULT '',
    due_date TEXT CHECK (
        due_date IS NULL OR (
            length(due_date) = 10
            AND substr(due_date, 5, 1) = '-'
            AND substr(due_date, 8, 1) = '-'
        )
    ),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'completed', 'skipped')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_goal_milestones_goal_status_sort
    ON goal_milestones(goal_id, status, sort_order, id);

CREATE INDEX IF NOT EXISTS idx_goal_milestones_due_date
    ON goal_milestones(due_date, status);
