# Bug Report: v2.5 Agent Team Review

> 生成日期：2026-03-09
> 基于 commit 19459d8 (phase 4) 的 Review
> 质量门禁：scripts/check.sh 4/4 通过，pytest 333 passed

---

## 🔴 P0: 严重问题（功能不工作）

### Bug 1: Bus → Agent 消息没有桥接

**现象**：agent A 调 `send_message(to="bob", content="xxx")`，消息写入 TeamBus 的 bob queue，但 bob 的 AgentLoop 永远收不到。

**根因**：`TeamManager._build_agent_loop` 创建了一个独立的 `asyncio.Queue[str]` 作为 `user_queue` 传给 AgentLoop：

```python
# manager.py L158
user_queue: asyncio.Queue[str] = asyncio.Queue()
```

而 `send_message` tool 写入的是 `TeamBus._queues[agent_id]`（类型 `asyncio.Queue[BusMessage]`）。这是**两个完全独立的 queue**，中间没有任何连接。

`AgentLoop._drain_user_queue` 只读 `self._user_queue`（空的那个），不知道 bus queue 的存在。

**修复方案**：

方案 A（推荐）：把 bus queue 的内容桥接到 agent 的 user_queue。在 `_drain_user_queue` 之前加一步 drain bus：

```python
# loop.py — 新增 bus drain
def __init__(self, ..., team_bus=None, agent_id=None):
    self._team_bus = team_bus
    self._agent_id = agent_id

def _drain_team_bus(self, ctx: ConversationContext) -> int:
    """Drain team bus messages into context."""
    if self._team_bus is None or self._agent_id is None:
        return 0
    messages = self._team_bus.drain(self._agent_id)
    for msg in messages:
        ctx.add_user_reply(f"[teammate {msg.sender}]: {msg.content}")
    return len(messages)
```

在 `_run_loop` 中调基模前调用：
```python
self._drain_team_bus(ctx)
self._drain_user_queue(ctx)
```

`TeamManager._build_agent_loop` 传入 bus：
```python
loop = AgentLoop(
    ...,
    team_bus=self._bus,
    agent_id=agent_id,
)
```

方案 B：直接把 bus 的 queue 作为 user_queue 传入，但需要类型适配（BusMessage → str），更 hacky。

---

### Bug 2: screen_lock 创建了但没使用

**现象**：多个 agent 可以同时操作屏幕（click、type_text、screenshot），导致互相干扰。

**根因**：`TeamManager.__init__` 创建了 `self._screen_lock = asyncio.Lock()`，但没传给任何 AgentLoop，也没在 tool 执行处加锁。

**修复方案**：

1. 把 screen_lock 传给每个 AgentLoop：
```python
loop = AgentLoop(
    ...,
    screen_lock=self._screen_lock,
)
```

2. AgentLoop 执行屏幕相关 tool 时加锁：
```python
SCREEN_TOOLS = {"screenshot", "click", "type_text", "scroll", "drag", "hotkey"}

async def _execute_tool(self, tool_name, args):
    if self._screen_lock and tool_name in SCREEN_TOOLS:
        async with self._screen_lock:
            return await self._registry.execute(tool_name, args)
    else:
        return await self._registry.execute(tool_name, args)
```

---

### Bug 3: Agent 全部并发跑同一个 task，无启动协调

**现象**：`TeamManager.run()` 让所有 agent（包括 worker）并发启动，每个 agent 收到同一个 task。Worker 没等 leader 分解任务就开始干活。

**根因**：
```python
# manager.py L88-97
tasks = [
    asyncio.create_task(_run_agent(aid, loop))
    for aid, loop in loops.items()
]
await asyncio.gather(*tasks, return_exceptions=True)
```

所有 agent 同时 `asyncio.create_task` 启动，没有先后顺序。

**修复方案**：

**方案 A（简单，推荐 v2.5）**：Leader 先跑，Worker 等待任务分配。

```python
async def run(self, task: str) -> TeamRunResult:
    self._board.create_task(title=task, description=task, created_by="system")

    # 1. 先启动 leader
    leader_id = self._team_def.leader
    if leader_id:
        leader_loop = loops[leader_id]
        leader_task = self._build_agent_task(leader_id, task)
        # Leader 跑完第一轮（分解任务、分配给 worker）
        results[leader_id] = await leader_loop.run(leader_task)

    # 2. 然后并发启动 worker
    worker_tasks = []
    for aid, loop in loops.items():
        if aid == leader_id:
            continue
        worker_tasks.append(asyncio.create_task(_run_agent(aid, loop)))
    await asyncio.gather(*worker_tasks, return_exceptions=True)
```

**方案 B（更灵活）**：Worker 启动后进入等待模式，轮询 TaskBoard 直到有自己的任务：

Worker 的初始 task 改为：
```
"你是 Team Worker。等待 leader 分配任务。用 list_tasks 查看任务列表，用 claim_task 领取分配给你的任务。"
```

这样 worker 启动后会自己 poll 任务列表，leader 创建任务后 worker 就能看到。但这更消耗 LLM 调用。

---

## ⚠️ P1: 中等问题（功能缺失）

### Bug 4: team_context 塞进 task 而不是 system prompt

**现象**：`_build_agent_task` 把 team context（团队信息、任务列表、协作规则）拼进 user message：

```python
def _build_agent_task(self, agent_id, task):
    team_context = self._build_team_context(agent_id)
    return f"{team_context}\n\n## 任务\n{task}"
```

**问题**：
- team_context 应该在 system prompt 里（`<TEAM_CONTEXT>` 段），`build_system_prompt` 已经支持 `team_context` 参数了
- 放在 user message 里会随着对话被 compact 掉，而 system prompt 一直保留

