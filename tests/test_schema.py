import sqlite3
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATION = ROOT / "migrations" / "0001_initial.sql"


class InitialSchemaTests(unittest.TestCase):
    def setUp(self):
        self.connection = sqlite3.connect(":memory:")
        self.connection.execute("PRAGMA foreign_keys = ON")
        self.connection.executescript(MIGRATION.read_text(encoding="utf-8"))

    def tearDown(self):
        self.connection.close()

    def test_schema_has_expected_tables(self):
        names = {
            row[0]
            for row in self.connection.execute(
                "SELECT name FROM sqlite_master WHERE type='table'"
            )
        }
        self.assertTrue({"habits", "habit_entries", "goals", "settings"}.issubset(names))

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


if __name__ == "__main__":
    unittest.main(verbosity=2)
