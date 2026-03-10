# Report: 旧代码清理 + 兼容残留根除

> 日期：2026-03-10 | 作者：蓝莓🫐 | 基于 commit af738f9 (v3.1)
>
> **原则：不要做向后兼容。PRD 说删就删干净，不留 fallback。跑不起来会有测试告诉你。**

---

## 一、测试污染真实目录（紧急）

### 问题

每次跑 `scripts/check.sh`，`~/.see-agent/sessions/` 和 `~/.see-agent/memory/` 会被重新创建。管理者已经手动删过多次，每次跑测试又出现。

### 根因

`conftest.py` 的 `isolate_workspace` patch 了 `see_agent.config.SESSIONS_DIR`，但 `session/store.py` 在 import 时拷贝了一份引用：

```python
# session/store.py
from see_agent.config import SESSIONS_DIR  # ← 拷贝了引用
```

conftest patch `see_agent.config.SESSIONS_DIR` 不影响 `see_agent.session.store.SESSIONS_DIR`。某些测试间接触发 `SessionStore.create()` → fallback 到真实的 `~/.see-agent/sessions/` → `mkdir(parents=True)` 连带创建父目录。

`file_backend.py` 同理：`from see_agent.config import WORKSPACE_DIR` 后 fallback 到 `WORKSPACE_DIR / "memory"`。

### 修复

**方案 A（止血）**：conftest.py 补两行 patch：

```python
patches = {
    # ... 现有的 ...
    "see_agent.session.store.SESSIONS_DIR": ws / "sessions",
    "see_agent.memory.file_backend.WORKSPACE_DIR": ws,
}
```

**方案 B（根治，推荐）**：删掉 fallback，见下文第二节。

---

## 二、全局 fallback 根除

所有"参数可选 → fallback 到全局目录"的设计都应该改为**必传参数**。

### 2.1 删除 `SESSIONS_DIR` 常量 + fallback

**改动**：

1. `config.py` — 删除 `SESSIONS_DIR = WORKSPACE_DIR / "sessions"`
2. `session/store.py` — `root_dir` 参数从 `Path | None = None` 改为 `Path`（必传），删除所有 `if root_dir is not None else SESSIONS_DIR` 的 fallback。涉及 5 个方法：`create`、`load`、`list`、`delete`、`clean`
3. 所有调用方必须传 `root_dir`，不传的地方 pyright 会报错

**确认所有调用方**：
- `agent/loop.py` — 已经传了 `self._session_root`（来自 worker 或 team manager）✅
- `server/routes/sessions.py` — **没传**，见第三节要删
- `server/routes/chat.py` — **没传**，见第三节要删

### 2.2 删除 `file_backend.py` 的全局 memory fallback

**改动**：

```python
# 现在
def __init__(self, memory_dir: Path | None = None) -> None:
    if memory_dir is None:
        memory_dir = WORKSPACE_DIR / "memory"   # ← 删掉这个 fallback

# 改为
def __init__(self, memory_dir: Path) -> None:  # 必传
```

调用方（`team/manager.py:244`）已经传了 `memory_dir=agent_base / "memory"` ✅。

### 2.3 删除 `mem0_backend` 全局 storage_path

`config.py` DEFAULT_CONFIG 里 `"storage_path": "~/.see-agent/memory/qdrant"` — 这个全局路径不应该在 v3.1 存在。mem0 的 storage 应该归 agent。

**改动**：DEFAULT_CONFIG 里的 `storage_path` 改为空字符串 `""`，由 team manager 在启动时按 agent 路径覆盖。

---

## 三、删除 v2 时代的单 agent 路由

以下 4 个路由文件/模块是 v2 单 agent 时代的产物，v3.1 team 模式下**没有入口调用**，但仍然注册在 app.py 中，白占端口且 fallback 到全局目录。

### 3.1 `chat.py`（261 行）— 删除

**是什么**：`POST /api/chat` + `POST /api/chat/{task_id}/message` — 单 agent 运行入口。

**为什么删**：
- 创建 AgentLoop 时不传 `session_root` → fallback 到全局 `SESSIONS_DIR`
- 不传 `agent_id` → 没有 team 概念
- v3.1 的 agent 全部通过 team 启动（`POST /api/teams/{id}/run`）

**改动**：删除 `see_agent/server/routes/chat.py`，`app.py` 去掉 `import chat` 和 `include_router(chat.router)`。

### 3.2 `task.py`（36 行）— 删除

**是什么**：`GET /api/task/{task_id}` — 查询单 agent 任务状态。

**为什么删**：配合 `chat.py` 使用的，chat 删了这个也没用。v3.1 的任务状态通过 `GET /api/teams/{id}/status` 查看。

**改动**：删除 `see_agent/server/routes/task.py`，`app.py` 去掉注册。

### 3.3 `ws.py`（67 行）— 删除

**是什么**：`WS /api/ws/{task_id}` — 单 agent 步骤流式推送。

**为什么删**：配合 `chat.py` 使用的。v3.1 的 WebSocket 应该是 team 级别的（`/api/ws/team/{id}/messages` 和 `/api/ws/team/{id}/tasks`，PRD 里定义了但还没实现）。

