# v2 优化 Report：System Prompt 记录 + 一键安装 + 日志分层

> 日期：2026-03-06
> 版本：v2 ReAct Agent（commit ba0a396）
> 前置：211 个测试全过，check.sh 4/4 pass

---

## 1. Session 内记录 System Prompt

### 问题

- system prompt 每次 `AgentLoop.run()` 时在内存中动态拼接（`build_system_prompt()`），不持久化
- 日志里的 DEBUG `LLM request messages` 把 prompt 截断到 500 字符，看不到完整内容
- resume 前后 prompt 可能变化（装了新 skill、memory 数据变了、换了 profile），无处可查

### 方案

在 session 目录下新增 `system_prompt_log.md`，**每次 LLM 调用前如果 system prompt 变了就追加写入**。

#### 文件位置

```
sessions/20260306_173639_fc0f0a/
├── meta.json
├── messages.jsonl
├── system_prompt_log.md     ← 新增
├── session.log              ← 新增（见第 3 节）
└── screenshots/
```

#### 写入时机

在 `AgentLoop.run()` 中，构建 system prompt 之后、首次 LLM 调用之前：

```python
system_prompt = build_system_prompt(config, skills=skills, memory_block=memory_block)

# 记录 system prompt（仅在变化时追加）
session.log_system_prompt(system_prompt)
```

#### Session 新增方法

```python
class Session:
    _last_prompt_hash: str | None = None

    def log_system_prompt(self, prompt: str) -> None:
        """追加 system prompt 到 system_prompt_log.md（仅在内容变化时写入）。"""
        import hashlib
        h = hashlib.md5(prompt.encode()).hexdigest()
        if h == self._last_prompt_hash:
            return
        self._last_prompt_hash = h

        log_path = self.dir / "system_prompt_log.md"
        with open(log_path, "a", encoding="utf-8") as f:
            from datetime import datetime
            ts = datetime.now().isoformat(timespec="seconds")
            f.write(f"\n---\n## {ts}\n\n")
            f.write(prompt)
            f.write("\n")
```

#### 产出文件示例

```markdown
---
## 2026-03-06T17:36:39+08:00

你是一个能操作 Mac 电脑的 AI 助手...

<RULES>
1. 你可以在一次回复中调用多个工具...
</RULES>

<SKILLS>
- **Agent Browser**: A fast Rust-based headless browser automation CLI...
- **tavily-search**: Web search via Tavily API...
</SKILLS>

<PERSONALITY>
你是一个高效、谨慎又机灵的桌面操作助手。
</PERSONALITY>

---
## 2026-03-06T18:15:02+08:00

（resume 后重新生成，此时多了一个新 skill）

你是一个能操作 Mac 电脑的 AI 助手...

<SKILLS>
- **Agent Browser**: ...
- **tavily-search**: ...
- **dingtalk**: 操作钉钉桌面客户端    ← 新增
</SKILLS>
```

#### 注意

- `system_prompt_log.md` 是只追加的审计日志，**不参与任何运行时逻辑**
- 代码构建 prompt 的方式不变，不从这个文件读
- messages.jsonl 中**不存** system 消息（避免重复）

---

## 2. 一键安装依赖 CLI

### 问题

- `mem0ai` 和 `mcp` 是可选依赖，需要 `pip install see-agent[memory]` 和 `pip install see-agent[mcp]` 分别安装
- 新用户不知道这些可选功能存在，也不知道怎么装
- 启动时可选功能初始化失败只在日志里 WARNING，CLI 不提示
- 项目已用 `uv` 管理依赖（有 `uv.lock`，venv 由 uv 创建）

### 方案

#### 2.1 pyproject.toml 新增 `all` 分组

```toml
[project.optional-dependencies]
dev = [
    "pytest>=8.0.0",
    "pytest-asyncio>=0.25.0",
    "ruff>=0.8.0",
]
memory = [
    "mem0ai>=1.0.0",
]
mcp = [
    "mcp>=1.0.0",
]
all = [
    "mem0ai>=1.0.0",
    "mcp>=1.0.0",
]
```

