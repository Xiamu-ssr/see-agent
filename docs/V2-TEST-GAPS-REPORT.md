# v2 测试补全 + 功能缺口 Report

> 日期：2026-03-06
> 版本：v2 ReAct Agent（commit 6d9ed52）
> 现状：161 个测试全过，check.sh 4/4 pass
> 目标：修复 PRD vs 代码的功能缺口 + 补全缺失的测试覆盖

---

## Part A：PRD vs 代码功能缺口

对照 PRD.md §2-§9 逐项排查，以下是**代码未实现或实现不完整**的功能。

### 🔴 A1：MCP 未接入 AgentLoop — 配置了也不会生效

**PRD 设计（§4.4）：** Agent 启动时连接所有配置的 MCP 服务器，获取 tool 列表，注册到 ToolRegistry。

**实际代码：**
- `hand/mcp.py` 写好了完整的 `MCPClient` / `MCPToolWrapper` / `MCPManager`（连接、注册、执行）
- `cli/main.py` 写好了 `mcp list/add/remove` CLI 命令（读写 config.json）
- **但是 `_build_components()` 和 `AgentLoop` 里完全没有调用 `MCPManager`**
- 也就是说：用户 `mcp add tavily ...` 写入了配置，但 `see-agent chat` 运行时**根本不连接 MCP 服务器，不注册 MCP 工具**

**需要修复：** 在 `_build_components()` 中，读取 `config["mcp_servers"]`，如果非空则创建 `MCPManager` → `connect_all()` → `register_tools(registry)` → 将 MCP 工具注入到 AgentLoop 的 registry 中。同时需要在退出时调用 `disconnect_all()`。

### 🔴 A2：Mem0 配置为空壳 — 开启后会用默认配置初始化

**PRD 设计（§6.4 + §7.2）：** `memory.mem0` 应包含 `llm_base_url`、`llm_api_key`、`llm_model`、`embedding_model`、`storage_path` 等子配置，用于初始化 Mem0 的 LLM、embedding、vector_store。

**实际代码：**
- `DEFAULT_CONFIG` 中 `"mem0": {}`（空 dict）
- `Mem0Memory.__init__` 收到空 dict 后走 `Memory()`（mem0 的纯默认配置）
- PRD 里设计的 `from_config` 参数格式（指定 qdrant 路径、自定义 LLM/embedding provider）完全没实现
- 用户即使配了 `memory.enabled: true`，也没法控制 Mem0 用哪个 LLM、存在哪里

**需要修复：**
1. `DEFAULT_CONFIG["memory"]["mem0"]` 补上 PRD 设计的子字段
2. `Mem0Memory.__init__` 根据 config 构建 mem0 的 provider 配置（llm、embedder、vector_store），而不是简单地 `Memory.from_config(config)` 或 `Memory()`

### 🟡 A3：`~/.see-agent/skills/` 目录不自动创建

**PRD 设计（§8）：** 工作目录包含 `skills/`。

**实际代码：**
- `config.py` 没有 `SKILLS_DIR` 常量，`ensure_workspace()` 不创建 `skills/`
- `skill/loader.py` 的 `load_skills()` 对不存在的目录静默跳过，不报错
- 用户需要手动 `mkdir ~/.see-agent/skills` 才能放技能文件

**需要修复：** `config.py` 加 `SKILLS_DIR = WORKSPACE_DIR / "skills"`，`ensure_workspace()` 加 `SKILLS_DIR.mkdir(exist_ok=True)`

### 🟡 A4：`~/.see-agent/memory/` 目录不自动创建

**PRD 设计（§8）：** 工作目录包含 `memory/qdrant/`。

**实际代码：** 同上，没有定义也没有创建。

**需要修复：** `config.py` 加 `MEMORY_DIR = WORKSPACE_DIR / "memory"`，`ensure_workspace()` 加 `MEMORY_DIR.mkdir(exist_ok=True)`

### 🟡 A5：`mcp list` 没有健康检查

**PRD 设计（§4.4）：** `mcp list` 应"遍历配置 → 尝试连接 → 打印健康状态"。

**实际代码：** `mcp list` 只读 config.json 打印名称和 command，不尝试连接，不报告健康状态。

