"""Unit tests for TaskBoard."""

import pytest

from see_agent.team.task_board import TaskBoard


@pytest.fixture
def board(tmp_path):
    return TaskBoard(tmp_path)


class TestTaskBoard:

    def test_empty_board(self, board):
        assert board.list_tasks() == []

    def test_create_task(self, board):
        task = board.create_task("Do something", description="details", created_by="alice")
        assert task.title == "Do something"
        assert task.description == "details"
        assert task.status == "pending"
        assert task.created_by == "alice"
        assert task.id  # non-empty

    def test_create_persists(self, board):
        board.create_task("Task A")
        tasks = board.list_tasks()
        assert len(tasks) == 1
        assert tasks[0].title == "Task A"

    def test_claim_task(self, board):
        task = board.create_task("Task A")
        claimed = board.claim_task(task.id, "bob")
        assert claimed.status == "claimed"
        assert claimed.assigned_to == "bob"

    def test_complete_task(self, board):
        task = board.create_task("Task A")
        board.claim_task(task.id, "bob")
        done = board.complete_task(task.id, "bob", result="all good")
        assert done.status == "done"
        assert done.result == "all good"

    def test_assign_task(self, board):
        task = board.create_task("Task A")
        assigned = board.assign_task(task.id, "charlie")
        assert assigned.assigned_to == "charlie"
        # Status stays pending when using assign (not claim).
        assert assigned.status == "pending"

    def test_update_task(self, board):
        task = board.create_task("Task A")
        updated = board.update_task(task.id, status="in_progress")
        assert updated.status == "in_progress"

    def test_list_tasks_filter_by_status(self, board):
        board.create_task("A")
        t2 = board.create_task("B")
        board.claim_task(t2.id, "bob")
        pending = board.list_tasks(status="pending")
        claimed = board.list_tasks(status="claimed")
        assert len(pending) == 1
        assert len(claimed) == 1
        assert pending[0].title == "A"
        assert claimed[0].title == "B"

    def test_unknown_task_raises(self, board):
        with pytest.raises(KeyError):
            board.claim_task("nonexistent", "bob")

    def test_multiple_tasks_ordered(self, board):
        for i in range(3):
            board.create_task(f"Task {i}")
        tasks = board.list_tasks()
        assert len(tasks) == 3
        assert [t.title for t in tasks] == ["Task 0", "Task 1", "Task 2"]