#### 2.2 新增 `see-agent setup` CLI 命令

```python
setup_app = typer.Typer(name="setup", help="Install dependencies and verify environment.")
app.add_typer(setup_app, name="setup")


@setup_app.command("install")
def setup_install(
    full: bool = typer.Option(False, "--full", help="Install all optional dependencies."),
    memory: bool = typer.Option(False, "--memory", help="Install memory (mem0) support."),
    mcp: bool = typer.Option(False, "--mcp", help="Install MCP support."),
    dev: bool = typer.Option(False, "--dev", help="Install development dependencies."),
) -> None:
    """Install see-agent dependencies."""
    import subprocess
    import shutil

    # 检测包管理器
    uv = shutil.which("uv")
    pip_cmd = [uv, "pip", "install"] if uv else ["pip", "install"]

    extras: list[str] = []
    if full:
        extras.append("all")
    else:
        if memory:
            extras.append("memory")
        if mcp:
            extras.append("mcp")
    if dev:
        extras.append("dev")

    if not extras:
        extras.append("all")  # 默认全装

    extra_str = ",".join(extras)
    cmd = [*pip_cmd, "-e", f".[{extra_str}]"]

    typer.echo(f"📦 Installing: see-agent[{extra_str}]")
    typer.echo(f"   Command: {' '.join(cmd)}\n")

    result = subprocess.run(cmd, cwd=_project_root())
    if result.returncode == 0:
        typer.echo("\n✅ Dependencies installed successfully.")
    else:
        typer.echo("\n❌ Installation failed.", err=True)
        raise typer.Exit(code=1)


@setup_app.command("check")
def setup_check() -> None:
    """Check which optional features are available."""
    checks = [
        ("mem0ai", "memory", "see-agent setup install --memory"),
        ("mcp", "mcp", "see-agent setup install --mcp"),
    ]
    all_ok = True
    for module, feature, fix_cmd in checks:
        try:
            __import__(module)
            typer.echo(f"  ✅ {feature}: installed")
        except ImportError:
            typer.echo(f"  ❌ {feature}: not installed → {fix_cmd}")
            all_ok = False

    # 检查配置
    config = load_config()
    mem_enabled = config.get("memory", {}).get("enabled", False)
    mcp_servers = config.get("mcp_servers", {})

    if mem_enabled:
        try:
            __import__("mem0")
            typer.echo(f"  ✅ memory: enabled and ready")
        except ImportError:
            typer.echo(f"  ⚠️  memory: enabled in config but mem0ai not installed")
            all_ok = False

    if mcp_servers:
        try:
            __import__("mcp")
            typer.echo(f"  ✅ mcp: {len(mcp_servers)} server(s) configured and ready")
        except ImportError:
            typer.echo(f"  ⚠️  mcp: {len(mcp_servers)} server(s) configured but mcp not installed")
            all_ok = False

    if all_ok:
        typer.echo("\n🎉 All features ready.")
    else:
        typer.echo("\n💡 Run `see-agent setup install` to install all dependencies.")
```

#### 2.3 CLI 使用方式

```bash
# 全功能安装（默认）
see-agent setup install

# 只装某个功能
see-agent setup install --memory
see-agent setup install --mcp
see-agent setup install --dev

# 检查当前环境
see-agent setup check

# 输出示例：
#   ✅ memory: installed
#   ❌ mcp: not installed → see-agent setup install --mcp
#   ⚠️  memory: enabled in config but mem0ai not installed
#
#   💡 Run `see-agent setup install` to install all dependencies.
```

#### 2.4 启动时提示（补充）

在 `_build_components()` 中，当可选功能初始化失败时，除了日志 WARNING，在 CLI 也输出一行提示：