**需要修复：** `mcp_list` 命令中 import `MCPManager`，对每个 server 尝试 `connect()` + `list_tools()`，打印连接状态和工具数量。

### 🟡 A6：`mcp add` 不支持 `--type` 和 `--url` 参数

**PRD 设计（§4.4）：** `mcp add github --type http --url "https://..."` 支持 stdio 和 http 两种 transport。

**实际代码：** `mcp_add` 只接受 `name` + `command` + `--arg`，写死了 `"command"` 和 `"args"` 字段。不支持 `type`、`url`、`headers`、`env` 参数。

**需要修复：** 扩展 `mcp_add` 参数，或至少支持 `--type stdio|http` 和 `--url`。

### 🟡 A7：`screenshots/` 目录仍在创建（dead code）

**PRD 设计（§8）：** v2 工作目录不包含顶层 `screenshots/`（截图在 `sessions/<id>/screenshots/` 下）。

**实际代码：** `ensure_workspace()` 还在创建 `SCREENSHOTS_DIR`，`config.py` 还在定义 `SCREENSHOTS_DIR` 常量。

**需要修复：** 删除 `SCREENSHOTS_DIR` 相关代码。

### ⚪ A8：config.json 中用户缺少 v2 新字段

**现象：** 用户的 config.json 是 v1 写的，缺少 `tool_delay_ms`、`profile`、`skills_dirs`、`memory`、`env`、`mcp_servers`。

**影响：** 无。`load_config()` 的 `_deep_merge(DEFAULT_CONFIG, config)` 会补全所有缺失字段。`config show` 输出完整。

**不需要修复。** 但可选提供 `config migrate` 命令把默认值写入 config.json，让文件内容和运行时一致。

---

## Part B：缺失测试（按优先级排列）

## 现有覆盖总览

| 测试文件 | 测试数 | 覆盖范围 |
|----------|--------|---------|
| test_brain.py | 12 | prompt 构建、response 解析、message 摘要 |
| test_config.py | 4 | deep_merge、profile 加载 |
| test_context.py | 10 | 消息管理、滑动窗口、ToolResult 兼容 |
| test_environment.py | 4 | 环境信息收集 |
| test_eye.py | 10 | Screenshot 数据模型 |
| test_hand.py | 14 | Tool ABC、Registry、ToolResult 包装 |
| test_integration_assembly.py | 6 | CLI/Server 组件组装 |
| test_loop.py | 10 | AgentLoop 核心流程 |
| test_mcp.py | 4 | env 展开、tool wrapper |
| test_memory.py | 6 | BaseMemory mock、strip_base64 |
| test_overlay.py | 17 | overlay 渲染 + loop 集成 |
| test_scaling.py | 20 | 坐标缩放 |
| test_session.py | 16 | session CRUD、restore_context |
| test_skill.py | 8 | 技能加载、prompt 注入 |
| **合计** | **161** | |

---

## 缺失测试（按优先级排列）

### P0：AgentLoop v2 新行为

**文件：** `tests/test_loop.py`

这些是 v2 的核心逻辑变更，直接影响 agent 运行质量。

