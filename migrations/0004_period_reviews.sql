-- M5 local weekly/monthly reflection notes. This uses its own migration ledger
-- so it remains additive and does not change the core habit schema version.
CREATE TABLE IF NOT EXISTS period_reviews (
    id INTEGER PRIMARY KEY,
    period_kind TEXT NOT NULL CHECK (period_kind IN ('week', 'month')),
    period_start TEXT NOT NULL CHECK (
        length(period_start) = 10
        AND date(period_start) IS NOT NULL
        AND date(period_start) = period_start
    ),
    went_well TEXT NOT NULL DEFAULT '' CHECK (length(went_well) <= 8000),
    challenges TEXT NOT NULL DEFAULT '' CHECK (length(challenges) <= 8000),
    lessons TEXT NOT NULL DEFAULT '' CHECK (length(lessons) <= 8000),
    next_focus TEXT NOT NULL DEFAULT '' CHECK (length(next_focus) <= 8000),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (period_kind, period_start),
    CHECK (
        (period_kind = 'week' AND strftime('%w', period_start) = '1')
        OR
        (period_kind = 'month' AND strftime('%d', period_start) = '01')
    )
);

CREATE INDEX IF NOT EXISTS idx_period_reviews_recent
    ON period_reviews(period_kind, period_start DESC);
