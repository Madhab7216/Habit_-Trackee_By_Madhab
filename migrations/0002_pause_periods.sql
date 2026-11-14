-- Pause periods are retained as history. ends_on is an exclusive local-date boundary:
-- a habit resumed on a date is active again on that date.
CREATE TABLE IF NOT EXISTS habit_pause_periods (
    id INTEGER PRIMARY KEY,
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE RESTRICT,
    starts_on TEXT NOT NULL
        CHECK (date(starts_on) IS NOT NULL AND date(starts_on) = starts_on),
    ends_on TEXT
        CHECK (
            ends_on IS NULL OR (
                date(ends_on) IS NOT NULL
                AND date(ends_on) = ends_on
                AND ends_on >= starts_on
            )
        ),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- A habit may have many historical pause periods, but only one open pause.
CREATE UNIQUE INDEX IF NOT EXISTS idx_habit_one_open_pause
    ON habit_pause_periods(habit_id)
    WHERE ends_on IS NULL;

CREATE INDEX IF NOT EXISTS idx_habit_pause_periods_dates
    ON habit_pause_periods(habit_id, starts_on, ends_on);