```python
class TestAgentLoopV2Behavior:

    @pytest.mark.asyncio
    async def test_no_screenshot_warning_injected(self, tmp_path):
        """Agent 连续 5 步不截图时应注入 system_hint 提醒。

        构造场景：brain 连续返回 5 次 click（无 screenshot tool），
        验证第 5 步后 context 中出现 "you have not taken a screenshot" hint。
        """

    @pytest.mark.asyncio
    async def test_screenshot_tool_images_saved_to_disk(self, tmp_path):
        """screenshot tool 返回 ToolResult 带 images 时，图片应保存到 session 目录。

        mock registry.execute 返回 ToolResult(text=..., images=[ToolResultImage(...)]),
        验证 session/screenshots/ 下出现对应的 .webp 文件。
        """

    @pytest.mark.asyncio
    async def test_screenshot_tool_images_hash_detection(self, tmp_path):
        """screenshot tool 返回的图片应参与 no-progress 检测。

        连续返回相同 base64 的 ToolResultImage，验证 NO_PROGRESS_LIMIT 后
        context 中出现 "screen has not changed" hint。
        """

    @pytest.mark.asyncio
    async def test_tool_delay_ms_respected(self, tmp_path):
        """tool_delay_ms 配置应在工具执行间产生延迟。

        设置 tool_delay_ms=100，验证两次 tool 执行间至少间隔 ~100ms。
        （用 time.monotonic 计时，允许 ±50ms 误差）
        """

    @pytest.mark.asyncio
    async def test_save_memory_called_on_finished(self, tmp_path):
        """任务完成（finished）时应调用 memory.add()。

        传入 mock memory backend，验证 finished 后 memory.add 被调用，
        且传入的 messages 不含 base64 数据。
        """

    @pytest.mark.asyncio
    async def test_save_memory_failure_non_fatal(self, tmp_path):
        """memory.add() 抛异常不应中断任务流程。

        mock memory.add 抛出 RuntimeError，验证任务仍然正常 return RunResult。
        """

    @pytest.mark.asyncio
    async def test_memory_search_failure_non_fatal(self, tmp_path):
        """memory.search() 失败不影响主循环。

        mock memory.search 抛出 Exception，验证 loop.run 仍正常运行。
        """

    @pytest.mark.asyncio
    async def test_skills_injected_into_prompt(self, tmp_path):
        """config 中 skills_dirs 配置的技能应出现在 system prompt 中。

        创建临时 SKILL.md，配置 skills_dirs 指向它，
        验证 brain.chat 收到的 messages[0] system prompt 包含技能描述。
        """

    @pytest.mark.asyncio
    async def test_profile_changes_model(self, tmp_path):
        """--profile 参数应改变使用的模型。

        创建一个 profile JSON 覆盖 llm.model，
        验证 brain 初始化时使用了 profile 中的 model。
        注意：这个可能需要在 test_integration_assembly.py 中测。
        """
```

### P1：CLI 命令测试

**文件：** 新建 `tests/test_cli.py`

目前**零 CLI 命令测试**。使用 Typer 的 `CliRunner` 测试。

```python
from typer.testing import CliRunner
from see_agent.cli.main import app

runner = CliRunner()


class TestMCPCommands:

    def test_mcp_list_empty(self, tmp_path):
        """mcp list 无配置时应显示 'No MCP servers configured.'"""
        # patch load_config 返回空 mcp_servers
        result = runner.invoke(app, ["mcp", "list"])
        assert "No MCP servers" in result.output

    def test_mcp_list_shows_servers(self, tmp_path):
        """mcp list 有配置时应显示 server 名称和 command。"""

    def test_mcp_add(self, tmp_path):
        """mcp add <name> <command> 应写入 config.json。"""
        # patch CONFIG_PATH 到 tmp_path
        result = runner.invoke(app, ["mcp", "add", "test-server", "node", "--arg", "server.js"])
        assert result.exit_code == 0
        # 验证 config.json 中有 test-server

    def test_mcp_remove(self, tmp_path):
        """mcp remove <name> 应从 config.json 删除。"""

    def test_mcp_remove_nonexistent(self):
        """mcp remove 不存在的 server 应报错退出。"""
        result = runner.invoke(app, ["mcp", "remove", "ghost"])
        assert result.exit_code == 1


class TestSessionsCommands:

    def test_sessions_list_empty(self, tmp_path):
        """sessions list 无会话时应显示 'No sessions found.'"""
        result = runner.invoke(app, ["sessions", "list"])
        assert "No sessions found" in result.output

    def test_sessions_list_shows_sessions(self, tmp_path):
        """创建几个 session 后 list 应显示它们的 ID。"""

    def test_sessions_list_status_filter(self, tmp_path):
        """--status completed 只显示已完成的会话。"""

    def test_sessions_show_existing(self, tmp_path):
        """sessions show <id> 应打印 meta.json 内容。"""

    def test_sessions_show_nonexistent(self):
        """sessions show <bad_id> 应退出码 1。"""
        result = runner.invoke(app, ["sessions", "show", "nonexistent"])
        assert result.exit_code == 1

    def test_sessions_clean(self, tmp_path):
        """sessions clean --keep 0 应删除所有会话。"""

    def test_sessions_clean_empty_only(self, tmp_path):
        """sessions clean --empty 只删除无截图的会话。"""


class TestResumeCommand:

    def test_resume_nonexistent_session(self):
        """resume 不存在的 session 应报错退出。"""
        result = runner.invoke(app, ["resume", "nonexistent"])
        assert result.exit_code == 1

    def test_resume_last_no_sessions(self, tmp_path):
        """resume --last 无会话时应报错。"""


class TestProfileOption:

    def test_config_show_with_profile(self, tmp_path):
        """config show --profile <name> 应加载 profile 覆盖。"""

    def test_config_show_nonexistent_profile(self):
        """config show --profile ghost 应报错。"""
```

