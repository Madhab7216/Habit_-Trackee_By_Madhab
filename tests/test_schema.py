import sqlite3
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = [
    ROOT / "migrations" / "0001_initial.sql",
    ROOT / "migrations" / "0002_pause_periods.sql",
    ROOT / "migrations" / "0004_period_reviews.sql",
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
            {"habits", "habit_entries", "habit_pause_periods", "goals", "settings", "period_reviews"}.issubset(
                names
            )
        )

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

    def test_period_reviews_enforce_week_and_month_boundaries(self):
        self.connection.execute(
            "INSERT INTO period_reviews(period_kind, period_start, went_well) "
            "VALUES ('week', '2026-11-16', 'Walked regularly')"
        )
        self.connection.execute(
            "INSERT INTO period_reviews(period_kind, period_start) "
            "VALUES ('month', '2026-11-01')"
        )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO period_reviews(period_kind, period_start) "
                "VALUES ('week', '2026-11-17')"
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO period_reviews(period_kind, period_start) "
                "VALUES ('month', '2026-11-02')"
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO period_reviews(period_kind, period_start) "
                "VALUES ('week', '2026-11-16')"
            )

    def test_review_notes_are_length_bounded(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO period_reviews(period_kind, period_start, lessons) "
                "VALUES ('week', '2026-11-16', ?)",
                ("x" * 8001,),
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