```python
# Memory
if mem_cfg.get("enabled", False):
    try:
        from see_agent.memory.mem0_backend import Mem0Memory
        memory = Mem0Memory(config=mem_cfg.get("mem0") or None)
    except ImportError:
        typer.echo("⚠️  Memory enabled but mem0ai not installed. Run: see-agent setup install --memory")
    except Exception:
        logger.warning("Failed to initialize memory backend", exc_info=True)

# MCP
if mcp_servers:
    try:
        from see_agent.hand.mcp import MCPManager
        mcp_manager = MCPManager(mcp_servers, global_env=config.get("env", {}))
    except ImportError:
        typer.echo("⚠️  MCP servers configured but mcp not installed. Run: see-agent setup install --mcp")
    except Exception:
        logger.warning("Failed to initialize MCP", exc_info=True)
```

---

## 3. 日志分层

### 问题

当前所有日志混在 `logs/YYYY-MM-DD.log` 一个文件里，一天 ~2500 行，包含：
- 全局生命周期（启动、配置、tool 注册）— 少量，有用
- session 运行时（Step、Thought、Tool call、LLM request、截图、坐标缩放）— 大量，只跟特定 session 有关
- PIL 图片解码 DEBUG — 788 行垃圾，完全无用
- httpx HTTP 请求 — 跟 session 相关

### 方案

#### 3.1 新增 session 级日志

每个 session 有自己的 `session.log`：

```
sessions/20260306_173639_fc0f0a/
├── session.log              ← session 专属日志
├── ...
```

#### 3.2 日志路由规则

| 来源 | 级别 | 写入位置 |
|------|------|---------|
| `cli.main`（启动/配置/初始化结果） | INFO+ | 全局 `logs/` |
| `config`（workspace 创建） | INFO+ | 全局 `logs/` |
| `session.store`（session 创建/加载/完成） | INFO+ | 全局 `logs/` |
| `hand.tool`（tool 注册） | INFO | 全局 `logs/`（仅启动时） |
| `skill.loader`（skill 加载） | INFO | 全局 `logs/` |
| `hand.mcp`（MCP 连接成功/失败） | INFO+ | 全局 `logs/` |
| 所有 WARNING/ERROR | * | 全局 `logs/` |
| `agent.loop`（Step、Thought、Tool call、坐标缩放） | ALL | session `session.log` |
| `agent.context`（Added message） | ALL | session `session.log` |
| `brain.openai_client`（LLM request 摘要） | ALL | session `session.log` |
| `eye.mac`（截图捕获） | ALL | session `session.log` |
| `eye.scaling`（缩放计算） | ALL | session `session.log` |
| `hand.tools.*`（工具执行详情） | ALL | session `session.log` |
| `httpx`（HTTP 请求） | ALL | session `session.log` |
| `PIL.*` | WARNING+ | 全局（实质上不再有输出） |

#### 3.3 实现方式

##### Session 创建日志 handler

```python
class Session:
    def __init__(self, ...):
        ...
        self._log_handler: logging.FileHandler | None = None

    def setup_logging(self) -> logging.FileHandler:
        """创建 session 级别的日志 handler，返回它以便后续移除。"""
        log_path = self.dir / "session.log"
        handler = logging.FileHandler(log_path, encoding="utf-8")
        handler.setLevel(logging.DEBUG)
        handler.setFormatter(logging.Formatter(
            "%(asctime)s  %(levelname)-8s  %(name)s  %(message)s",
            datefmt="%H:%M:%S",
        ))
        # 只捕获 session 相关的 logger
        for name in [
            "see_agent.agent",
            "see_agent.brain",
            "see_agent.eye",
            "see_agent.hand",
            "httpx",
        ]:
            logging.getLogger(name).addHandler(handler)
        self._log_handler = handler
        return handler

    def teardown_logging(self) -> None:
        """移除 session 日志 handler。"""
        if self._log_handler is None:
            return
        for name in [
            "see_agent.agent",
            "see_agent.brain",
            "see_agent.eye",
            "see_agent.hand",
            "httpx",
        ]:
            logging.getLogger(name).removeHandler(self._log_handler)
        self._log_handler.close()
        self._log_handler = None
```

##### AgentLoop 中使用

```python
async def run(self, task, session_id=None):
    # 创建/加载 session
    session = ...

    # 挂载 session 日志
    session.setup_logging()

    try:
        # ... 主循环 ...
    finally:
        session.teardown_logging()
```

