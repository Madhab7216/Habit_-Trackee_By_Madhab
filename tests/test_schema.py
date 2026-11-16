import sqlite3
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = [
    ROOT / "migrations" / "0001_initial.sql",
    ROOT / "migrations" / "0002_pause_periods.sql",
    ROOT / "migrations" / "0003_schedule_revisions.sql",
]


class InitialSchemaTests(unittest.TestCase):
    def setUp(self):
        self.connection = sqlite3.connect(":memory:")
        self.connection.execute("PRAGMA foreign_keys = ON")
        for migration in MIGRATIONS:
            self.connection.executescript(migration.read_text(encoding="utf-8"))

    def tearDown(self):
        self.connection.close()

    def test_schema_has_expected_tables(self):
        names = {
            row[0]
            for row in self.connection.execute(
                "SELECT name FROM sqlite_master WHERE type='table'"
            )
        }
        self.assertTrue(
            {"habits", "habit_entries", "habit_pause_periods", "habit_schedule_revisions", "goals", "settings"}.issubset(
                names
            )
        )


    def test_new_habit_seeds_an_initial_schedule_revision(self):
        cursor = self.connection.execute(
            "INSERT INTO habits(name, schedule_type, weekday_mask, start_date) "
            "VALUES ('Walk', 'weekdays', 31, '2026-11-01')"
        )
        row = self.connection.execute(
            "SELECT effective_from_date, effective_until_date, schedule_type, weekday_mask "
            "FROM habit_schedule_revisions WHERE habit_id = ?",
            (cursor.lastrowid,),
        ).fetchone()
        self.assertEqual(row, ("2026-11-01", None, "weekdays", 31))

    def test_schedule_revisions_cannot_overlap(self):
        cursor = self.connection.execute(
            "INSERT INTO habits(name, start_date) VALUES ('Read', '2026-11-01')"
        )
        habit_id = cursor.lastrowid
        self.connection.execute(
            "UPDATE habit_schedule_revisions SET effective_until_date = '2026-11-20' "
            "WHERE habit_id = ?",
            (habit_id,),
        )
        self.connection.execute(
            "INSERT INTO habit_schedule_revisions "
            "(habit_id, effective_from_date, schedule_type) "
            "VALUES (?, '2026-11-20', 'daily')",
            (habit_id,),
        )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_schedule_revisions "
                "(habit_id, effective_from_date, effective_until_date, schedule_type) "
                "VALUES (?, '2026-11-19', '2026-11-21', 'daily')",
                (habit_id,),
            )

    def test_effective_schedule_and_legacy_schedule_fields_are_immutable(self):
        cursor = self.connection.execute(
            "INSERT INTO habits(name, start_date) VALUES ('Journal', '2026-11-01')"
        )
        habit_id = cursor.lastrowid
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "UPDATE habit_schedule_revisions SET schedule_type = 'weekdays', "
                "weekday_mask = 1 WHERE habit_id = ?",
                (habit_id,),
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "DELETE FROM habit_schedule_revisions WHERE habit_id = ?",
                (habit_id,),
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "UPDATE habits SET schedule_type = 'weekdays', weekday_mask = 1 "
                "WHERE id = ?",
                (habit_id,),
            )

    def test_schedule_revision_migration_backfills_existing_habits(self):
        connection = sqlite3.connect(":memory:")
        connection.execute("PRAGMA foreign_keys = ON")
        connection.executescript(
            (ROOT / "migrations" / "0001_initial.sql").read_text(encoding="utf-8")
        )
        connection.executescript(
            (ROOT / "migrations" / "0002_pause_periods.sql").read_text(encoding="utf-8")
        )
        cursor = connection.execute(
            "INSERT INTO habits(name, schedule_type, interval_days, start_date, end_date) "
            "VALUES ('Stretch', 'every_n_days', 3, '2026-11-01', '2026-12-31')"
        )
        connection.executescript(
            (ROOT / "migrations" / "0003_schedule_revisions.sql").read_text(encoding="utf-8")
        )
        row = connection.execute(
            "SELECT effective_from_date, effective_until_date, schedule_type, "
            "interval_days, schedule_end_date "
            "FROM habit_schedule_revisions WHERE habit_id = ?",
            (cursor.lastrowid,),
        ).fetchone()
        self.assertEqual(
            row,
            ("2026-11-01", None, "every_n_days", 3, "2026-12-31"),
        )
        connection.close()

    def test_habit_can_be_created_and_completed_once_per_day(self):
        cursor = self.connection.execute(
            "INSERT INTO habits(name) VALUES (?)", ("Read 20 pages",)
        )
        habit_id = cursor.lastrowid
        self.connection.execute(
            "INSERT INTO habit_entries(habit_id, local_date, status, value) "
            "VALUES (?, '2026-11-14', 'completed', 1)",
            (habit_id,),
        )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_entries(habit_id, local_date, status, value) "
                "VALUES (?, '2026-11-14', 'completed', 1)",
                (habit_id,),
            )

    def test_foreign_keys_prevent_orphan_entries(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_entries(habit_id, local_date, status) "
                "VALUES (999, '2026-11-14', 'completed')"
            )

    def test_invalid_habit_type_is_rejected(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habits(name, tracking_type) VALUES ('Invalid', 'mood')"
            )

    def test_invalid_status_is_rejected(self):
        cursor = self.connection.execute("INSERT INTO habits(name) VALUES ('Read')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_entries(habit_id, local_date, status) "
                "VALUES (?, '2026-11-14', 'missed')",
                (cursor.lastrowid,),
            )

    def test_invalid_date_is_rejected(self):
        cursor = self.connection.execute("INSERT INTO habits(name) VALUES ('Journal')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_entries(habit_id, local_date, status) "
                "VALUES (?, 'not-a-date', 'completed')",
                (cursor.lastrowid,),
            )

    def test_habit_history_is_protected_from_accidental_delete(self):
        cursor = self.connection.execute("INSERT INTO habits(name) VALUES ('Exercise')")
        habit_id = cursor.lastrowid
        self.connection.execute(
            "INSERT INTO habit_entries(habit_id, local_date, status) "
            "VALUES (?, '2026-11-14', 'completed')",
            (habit_id,),
        )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute("DELETE FROM habits WHERE id = ?", (habit_id,))

    def test_pause_history_is_linked_to_a_real_habit(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_pause_periods(habit_id, starts_on) "
                "VALUES (999, '2026-11-14')"
            )

    def test_only_one_open_pause_period_per_habit(self):
        cursor = self.connection.execute("INSERT INTO habits(name) VALUES ('Walk')")
        habit_id = cursor.lastrowid
        self.connection.execute(
            "INSERT INTO habit_pause_periods(habit_id, starts_on) "
            "VALUES (?, '2026-11-10')",
            (habit_id,),
        )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_pause_periods(habit_id, starts_on) "
                "VALUES (?, '2026-11-12')",
                (habit_id,),
            )

    def test_pause_end_cannot_precede_start(self):
        cursor = self.connection.execute("INSERT INTO habits(name) VALUES ('Journal')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO habit_pause_periods(habit_id, starts_on, ends_on) "
                "VALUES (?, '2026-11-14', '2026-11-13')",
                (cursor.lastrowid,),
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