**改动**：删除 `see_agent/server/routes/ws.py`，`app.py` 去掉注册。

### 3.4 `sessions.py`（95 行）— 删除

**是什么**：`GET /api/sessions` + `GET /api/sessions/{id}` + `DELETE /api/sessions/{id}` — 列出/查看/删除全局 session。

**为什么删**：查的是全局 `SESSIONS_DIR`，v3.1 session 归属 agent（在 `teams/{id}/agents/{aid}/sessions/` 下）。前端已经有 team 日志页面。

**改动**：删除 `see_agent/server/routes/sessions.py`，`app.py` 去掉注册。

### 3.5 `models.py` 清理

删除 `chat.py` 后，`models.py` 里的以下模型不再有人使用：
- `ChatRequest` / `ChatResponse` — chat 路由用的
- `TaskStatus` — task 路由用的
- `UserMessageRequest` — chat inject message 用的
- `StepMessage` — ws 路由用的

**改动**：整个 `see_agent/server/models.py` 删除（或者只保留有其他地方用的模型，pyright 会告诉你哪些还在用）。

---

## 四、DEFAULT_CONFIG 清理

| 字段 | 当前值 | 建议 | 理由 |
|------|--------|------|------|
| `soul_path` | `None` | **删除** | v3.1 soul 归属 agent（agent.json 里），全局 soul_path 无意义 |
| `show_overlay` | `True` | **删除** | v3.1 agent 在子进程中，没有 GUI 线程，overlay 无法工作 |
| `context_engine` | `"legacy"` | **改为 `"default"`** | "legacy" 这个命名暗示有新的替代方案，但实际只有这一个实现。改名去掉误导 |
| `memory.mem0.storage_path` | `"~/.see-agent/memory/qdrant"` | **改为 `""`** | 全局路径不应存在，mem0 storage 应归 agent 目录 |

---

## 五、`overlay/` 模块评估（514 行）

`see_agent/overlay/mac_overlay.py` 用 AppKit 在屏幕上画覆盖动画（点击位置、工具名称等）。

**v3.1 现状**：
- agent 在子进程中运行，没有 GUI 线程 → overlay 不能从子进程调用
- `AgentLoop.__init__` 仍然接受 `overlay` 参数
- `worker.py` 创建 AgentLoop 时**不传 overlay**（正确）
- `loop.py:579,605,672` 有 `if self._overlay:` 检查（不会崩，但是死代码）

**建议**：暂时不删 overlay 模块，但：
1. `AgentLoop.__init__` 删除 `overlay` 参数
2. `loop.py` 删除所有 `if self._overlay` 分支（约 20 行）
3. v3.2 如果需要 overlay 效果，改为子进程通过 UDS 通知主进程画 overlay

---

## 六、`context.py` 的 legacy screenshot 路径

`add_tool_result()` 同时支持两种方式传截图：
1. `ToolResult.images`（新方式）
2. `screenshot_b64` 参数（旧方式，注释写 "legacy path"）

搜索发现 `loop.py` 只用 `ToolResult` 方式，`screenshot_b64` 参数没有任何调用方了。

**建议**：`add_tool_result()` 删除 `screenshot_b64` / `detail` / `mime_type` 参数，只保留 `ToolResult` 路径。

---

## 七、`tool.py` 的 `str` 返回值兼容

`execute()` 返回类型是 `str | ToolResult`，注释写 "backward-compatible"。

搜索所有 tool 实现，大部分已经返回 `ToolResult`。如果还有返回 `str` 的，改成 `ToolResult(text=...)` 然后统一返回类型为 `ToolResult`。

**建议**：
1. `execute()` 返回类型改为 `ToolResult`
2. `loop.py` 里不再需要 `isinstance(result, str)` 的判断
3. 所有 tool 实现统一返回 `ToolResult`

---

## 执行清单

| # | 任务 | 优先级 | 涉及文件 |
|---|------|--------|---------|
| 1 | 删 `SESSIONS_DIR` 常量 + 所有 fallback | P0 | config.py, session/store.py |
| 2 | `file_backend.py` memory_dir 改为必传 | P0 | memory/file_backend.py |
| 3 | conftest.py 补 patch（如果选止血方案） | P0 | tests/conftest.py |
| 4 | 删 `chat.py` + `task.py` + `ws.py` + `sessions.py` | P0 | server/routes/ 4 个文件, app.py |
| 5 | 删或清理 `models.py` | P1 | server/models.py |
| 6 | DEFAULT_CONFIG 清理（soul_path, show_overlay, context_engine, storage_path） | P1 | config.py |
| 7 | AgentLoop 删 overlay 参数 + 死代码 | P1 | agent/loop.py |
| 8 | context.py 删 legacy screenshot 参数 | P2 | agent/context.py |
| 9 | tool.py execute() 返回类型统一为 ToolResult | P2 | hand/tool.py + 所有 tool 实现 |
| 10 | mem0 storage_path 改空 | P2 | config.py |

做完跑 `scripts/check.sh`，9 步全过即可。pyright 会帮你找到所有因删除 fallback 而需要同步修改的调用方。