##### 全局日志过滤

修改 `setup_logging()`，让全局 file handler 只记录全局事件：

```python
def setup_logging() -> None:
    ...
    # 全局日志只记录 WARNING+ 的 session 相关日志
    # （session.setup_logging 会给这些 logger 加自己的 handler）
    for name in [
        "see_agent.agent",
        "see_agent.brain",
        "see_agent.eye",
        "see_agent.hand",
    ]:
        logging.getLogger(name).setLevel(logging.WARNING)  # 全局只记WARNING+

    # 彻底静默 PIL
    logging.getLogger("PIL").setLevel(logging.WARNING)

    # httpx 只在全局记 WARNING+
    logging.getLogger("httpx").setLevel(logging.WARNING)
```

当 session.setup_logging() 给这些 logger 加了自己的 handler 后，DEBUG/INFO 会写到 session.log 里，WARNING+ 同时写到全局日志和 session.log。

#### 3.4 改造后的日志效果

**全局 `logs/2026-03-06.log`**（一天几十行）：

```
17:36:39  INFO   see_agent.cli.main        see-agent started (model=claude-opus-4-6)
17:36:39  INFO   see_agent.skill.loader    Loaded 4 skills from 2 dirs
17:36:39  WARN   see_agent.cli.main        Memory: mem0ai not installed, skipping
17:36:39  WARN   see_agent.hand.mcp        MCP: mcp package not installed
17:36:39  INFO   see_agent.session.store   Session 20260306_173639_fc0f0a created
17:37:08  INFO   see_agent.session.store   Session 20260306_173639_fc0f0a completed (1 step, 7.2s)
17:38:55  INFO   see_agent.session.store   Session 20260306_173855_abc123 created
17:39:40  INFO   see_agent.session.store   Session 20260306_173855_abc123 completed (5 steps, 45.0s)
```

**Session `sessions/20260306_173639_fc0f0a/session.log`**（该 session 的完整记录）：

```
17:37:01  INFO   see_agent.agent.loop       Session 20260306_173639_fc0f0a — task dir: ...
17:37:01  INFO   see_agent.eye.mac          Captured screenshot: 1366x768 (detail=high)
17:37:01  INFO   see_agent.brain.openai_client  LLM request: model=claude-opus-4-6 messages=2 tools=10
17:37:01  INFO   httpx                       HTTP POST https://matrixllm.alipay.com/v1/... 200
17:37:08  INFO   see_agent.agent.loop       Thought: 用户问我是谁...
17:37:08  INFO   see_agent.agent.loop       Tool call: finished(summary="...")
17:37:08  INFO   see_agent.agent.loop       Task finished: ...
```

#### 3.5 砍掉的日志

| 内容 | 原始 | 改造后 |
|------|------|--------|
| `brain.openai_client` DEBUG `LLM request messages` | 每次调用一行万字 JSON | **删掉**。完整 prompt 看 `system_prompt_log.md`，对话看 `messages.jsonl` |
| `agent.context` DEBUG `Added user task / assistant` | 每条消息一行 | **保留但只在 session.log** |
| `PIL.*` DEBUG 图片解码 | 788 行 | **降级到 WARNING，实质消失** |
| `hand.tool` DEBUG `Registered tool: xxx` | 启动时每个 tool 一行 | **保留在全局日志，改为一行汇总** |

---

## 执行顺序

```
1. system_prompt_log.md
   ├── Session 新增 log_system_prompt() 方法
   └── AgentLoop.run() 中 build_system_prompt 后调用

2. 一键安装 CLI
   ├── pyproject.toml 加 [all] 分组
   ├── CLI 新增 see-agent setup install / check
   └── _build_components() 启动时提示缺失依赖

3. 日志分层
   ├── Session 新增 setup_logging() / teardown_logging()
   ├── AgentLoop.run() 中挂载/卸载 session handler
   ├── setup_logging() 全局日志过滤调整
   ├── 砍掉 brain DEBUG LLM request messages
   └── PIL 降级到 WARNING

4. 跑 bash scripts/check.sh 确保全过
```
