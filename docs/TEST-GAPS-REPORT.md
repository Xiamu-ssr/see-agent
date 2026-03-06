# 测试补全 Report

> 日期：2026-03-06
> 触发：Phase 2+3 代码 review，发现测试覆盖不足
> 目标：补全会话持久化相关的测试用例

---

## 现有测试覆盖

| 文件 | 测试数 | 覆盖范围 |
|------|--------|---------|
| `test_session.py` | 10 | SessionStore CRUD、clean、消息读写、meta 更新 |
| `test_loop.py` | 5 | AgentLoop 基本流程（已迁移到 session 模式） |
| `test_context.py` | 8 | ConversationContext 消息管理 + 滑动窗口 |

## 需要补全的测试

### 1. ConversationContext + on_append 回调（test_context.py）

现有 test_context.py 的 8 个测试**全部没有测试 on_append 回调**，因为是 Phase 2 之前写的。

需要新增：

```python
class TestOnAppendCallback:
    """Tests for the on_append persistence callback."""

    def test_on_append_called_on_user_task(self):
        """add_user_task should fire on_append with type=user_task and screenshot_ref."""
        recorded = []
        ctx = ConversationContext("sys", on_append=recorded.append)
        ctx.add_user_task("hello", "base64data", "high", screenshot_ref="step_000.webp")
        # recorded should contain: system message + user_task
        assert len(recorded) == 2
        assert recorded[1]["type"] == "user_task"
        assert recorded[1]["screenshot"] == "step_000.webp"
        assert "base64data" not in json.dumps(recorded)  # no base64 leak

    def test_on_append_called_on_assistant(self):
        """add_assistant should fire on_append with type=assistant and parsed tool_calls."""
        recorded = []
        ctx = ConversationContext("sys", on_append=recorded.append)
        # Need a mock message with model_dump()
        msg = MagicMock()
        msg.model_dump.return_value = {
            "role": "assistant",
            "content": "thinking...",
            "tool_calls": [
                {"id": "tc1", "type": "function", "function": {"name": "click", "arguments": '{"x": 100}'}}
            ],
        }
        ctx.add_assistant(msg)
        assert recorded[-1]["type"] == "assistant"
        assert recorded[-1]["tool_calls"][0]["name"] == "click"

    def test_on_append_called_on_tool_result(self):
        """add_tool_result should fire on_append with screenshot_ref, no base64."""
        recorded = []
        ctx = ConversationContext("sys", on_append=recorded.append)
        ctx.add_tool_result("tc1", "ok", "base64img", screenshot_ref="step_001.webp")
        tool_msg = [r for r in recorded if r.get("type") == "tool_result"]
        assert len(tool_msg) == 1
        assert tool_msg[0]["screenshot"] == "step_001.webp"
        assert "base64img" not in json.dumps(recorded)

    def test_on_append_called_on_user_reply(self):
        """add_user_reply should fire on_append."""
        recorded = []
        ctx = ConversationContext("sys", on_append=recorded.append)
        ctx.add_user_reply("yes")
        assert recorded[-1]["type"] == "user_reply"
        assert recorded[-1]["text"] == "yes"

    def test_on_append_called_on_system_hint(self):
        """add_system_hint should fire on_append."""
        recorded = []
        ctx = ConversationContext("sys", on_append=recorded.append)
        ctx.add_system_hint("You are stuck")
        assert recorded[-1]["type"] == "system_hint"

    def test_on_append_system_message_on_init(self):
        """Constructor should persist the system prompt via on_append."""
        recorded = []
        ctx = ConversationContext("You are a helper", on_append=recorded.append)
        assert recorded[0]["type"] == "system"
        assert "helper" in recorded[0]["content"]

    def test_no_on_append_still_works(self):
        """Without on_append, all add_* methods should work normally (backward compat)."""
        ctx = ConversationContext("sys")
        ctx.add_user_task("test", "b64", "high")
        ctx.add_user_reply("ok")
        assert len(ctx.get_messages()) == 3  # system + user_task + reply
```

### 2. AgentLoop + session 集成（test_loop.py）

现有 test_loop.py 测试了 loop 能跑，但没有验证 session 数据是否正确写入。

需要新增：