### P1：API sessions 路由测试

**文件：** 新建 `tests/test_api_sessions.py`

`see_agent/server/routes/sessions.py` 有 4 个接口，零测试。

```python
from fastapi.testclient import TestClient


class TestSessionsAPI:

    def test_list_sessions_empty(self, client):
        """GET /api/sessions 无会话时返回空列表。"""
        resp = client.get("/api/sessions")
        assert resp.status_code == 200
        assert resp.json()["sessions"] == []

    def test_list_sessions(self, client):
        """GET /api/sessions 应返回已创建的会话。"""

    def test_list_sessions_status_filter(self, client):
        """GET /api/sessions?status=completed 只返回已完成的。"""

    def test_get_session_detail(self, client):
        """GET /api/sessions/<id> 应返回 meta + 计数。"""

    def test_get_session_404(self, client):
        """GET /api/sessions/<bad_id> 应返回 404。"""

    def test_get_screenshot(self, client):
        """GET /api/sessions/<id>/screenshot/0 应返回 WebP 文件。"""

    def test_get_screenshot_404_no_step(self, client):
        """GET /api/sessions/<id>/screenshot/999 应返回 404。"""

    def test_delete_session(self, client):
        """DELETE /api/sessions/<id> 应删除会话。"""

    def test_delete_session_404(self, client):
        """DELETE /api/sessions/<bad_id> 应返回 404。"""

    def test_chat_with_session_id(self, client):
        """POST /api/chat {"task":"...", "session_id":"xxx"} 应复用已有会话。"""
```

### P2：MCP 集成路径

**文件：** `tests/test_mcp.py`（补充）

```python
class TestMCPIntegration:

    @pytest.mark.asyncio
    async def test_manager_connect_failure_non_fatal(self):
        """单个 MCP server 连接失败不应影响其他 server。

        配置 2 个 server，其中 1 个 command 不存在，
        验证 connect_all 不抛异常，另一个 server 正常注册工具。
        """

    @pytest.mark.asyncio
    async def test_tool_execution_failure_returns_error_text(self):
        """MCP 工具执行失败应返回包含错误信息的 ToolResult。"""

    @pytest.mark.asyncio
    async def test_disconnect_all_tolerates_errors(self):
        """disconnect_all 中单个 server 关闭失败不影响其他。"""

    def test_mcp_tool_name_format(self):
        """MCP 工具名格式应为 mcp__{server}__{tool}。"""
        # 已有类似测试，但可加更多边界情况
```

### P2：Memory 集成补充

**文件：** `tests/test_memory.py`（补充）

```python
class TestMem0MemoryInit:

    def test_mem0_import_error_message(self):
        """未安装 mem0ai 时应给出清晰的安装提示。"""
        # mock import 失败
        with pytest.raises(ImportError, match="mem0ai is required"):
            ...

    def test_mem0_search_exception_returns_empty(self):
        """mem0 search 内部异常应返回空列表，不抛出。"""

    def test_mem0_add_exception_silent(self):
        """mem0 add 内部异常应被吞掉（只 log），不抛出。"""
```

### P2：Skill loader 边界场景

**文件：** `tests/test_skill.py`（补充）

