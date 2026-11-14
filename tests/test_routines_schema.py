import sqlite3
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = [
    ROOT / "migrations" / "0001_initial.sql",
    ROOT / "migrations" / "0002_pause_periods.sql",
    ROOT / "migrations" / "0003_goal_milestones.sql",
    ROOT / "migrations" / "0004_routines.sql",
]


class RoutineSchemaTests(unittest.TestCase):
    def setUp(self):
        self.connection = sqlite3.connect(":memory:")
        self.connection.execute("PRAGMA foreign_keys = ON")
        for migration in MIGRATIONS:
            self.connection.executescript(migration.read_text(encoding="utf-8"))

    def tearDown(self):
        self.connection.close()

    def create_routine(self):
        cursor = self.connection.execute(
            "INSERT INTO routines(title, start_date) VALUES (?, ?)",
            ("Morning focus", "2026-11-14"),
        )
        return cursor.lastrowid

    def test_routine_tables_and_indexes_exist(self):
        names = {
            row[0]
            for row in self.connection.execute(
                "SELECT name FROM sqlite_master WHERE type IN ('table', 'index')"
            )
        }
        self.assertTrue(
            {
                "routines",
                "routine_steps",
                "routine_runs",
                "routine_run_steps",
                "idx_routines_status_sort",
                "idx_routine_steps_routine_sort",
                "idx_routine_runs_routine_date",
                "idx_routine_run_steps_run_status",
            }.issubset(names)
        )

    def test_routine_title_and_schedule_are_constrained(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routines(title, start_date) VALUES ('   ', '2026-11-14')"
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routines(title, schedule_type, weekday_mask, start_date) "
                "VALUES ('Invalid schedule', 'weekdays', 0, '2026-11-14')"
            )
        for invalid in ("2027-02-29", "2027-13-01", "not-a-date"):
            with self.subTest(invalid=invalid), self.assertRaises(sqlite3.IntegrityError):
                self.connection.execute(
                    "INSERT INTO routines(title, start_date) VALUES ('Invalid date', ?)",
                    (invalid,),
                )

    def test_routine_step_requires_parent_and_valid_kind(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routine_steps(routine_id, title) VALUES (999, 'Orphan')"
            )
        routine_id = self.create_routine()
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routine_steps(routine_id, title, step_type) "
                "VALUES (?, 'Missing habit', 'habit')",
                (routine_id,),
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routine_steps(routine_id, title, step_type) "
                "VALUES (?, 'Wrong type', 'unknown')",
                (routine_id,),
            )

    def test_habit_step_must_reference_a_real_habit(self):
        routine_id = self.create_routine()
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routine_steps"
                "(routine_id, title, step_type, linked_habit_id) "
                "VALUES (?, 'Read', 'habit', 999)",
                (routine_id,),
            )

    def test_run_is_unique_per_routine_and_local_date(self):
        routine_id = self.create_routine()
        self.connection.execute(
            "INSERT INTO routine_runs(routine_id, local_date) VALUES (?, ?)",
            (routine_id, "2026-11-14"),
        )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routine_runs(routine_id, local_date) VALUES (?, ?)",
                (routine_id, "2026-11-14"),
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO routine_runs(routine_id, local_date) VALUES (?, ?)",
                (routine_id, "2027-02-29"),
            )

    def test_run_step_status_and_snapshot_survive_template_edit(self):
        routine_id = self.create_routine()
        cursor = self.connection.execute(
            "INSERT INTO routine_steps(routine_id, title, sort_order) "
            "VALUES (?, 'Original step', 0)",
            (routine_id,),
        )
        step_id = cursor.lastrowid
        run_id = self.connection.execute(
            "INSERT INTO routine_runs(routine_id, local_date) VALUES (?, ?)",
            (routine_id, "2026-11-14"),
        ).lastrowid
        self.connection.execute(
            "INSERT INTO routine_run_steps"
            "(run_id, step_id, title_snapshot, step_type_snapshot, is_required, sort_order) "
            "VALUES (?, ?, 'Original step', 'action', 1, 0)",
            (run_id, step_id),
        )
        self.connection.execute("DELETE FROM routine_steps WHERE id = ?", (step_id,))
        saved = self.connection.execute(
            "SELECT title_snapshot FROM routine_run_steps WHERE run_id = ?",
            (run_id,),
        ).fetchone()
        self.assertEqual(saved[0], "Original step")
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "UPDATE routine_run_steps SET status = 'missed' WHERE run_id = ?",
                (run_id,),
            )

    def test_routine_step_delete_does_not_delete_run_history(self):
        routine_id = self.create_routine()
        self.connection.execute(
            "INSERT INTO routine_steps(routine_id, title) VALUES (?, 'Step')",
            (routine_id,),
        )
        run_id = self.connection.execute(
            "INSERT INTO routine_runs(routine_id, local_date) VALUES (?, ?)",
            (routine_id, "2026-11-14"),
        ).lastrowid
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute("DELETE FROM routines WHERE id = ?", (routine_id,))
        self.connection.execute("DELETE FROM routine_runs WHERE id = ?", (run_id,))
        remaining = self.connection.execute(
            "SELECT COUNT(*) FROM routine_run_steps WHERE run_id = ?",
            (run_id,),
        ).fetchone()[0]
        self.assertEqual(remaining, 0)


if __name__ == "__main__":
    unittest.main(verbosity=2)
