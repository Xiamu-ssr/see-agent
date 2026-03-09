"""Unit tests for team collaboration tools."""

from __future__ import annotations

import pytest

from see_agent.hand.tools.team_tools import (
    AssignTaskTool,
    ClaimTaskTool,
    CompleteTaskTool,
    CreateTaskTool,
    ListTasksTool,
    SendMessageTool,
    UpdateTaskTool,
)
from see_agent.team.bus import TeamBus
from see_agent.team.task_board import TaskBoard


@pytest.fixture
def bus(tmp_path):
    b = TeamBus(tmp_path)
    b.register("alice")
    b.register("bob")
    return b


@pytest.fixture
def board(tmp_path):
    return TaskBoard(tmp_path)


class TestSendMessageTool:

    @pytest.mark.asyncio
    async def test_send(self, bus):
        tool = SendMessageTool(bus, "alice")
        result = await tool.execute(to="bob", content="hello")
        assert "bob" in result
        msgs = bus.drain("bob")
        assert len(msgs) == 1
        assert msgs[0].content == "hello"


class TestListTasksTool:

    @pytest.mark.asyncio
    async def test_list_empty(self, board):
        tool = ListTasksTool(board)
        result = await tool.execute()
        assert "No tasks" in result

    @pytest.mark.asyncio
    async def test_list_with_tasks(self, board):
        board.create_task("Fix bug")
        tool = ListTasksTool(board)
        result = await tool.execute()
        assert "Fix bug" in result


class TestCreateTaskTool:

    @pytest.mark.asyncio
    async def test_create(self, board):
        tool = CreateTaskTool(board, "alice")
        result = await tool.execute(title="New task", description="desc")
        assert "created" in result.lower()
        tasks = board.list_tasks()
        assert len(tasks) == 1
        assert tasks[0].created_by == "alice"


class TestClaimTaskTool:

    @pytest.mark.asyncio
    async def test_claim(self, board):
        task = board.create_task("Task A")
        tool = ClaimTaskTool(board, "bob")
        result = await tool.execute(task_id=task.id)
        assert "Claimed" in result
        updated = board.list_tasks()[0]
        assert updated.status == "claimed"


class TestCompleteTaskTool:

    @pytest.mark.asyncio
    async def test_complete(self, board):
        task = board.create_task("Task A")
        tool = CompleteTaskTool(board, "bob")
        result = await tool.execute(task_id=task.id, result="done")
        assert "Completed" in result
        updated = board.list_tasks()[0]
        assert updated.status == "done"


class TestUpdateTaskTool:

    @pytest.mark.asyncio
    async def test_update(self, board):
        task = board.create_task("Task A")
        tool = UpdateTaskTool(board)
        result = await tool.execute(task_id=task.id, status="in_progress")
        assert "in_progress" in result


class TestAssignTaskTool:

    @pytest.mark.asyncio
    async def test_assign(self, board):
        task = board.create_task("Task A")
        tool = AssignTaskTool(board)
        result = await tool.execute(task_id=task.id, agent_id="charlie")
        assert "charlie" in result
        updated = board.list_tasks()[0]
        assert updated.assigned_to == "charlie"