```python
class TestSkillLoaderEdgeCases:

    def test_nested_directory_skill(self, tmp_path):
        """嵌套目录下的 SKILL.md 应被 glob 扫描到。

        创建 tmp_path/category/my-skill/SKILL.md，
        验证 load_skills([tmp_path]) 能找到它。
        """

    def test_tilde_expansion(self, tmp_path):
        """路径中的 ~ 应被展开。"""

    def test_empty_body_skill(self, tmp_path):
        """frontmatter 正确但 body 为空的 SKILL.md 应正常加载。"""

    def test_unicode_skill_content(self, tmp_path):
        """包含中文和 emoji 的 SKILL.md 应正常解析。"""
```

### P2：Session 边界场景补充

**文件：** `tests/test_session.py`（补充）

```python
class TestSessionEdgeCases:

    def test_corrupted_meta_json_skipped_in_list(self, sessions_dir):
        """meta.json 内容损坏的 session 在 list() 时应被跳过，不崩溃。"""
        bad_dir = sessions_dir / "bad_session"
        bad_dir.mkdir()
        (bad_dir / "meta.json").write_text("{invalid json")
        sessions = SessionStore.list()
        # 不应崩溃，bad_session 被跳过

    def test_create_session_id_uniqueness(self, sessions_dir):
        """同一秒内创建的两个 session 应有不同 ID。"""
        s1 = SessionStore.create("task1", {"llm": {"model": "m"}})
        s2 = SessionStore.create("task2", {"llm": {"model": "m"}})
        assert s1.id != s2.id

    def test_append_message_unicode_roundtrip(self, sessions_dir):
        """中文和 emoji 内容应正确写入和读回 JSONL。"""
        session = SessionStore.create("测试", {"llm": {"model": "m"}})
        session.append_message({"type": "user_task", "text": "打开Safari搜索🍓"})
        messages = session.read_messages()
        assert messages[0]["text"] == "打开Safari搜索🍓"

    def test_clean_age_based(self, sessions_dir):
        """超过 keep_days 的 session 应被清理。

        需要 mock session 的 created_at 为 10 天前，
        然后调用 clean(keep_days=7) 验证被删除。
        """

    def test_restore_context_empty_session(self, sessions_dir):
        """空 JSONL 的 session restore_context 应返回只有 system prompt 的 context。"""
```

---

## 执行指南

1. **按 P0 → P1 → P2 顺序实现**
2. 每完成一个测试文件，跑 `bash scripts/check.sh` 确保不破坏现有测试
3. **只写测试，不改业务代码**。如果测试暴露了 bug，在测试文件的注释中标注 `# BUG:` 并跳过（`@pytest.mark.skip(reason="BUG: ...")"`），单独记录
4. 新建的测试文件：`tests/test_cli.py`、`tests/test_api_sessions.py`
5. 补充到已有文件的：`tests/test_loop.py`、`tests/test_mcp.py`、`tests/test_memory.py`、`tests/test_skill.py`、`tests/test_session.py`
6. CLI 测试用 `from typer.testing import CliRunner`，API 测试用 `from fastapi.testclient import TestClient`
7. 所有需要文件系统的测试用 `tmp_path` fixture + `patch` SESSIONS_DIR / CONFIG_PATH 等路径常量

---

## 预期结果

补全后测试数应从 161 增至约 **210-220**，覆盖：
- CLI 所有子命令的 happy path + error path
- API sessions 所有接口
- AgentLoop v2 新行为（multi-tool、screenshot tool、memory、no-screenshot warning）
- MCP/Memory/Skill 集成路径
- 边界场景和错误处理

---

## Part C：执行优先级

建议执行顺序：

```
1. Part A 功能缺口修复（先修 bug 再补测试）
   ├── A1 MCP 接入 AgentLoop（🔴 核心功能不可用）
   ├── A2 Mem0 配置完善（🔴 核心功能不可用）
   ├── A3 skills/ 目录自动创建（🟡 小改）
   ├── A4 memory/ 目录自动创建（🟡 小改）
   ├── A5 mcp list 健康检查（🟡 中改）
   ├── A6 mcp add 参数扩展（🟡 中改）
   └── A7 删除 screenshots/ dead code（🟡 小改）

2. Part B P0 测试（AgentLoop v2 新行为）

3. Part B P1 测试（CLI + API）

4. Part B P2 测试（边界场景）
```

每完成一步跑 `bash scripts/check.sh` 确保不破坏现有测试。
