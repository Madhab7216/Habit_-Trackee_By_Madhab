import sqlite3
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = [
    ROOT / "migrations" / "0001_initial.sql",
    ROOT / "migrations" / "0002_pause_periods.sql",
    ROOT / "migrations" / "0003_goal_milestones.sql",
]


class GoalMilestoneSchemaTests(unittest.TestCase):
    def setUp(self):
        self.connection = sqlite3.connect(":memory:")
        self.connection.execute("PRAGMA foreign_keys = ON")
        for migration in MIGRATIONS:
            self.connection.executescript(migration.read_text(encoding="utf-8"))

    def tearDown(self):
        self.connection.close()

    def create_goal(self):
        cursor = self.connection.execute(
            "INSERT INTO goals(title, description) VALUES (?, ?)",
            ("Learn Rust", "Ship a small offline desktop app"),
        )
        return cursor.lastrowid

    def test_milestone_table_and_indexes_exist(self):
        table = self.connection.execute(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='goal_milestones'"
        ).fetchone()
        indexes = {
            row[0]
            for row in self.connection.execute(
                "SELECT name FROM sqlite_master WHERE type='index'"
            )
        }
        self.assertIsNotNone(table)
        self.assertIn("idx_goal_milestones_goal_status_sort", indexes)
        self.assertIn("idx_goal_milestones_due_date", indexes)

    def test_milestone_requires_existing_goal(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO goal_milestones(goal_id, title) VALUES (999, 'Orphan milestone')"
            )

    def test_milestone_status_is_constrained(self):
        goal_id = self.create_goal()
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO goal_milestones(goal_id, title, status) VALUES (?, ?, ?)",
                (goal_id, "Invalid status", "in-progress"),
            )

    def test_milestone_title_is_nonempty_and_bounded(self):
        goal_id = self.create_goal()
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO goal_milestones(goal_id, title) VALUES (?, '   ')",
                (goal_id,),
            )
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute(
                "INSERT INTO goal_milestones(goal_id, title) VALUES (?, ?)",
                (goal_id, "x" * 121),
            )

    def test_milestone_due_date_requires_a_real_iso_date(self):
        goal_id = self.create_goal()
        self.connection.execute(
            "INSERT INTO goal_milestones(goal_id, title, due_date) VALUES (?, ?, ?)",
            (goal_id, "Valid leap day", "2028-02-29"),
        )
        for invalid in ("2027-02-29", "2027-02-30", "2027-04-31", "2027-13-01", "not-a-date"):
            with self.subTest(invalid=invalid), self.assertRaises(sqlite3.IntegrityError):
                self.connection.execute(
                    "INSERT INTO goal_milestones(goal_id, title, due_date) VALUES (?, ?, ?)",
                    (goal_id, "Invalid due date", invalid),
                )

    def test_milestones_are_cascaded_with_their_goal(self):
        goal_id = self.create_goal()
        self.connection.execute(
            "INSERT INTO goal_milestones(goal_id, title) VALUES (?, ?)",
            (goal_id, "First step"),
        )
        self.connection.execute("DELETE FROM goals WHERE id = ?", (goal_id,))
        remaining = self.connection.execute(
            "SELECT COUNT(*) FROM goal_milestones WHERE goal_id = ?",
            (goal_id,),
        ).fetchone()[0]
        self.assertEqual(remaining, 0)

    def test_existing_goal_habit_links_remain_supported(self):
        goal_id = self.create_goal()
        habit_id = self.connection.execute(
            "INSERT INTO habits(name) VALUES ('Daily reading')"
        ).lastrowid
        self.connection.execute(
            "INSERT INTO goal_habits(goal_id, habit_id) VALUES (?, ?)",
            (goal_id, habit_id),
        )
        count = self.connection.execute(
            "SELECT COUNT(*) FROM goal_habits WHERE goal_id = ?",
            (goal_id,),
        ).fetchone()[0]
        self.assertEqual(count, 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