**修复**：TeamManager 构建 system prompt 时传入 team_context，task 只保留任务本身：

```python
def _build_agent_loop(self, agent_id):
    team_context = self._build_team_context(agent_id)
    # 把 team_context 存到 config 里，供 build_system_prompt 使用
    config["_team_context"] = team_context
    ...
```

或者给 AgentLoop 加一个 `team_context` 参数，在构建 system prompt 时传入。

---

### Bug 5: agent 配置的 tools.allowed/denied 没生效

**现象**：`agent.json` 可以配 `tools_config: { "denied": ["shell"] }`，ToolRegistry 有 `get_filtered` 方法，但 `_build_agent_loop` 注册完 tool 后没调用过滤。

**根因**：`_build_agent_loop` 里：

```python
registry = create_registry(eye)
self._register_team_tools(registry, agent_id)
# ← 这里应该根据 agent_def.tools_config 过滤，但没有
```

**修复**：

```python
# 加载 agent 配置
agent_def = AgentDefinition.load(agent_id)

# 注册所有 tool 后过滤
tools_cfg = agent_def.tools_config
if tools_cfg:
    filtered = registry.get_filtered(
        allowed=tools_cfg.get("allowed"),
        denied=tools_cfg.get("denied"),
    )
    # 替换 registry 内容或用 filtered 列表生成 tools_schema
```

注意：当前 `get_filtered` 返回 `list[Tool]` 但 AgentLoop 用的是整个 registry。需要改为：要么 registry 支持 `apply_filter()` 原地过滤，要么 AgentLoop 在生成 `tools_schema` 时用 `get_filtered` 的结果。

---

### Bug 6: quick chat 没有 stdin reader

**现象**：`quick_chat` 命令没有后台 stdin reader，agent 运行期间无法发送消息。之前修复的 `_stdin_reader_thread` 只在旧的 `chat` 命令里。

**修复**：把 `quick_chat` 里的循环改为和旧 `chat` 一样，加 `_stdin_reader_thread`：

```python
user_queue: asyncio.Queue[str] = asyncio.Queue()
loop = _build_components(config, ..., user_queue=user_queue)

stop_reader = threading.Event()
reader_thread = threading.Thread(
    target=_stdin_reader_thread, args=(user_queue, stop_reader), daemon=True,
)
reader_thread.start()
try:
    result = asyncio.run(loop.run(task, session_id=session.id))
finally:
    stop_reader.set()
    reader_thread.join(timeout=1.0)
```

---

### Bug 7: MCP enabled/disabled 过滤没生效

**现象**：`agent.json` 可以配 `mcp_config: { "enabled": ["tavily"] }`，但 `_build_agent_loop` 里没使用这个配置过滤 MCP server。所有 agent 拿到全部 MCP tool。

**修复**：`_build_agent_loop` 里在 MCP 连接前根据 agent 配置过滤：

```python
mcp_cfg = agent_def.mcp_config
mcp_servers = config.get("mcp_servers", {})
if mcp_cfg.get("enabled"):
    mcp_servers = {k: v for k, v in mcp_servers.items() if k in mcp_cfg["enabled"]}
elif mcp_cfg.get("disabled"):
    mcp_servers = {k: v for k, v in mcp_servers.items() if k not in mcp_cfg["disabled"]}
```

---

## 📝 P2: 小问题

### Bug 8: 旧的 `chat` 和 `run` 命令没删

PRD 说 `see-agent chat` → `see-agent quick chat`，`see-agent run` → `see-agent quick run`。但旧命令还在，和新命令重复。建议：要么删掉旧命令，要么把它们改为 alias 到 quick 版本。

### Bug 9: FileMemory.search 关键词重叠效果差

`search` 用简单的词重叠打分，中文分词会失败（中文不能 `.split()` 分词）。作为默认实现可以先接受，但至少要加个注释标明是 placeholder。

### Bug 10: _build_agent_loop 每次创建新的 MacEye 和 Brain

```python
eye = MacEye()
brain = OpenAIBrain(...)
```

每个 agent 都创建独立的 eye 和 brain 实例。MacEye 应该共享（只有一个屏幕），Brain 可以独立（不同 agent 可能用不同模型）。MacEye 创建多次不会出错但浪费资源。

---

## 执行优先级

| Bug | 优先级 | 预估改动 |
|-----|--------|---------|
| Bug 1: Bus→Agent 桥接 | 🔴 P0 | ~30 行（AgentLoop 加 team_bus + drain） |
| Bug 2: screen_lock 使用 | 🔴 P0 | ~20 行（传入 lock + tool 执行时加锁） |
| Bug 3: 启动协调 | 🔴 P0 | ~30 行（leader 先跑 or worker 等待模式） |
| Bug 4: team_context → system prompt | ⚠️ P1 | ~15 行 |
| Bug 5: tools filter 生效 | ⚠️ P1 | ~20 行 |
| Bug 6: quick chat stdin reader | ⚠️ P1 | ~15 行（复制已有逻辑） |
| Bug 7: MCP filter 生效 | ⚠️ P1 | ~10 行 |
| Bug 8: 删旧命令 | 📝 P2 | ~50 行删除 |
| Bug 9: 中文分词 | 📝 P2 | 加注释即可 |
| Bug 10: MacEye 共享 | 📝 P2 | ~5 行 |

**建议执行顺序**：Bug 1 + 2 + 3（三个 P0 一起改）→ Bug 4 + 5 + 7（配置生效）→ Bug 6（stdin）→ Bug 8/9/10

做完跑 `scripts/check.sh` 确保全过。
