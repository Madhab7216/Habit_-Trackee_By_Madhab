-- Version 3: retain the recurrence rules that were in force for each civil-date range.
-- effective_until_date is an exclusive upper bound; schedule_end_date remains inclusive,
-- matching the legacy habits.end_date contract.
CREATE TABLE IF NOT EXISTS habit_schedule_revisions (
    id INTEGER PRIMARY KEY,
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE RESTRICT,
    effective_from_date TEXT NOT NULL
        CHECK (date(effective_from_date) IS NOT NULL
           AND date(effective_from_date) = effective_from_date),
    effective_until_date TEXT
        CHECK (
            effective_until_date IS NULL OR (
                date(effective_until_date) IS NOT NULL
                AND date(effective_until_date) = effective_until_date
                AND effective_until_date > effective_from_date
            )
        ),
    schedule_type TEXT NOT NULL
        CHECK (schedule_type IN ('daily', 'weekdays', 'every_n_days', 'times_per_week')),
    weekday_mask INTEGER CHECK (weekday_mask IS NULL OR weekday_mask BETWEEN 1 AND 127),
    interval_days INTEGER CHECK (interval_days IS NULL OR interval_days >= 1),
    target_per_period INTEGER CHECK (target_per_period IS NULL OR target_per_period BETWEEN 1 AND 7),
    period_unit TEXT NOT NULL DEFAULT 'week'
        CHECK (period_unit IN ('day', 'week', 'month')),
    schedule_end_date TEXT
        CHECK (
            schedule_end_date IS NULL OR (
                date(schedule_end_date) IS NOT NULL
                AND date(schedule_end_date) = schedule_end_date
                AND schedule_end_date >= effective_from_date
            )
        ),
    time_zone_id TEXT NOT NULL DEFAULT 'local'
        CHECK (length(trim(time_zone_id)) BETWEEN 1 AND 255),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (schedule_type != 'weekdays' OR weekday_mask IS NOT NULL),
    CHECK (schedule_type != 'every_n_days' OR interval_days IS NOT NULL),
    CHECK (
        schedule_type != 'times_per_week'
        OR (target_per_period IS NOT NULL AND period_unit = 'week')
    )
);

CREATE INDEX IF NOT EXISTS idx_habit_schedule_revision_lookup
    ON habit_schedule_revisions(habit_id, effective_from_date, effective_until_date);

CREATE UNIQUE INDEX IF NOT EXISTS idx_habit_schedule_one_open_revision
    ON habit_schedule_revisions(habit_id)
    WHERE effective_until_date IS NULL;

-- Seed one snapshot from the last-known legacy schedule. Earlier edits, if any, cannot
-- be reconstructed because the previous schema overwrote those values in place.
INSERT INTO habit_schedule_revisions (
    habit_id, effective_from_date, effective_until_date, schedule_type,
    weekday_mask, interval_days, target_per_period, period_unit,
    schedule_end_date, time_zone_id
)
SELECT
    id, start_date, NULL, schedule_type,
    weekday_mask, interval_days, target_per_period, period_unit,
    end_date, time_zone_id
FROM habits;

-- Every newly created habit starts with an initial schedule snapshot.
CREATE TRIGGER IF NOT EXISTS habit_schedule_revision_seed_after_insert
AFTER INSERT ON habits
BEGIN
    INSERT INTO habit_schedule_revisions (
        habit_id, effective_from_date, effective_until_date, schedule_type,
        weekday_mask, interval_days, target_per_period, period_unit,
        schedule_end_date, time_zone_id
    )
    VALUES (
        NEW.id, NEW.start_date, NULL, NEW.schedule_type,
        NEW.weekday_mask, NEW.interval_days, NEW.target_per_period, NEW.period_unit,
        NEW.end_date, NEW.time_zone_id
    );
END;

-- Prevent overlapping half-open effective-date ranges for one habit.
CREATE TRIGGER IF NOT EXISTS habit_schedule_revision_no_overlap_before_insert
BEFORE INSERT ON habit_schedule_revisions
WHEN EXISTS (
    SELECT 1
    FROM habit_schedule_revisions existing
    WHERE existing.habit_id = NEW.habit_id
      AND (existing.effective_until_date IS NULL
           OR existing.effective_until_date > NEW.effective_from_date)
      AND (NEW.effective_until_date IS NULL
           OR NEW.effective_until_date > existing.effective_from_date)
)
BEGIN
    SELECT RAISE(ABORT, 'overlapping habit schedule revision');
END;

-- Only an open revision that has not taken effect yet may be edited in place.
-- Once its effective date arrives, its schedule fields are immutable.
CREATE TRIGGER IF NOT EXISTS habit_schedule_revision_immutable_after_effective
BEFORE UPDATE OF
    habit_id, effective_from_date, schedule_type, weekday_mask, interval_days,
    target_per_period, period_unit, schedule_end_date, time_zone_id, created_at
ON habit_schedule_revisions
WHEN OLD.effective_until_date IS NOT NULL
  OR OLD.effective_from_date <= date('now', 'localtime')
BEGIN
    SELECT RAISE(ABORT, 'effective habit schedule revisions are immutable');
END;

-- An open revision can be closed once, at a strictly later non-overlapping date.
CREATE TRIGGER IF NOT EXISTS habit_schedule_revision_close_once
BEFORE UPDATE OF effective_until_date ON habit_schedule_revisions
WHEN OLD.effective_until_date IS NOT NULL
  OR NEW.effective_until_date IS NULL
  OR date(NEW.effective_until_date) IS NULL
  OR date(NEW.effective_until_date) != NEW.effective_until_date
  OR NEW.effective_until_date <= OLD.effective_from_date
  OR EXISTS (
      SELECT 1
      FROM habit_schedule_revisions existing
      WHERE existing.habit_id = OLD.habit_id
        AND existing.id != OLD.id
        AND (existing.effective_until_date IS NULL
             OR existing.effective_until_date > OLD.effective_from_date)
        AND existing.effective_from_date < NEW.effective_until_date
  )
BEGIN
    SELECT RAISE(ABORT, 'invalid or overlapping schedule revision close');
END;

-- Schedule history is retained with the habit; users change it by adding a revision.
CREATE TRIGGER IF NOT EXISTS habit_schedule_revision_prevent_delete
BEFORE DELETE ON habit_schedule_revisions
BEGIN
    SELECT RAISE(ABORT, 'habit schedule revisions cannot be deleted');
END;

-- Force future schedule changes through the effective-dated API instead of overwriting
-- the legacy current-schedule columns and silently reinterpreting past dates.
CREATE TRIGGER IF NOT EXISTS habits_prevent_schedule_overwrite
BEFORE UPDATE OF
    schedule_type, weekday_mask, interval_days, target_per_period,
    period_unit, time_zone_id, start_date, end_date
ON habits
WHEN NEW.schedule_type IS NOT OLD.schedule_type
  OR NEW.weekday_mask IS NOT OLD.weekday_mask
  OR NEW.interval_days IS NOT OLD.interval_days
  OR NEW.target_per_period IS NOT OLD.target_per_period
  OR NEW.period_unit IS NOT OLD.period_unit
  OR NEW.time_zone_id IS NOT OLD.time_zone_id
  OR NEW.start_date IS NOT OLD.start_date
  OR NEW.end_date IS NOT OLD.end_date
BEGIN
    SELECT RAISE(ABORT, 'use an effective-dated habit schedule revision');
END;
