# Bug Report: session.log 为空 + Memory/MCP 依赖问题

## Bug 1 🔴: session.log 始终为空（日志分层失效）

### 现象
`sessions/<id>/session.log` 文件始终 0 字节，运行时日志（Step、Tool call、Thought、截图捕获等）哪儿都没记录。

### 根因
`config.py` 的 `setup_logging()` 第 202-206 行：

```python
# config.py L202-206
for _name in (
    "see_agent.agent", "see_agent.brain",
    "see_agent.eye", "see_agent.hand",
):
    logging.getLogger(_name).setLevel(logging.WARNING)
```

这把 session 相关 logger 的 **level 设为 WARNING**。

然后 `session/store.py` 的 `Session.setup_logging()` 给这些 logger 加了 FileHandler（level=DEBUG），但 **logger 自身的 level 仍然是 WARNING**。Python logging 的机制是：消息先过 logger level 过滤，再到 handler。所以 DEBUG/INFO 消息在 logger 层就被拦截了，根本到不了 handler。

运行时日志绑大多数是 INFO/DEBUG 级别：
- `loop.py`: "=== Step X ==="、"Thought: xxx"、"Tool call: xxx"、"Scaled args" 都是 INFO
- `context.py`: "Added user task"、"Added assistant message" 都是 DEBUG
- `brain/openai_client.py`: LLM request 摘要是 INFO
- `eye/mac.py`: 截图捕获是 DEBUG

全被拦了。只有 WARNING/ERROR 才能通过，正常运行时几乎没有。

### 修复方案

**`session/store.py` — `setup_logging()` 里降低 logger level：**

```python
def setup_logging(self) -> None:
    handler = logging.FileHandler(self.dir / "session.log", encoding="utf-8")
    handler.setLevel(logging.DEBUG)
    handler.setFormatter(logging.Formatter(
        "%(asctime)s  %(levelname)-8s  %(name)s  %(message)s",
        datefmt="%H:%M:%S",
    ))
    self._log_handler = handler
    self._original_levels: dict[str, int] = {}  # ← 新增：保存原始 level
    for name in self._SESSION_LOGGERS:
        lgr = logging.getLogger(name)
        self._original_levels[name] = lgr.level  # ← 保存
        lgr.setLevel(logging.DEBUG)               # ← 降为 DEBUG
        lgr.addHandler(handler)
```

**`teardown_logging()` 里恢复原始 level：**

```python
def teardown_logging(self) -> None:
    handler = self._log_handler
    if handler is None:
        return
    for name in self._SESSION_LOGGERS:
        lgr = logging.getLogger(name)
        lgr.removeHandler(handler)
        if hasattr(self, '_original_levels') and name in self._original_levels:
            lgr.setLevel(self._original_levels[name])  # ← 恢复
    handler.close()
    self._log_handler = None
```

这样：
- session 运行期间：DEBUG/INFO 写入 `session.log`，但全局日志文件因 handler level 仍然只收 WARNING+（全局 handler 的 filter 在 `config.py` 里单独控制）
- session 结束后：恢复 WARNING level，不影响后续全局日志行为

### 验证方式
1. 跑 `see-agent chat`，执行一个任务
2. 检查 `session.log` 不为空
3. 检查 `session.log` 包含 "Step"、"Tool call"、"Thought" 等 INFO 级别日志
4. 检查全局 `logs/` 仍然只有生命周期日志（不含 session 运行时日志）

---

## Bug 2 ⚠️: memory 目录为空（mem0 初始化静默失败的用户体验问题）

### 现象
`~/.see-agent/memory/` 目录为空。config.json 里 `memory.enabled: true`，但 Mem0 从未写入数据。

### 直接原因
之前 `mem0ai` 没装。运行 `see-agent setup install` 后，mem0ai 已安装（1.0.5），下次启动应该能正常初始化。**这个 bug 本身会在重新启动后自行解决。**

### 但代码层面有改进空间

1. **CLI 启动时应明确告知 memory 状态**：目前 mem0 初始化失败只打一行 Warning 到 stderr，用户很容易忽略。建议在 `see-agent chat` / `see-agent run` 启动时显示功能状态摘要：

```
🤖 see-agent v0.1 已启动
📋 会话 ID: 20260309_xxx
✅ Memory: active (mem0, qdrant)
✅ MCP: tavily (3 tools)
❌ MCP: some-server (connection failed)
```

2. **`see-agent setup check` 应检测 config 与依赖的一致性**：config 里开了 memory 但没装 mem0，应该报红而不是只说 "not installed"。

### 思路
- `main.py` 的 `_build_components()` 里，memory 和 mcp 初始化完成后，把状态汇总打印出来
- 失败时用 `typer.echo` + 醒目颜色（`typer.style`）提示，不只是 logging.warning

---

## Bug 3 ⚠️: MCP 未安装时 system prompt 里仍列出 MCP tool 的 skill 描述（幻觉源）

### 现象
MCP tavily server 连接失败（mcp 包未装），tavily 工具没有注册到 tool registry。但 system prompt 的 `<SKILLS>` 段仍然包含 `tavily-search` 的描述。导致 agent 以为自己能用 tavily，产生幻觉。

### 根因
Skills 和 MCP tools 是两个独立系统：
- `skill/loader.py` 从 `~/.openclaw/skills/` 加载 skill 描述 → 注入 `<SKILLS>` 到 system prompt
- `hand/mcp.py` 从 config 的 `mcp_servers` 连接 MCP server → 注册 tool 到 registry

tavily 同时存在于两个地方：作为 openclaw skill（有描述）和作为 MCP server（提供实际工具）。MCP 连接失败后工具没注册，但 skill 描述仍然被加载了。

### 修复思路
两种方案选一个：

**方案 A（简单）**：`build_system_prompt()` 时检查 skill 关联的工具是否真正注册成功，未注册的 skill 不写入 `<SKILLS>`。需要 skill 和 tool registry 之间有关联信息。

**方案 B（更实际）**：启动时如果 MCP server 连接失败，在 `<SKILLS>` 段对应 skill 后面加标注 `⚠️ (unavailable: MCP server not connected)`，让 agent 知道这个能力不可用。

---

## 执行优先级

| Bug | 优先级 | 说明 |
|-----|--------|------|
| Bug 1: session.log 为空 | 🔴 P0 | 日志分层是核心功能，当前完全失效 |
| Bug 3: skill 幻觉 | ⚠️ P1 | 会导致 agent 尝试不存在的工具，浪费步数 |
| Bug 2: memory 启动提示 | ⚠️ P2 | UX 改进，不影响功能 |

做完跑 `scripts/check.sh` 确保全过。