```python
class TestAgentLoopSession:
    """Verify that AgentLoop correctly creates and populates sessions."""

    @pytest.mark.asyncio
    async def test_session_created_on_new_run(self, tmp_path):
        """Running without session_id should create a new session directory."""
        # ... run loop ...
        # Assert: session dir exists, meta.json has status=completed, messages.jsonl non-empty

    @pytest.mark.asyncio
    async def test_session_meta_updated_on_completion(self, tmp_path):
        """meta.json should reflect final status, steps, and summary."""
        # ... run loop to completion ...
        # Assert: meta["status"] == "completed", meta["total_steps"] > 0

    @pytest.mark.asyncio
    async def test_session_meta_updated_on_failure(self, tmp_path):
        """Failed runs should set status=failed in meta.json."""
        # ... run loop that hits max_steps ...
        # Assert: meta["status"] == "failed"

    @pytest.mark.asyncio
    async def test_messages_jsonl_populated(self, tmp_path):
        """messages.jsonl should contain entries for each conversation turn."""
        # ... run loop with 1 click + finished ...
        # Read messages.jsonl, verify:
        #   - system message
        #   - user_task with screenshot ref
        #   - assistant with tool_calls
        #   - tool_result with screenshot ref
        #   - assistant (finished)

    @pytest.mark.asyncio
    async def test_screenshots_in_session_dir(self, tmp_path):
        """Screenshots should be saved inside session/screenshots/, not old SCREENSHOTS_DIR."""
        # ... run loop ...
        # Assert: session.screenshots_dir has step_000.webp etc.

    @pytest.mark.asyncio
    async def test_resume_uses_existing_session(self, tmp_path):
        """Passing session_id should reuse the existing session directory."""
        # Create session, run with session_id
        # Assert: same session dir, meta updated, no new session created

    @pytest.mark.asyncio
    async def test_result_contains_session_id(self, tmp_path):
        """RunResult.session_id should be populated."""
        # ... run loop ...
        # Assert: result.session_id is truthy and matches session dir name
```

### 3. CLI sessions 命令（test_cli.py 或 test_sessions_cli.py）

目前**没有任何 CLI sessions 命令的测试**。

需要新增：

```python
class TestSessionsCLI:
    """Tests for `see-agent sessions` subcommands using CliRunner."""

    def test_sessions_list_empty(self):
        """sessions list with no sessions should print 'No sessions found.'"""

    def test_sessions_list_shows_sessions(self):
        """Create a few sessions, verify list output contains their IDs."""

    def test_sessions_list_status_filter(self):
        """--status completed should only show completed sessions."""

    def test_sessions_show_existing(self):
        """sessions show <id> should print meta.json content."""

    def test_sessions_show_nonexistent(self):
        """sessions show <bad_id> should exit with error."""

    def test_sessions_clean_removes_old(self):
        """sessions clean --keep 0 should remove all sessions."""

    def test_sessions_clean_empty_only(self):
        """sessions clean --empty should only remove sessions without screenshots."""
```

### 4. API sessions 接口（test_api_sessions.py）

目前**没有任何 API sessions 路由的测试**。

需要新增：

```python
class TestSessionsAPI:
    """Tests for /api/sessions routes using TestClient."""

    def test_list_sessions_empty(self):
        """GET /api/sessions with no sessions should return empty list."""

    def test_list_sessions(self):
        """GET /api/sessions should return created sessions."""

    def test_get_session_detail(self):
        """GET /api/sessions/<id> should return meta + counts."""

    def test_get_session_404(self):
        """GET /api/sessions/<bad_id> should return 404."""

    def test_get_screenshot(self):
        """GET /api/sessions/<id>/screenshot/0 should return WebP file."""

    def test_get_screenshot_404(self):
        """GET /api/sessions/<id>/screenshot/999 should return 404."""

    def test_delete_session(self):
        """DELETE /api/sessions/<id> should remove the session."""

    def test_delete_session_404(self):
        """DELETE /api/sessions/<bad_id> should return 404."""

    def test_chat_with_session_id(self):
        """POST /api/chat with session_id should reuse existing session."""
```

### 5. 边界场景（补充到各测试文件）

```python
# test_session.py 补充
class TestSessionEdgeCases:

    def test_clean_with_corrupted_meta_json(self):
        """Session dir with invalid JSON in meta.json should be treated as orphan."""

    def test_list_with_mixed_valid_invalid_sessions(self):
        """Invalid session dirs should be skipped, not crash list()."""

    def test_append_message_unicode(self):
        """Chinese and emoji content should roundtrip through JSONL correctly."""

    def test_concurrent_append_messages(self):
        """Multiple rapid appends should not corrupt the JSONL file."""

    def test_create_session_id_uniqueness(self):
        """Two sessions created in the same second should have different IDs."""
        # (the _short_id() suffix should prevent collision)

    def test_load_session_from_different_status(self):
        """Loading a 'failed' session should preserve its status."""

    def test_clean_age_based(self):
        """Sessions older than keep_days should be deleted."""
        # Need to mock timestamps to make session appear old
```

---

## 执行方式

把这个文件给 CC，让它：

1. 按上面的用例逐个实现测试
2. 每写完一组跑 `bash scripts/check.sh` 确保不破坏现有测试
3. 所有测试全过后 commit

**注意：** 只写测试，不改业务代码。如果测试暴露了 bug，写到测试文件的注释里，单独提 issue。

---

## 优先级

1. **P0**：ConversationContext on_append 回调测试（这是持久化的核心路径）
2. **P0**：AgentLoop + session 集成测试
3. **P1**：CLI sessions 命令测试
4. **P1**：API sessions 接口测试
5. **P2**：边界场景
