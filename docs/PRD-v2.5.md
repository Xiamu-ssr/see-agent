# see-agent v2.5 PRD — Agent Team 大改造

> 版本：v2.5 | 作者：草莓🍓 + lanxuan | 日期：2026-03-09
> 基于 v2.0 ReAct Agent + v2.5 context compaction 之上的架构升级

---

## 1. 产品定位变化

### v2.0
> 一个能看见屏幕、操作电脑的 Mac AI Agent

### v2.5
> 一个**可扩展的、支持多 Agent 协作**的 Mac AI Agent 平台

核心变化：
- 从"单个 agent 干活"到"一组 agent 协作干活"
- 从"硬编码功能"到"可插拔扩展"
- 从"单一配置"到"分层配置体系"

---

## 2. 背景与调研

### 2.1 市面 Agent Team 方案

| 项目 | 架构 | agent 间通信 | 任务协调 |
|------|------|-------------|---------|
| **CC Agent Teams** | 同终端多实例（tmux） | 直接 message | 共享任务列表 |
| **CrewAI** | 角色制，Python 框架 | 任务输出传递 | 顺序/并行 flow |
| **AutoGen** | 对话式多 agent | 群聊对话 | 无结构化任务 |
| **HiClaw** | 多 Docker 容器（OpenClaw） | Matrix IM | Manager 文件分发 |
| **Agency** | 多 OpenClaw 子进程 | HTTP API + 任务评论 | SQLite 任务表 |
| **OpenClaw RFC** | 共享任务列表 + 邮箱 | 点对点 + 广播 | TaskBoard + 依赖 |

### 2.2 OpenClaw 插件体系参考

OpenClaw v2026.3.7（2026-03-07）发布了 ContextEngine 插件接口，6 种扩展方式：

1. `registerTool()` — 给 agent 加工具（进提示词）
2. `registerChannel()` — 接入消息平台
3. `registerHttpRoute()` — 暴露 HTTP 端点（webhook 回调）
4. `api.on()` — 生命周期钩子
5. `registerContextEngine()` — 替换 context 管理策略（独占 slot）
6. `registerCommand()` — 加斜杠命令 / CLI 子命令

插件通过 npm 安装，用 jiti 运行时加载 TS 代码，`init(api)` 函数接收注册 API。

### 2.3 see-agent 的架构优势

- AgentLoop 是轻量 Python class，**一个进程可跑多个实例**
- 已有 `user_queue` 异步消息注入机制（v2.5 phase 4）
- 已有 context compaction 自动压缩
- 已有 ToolRegistry、Skill loader、MCP 支持

---

## 3. 整体架构

### 3.1 三层设计

```
┌─────────────────────────────────────┐
│  插件层 (Plugin)                     │
│  Memory / ContextEngine / Tool /    │
│  Channel / Hook / Command           │
├─────────────────────────────────────┤
│  协作层 (Team)                       │
│  TeamManager / TeamBus / TaskBoard  │
│  Team Tools / Screen Lock           │
├─────────────────────────────────────┤
│  个体层 (Agent)                      │
│  AgentLoop / Brain / Eye / Hand /   │
│  Session / Overlay                  │
└─────────────────────────────────────┘
```

### 3.2 设计原则

1. **agent 是配置模板**：定义在 `agents/<id>/`，不能独立运行，必须加入 team
2. **agent 与实例 1:1**：一个 agent 定义只能在一个 team 中运行，避免 memory/session 冲突
3. **team 是运行单元**：成员组合、任务列表、通信记录都在 team 目录下
4. **配置分层继承**：全局 → agent 级 → 环境变量
5. **tool 统一管理**：内置 tool、MCP tool、team tool、plugin tool 全部注册到同一个 ToolRegistry，配置化过滤
6. **目录即数据库**：与 OpenClaw 设计哲学保持一致

---

## 4. 工作目录结构

### 4.1 完整目录

```
~/.see-agent/
│
├── config.json                     # 全局配置
│
├── agents/                         # agent 定义（静态配置 + 人格）
│   ├── alice/
│   │   ├── agent.json              # agent 级配置覆盖
│   │   └── SOUL.md                 # 人格文件
│   ├── bob/
│   │   ├── agent.json
│   │   └── SOUL.md
│   └── leader/
│       ├── agent.json
│       └── SOUL.md
│
├── teams/                          # team 运行时（每次任务产生）
│   └── <team_id>/
│       ├── team.json               # team 定义（成员、leader、配置）
│       ├── tasks.json              # 共享任务列表
│       ├── messages.jsonl          # agent 间通信记录（审计用）
│       ├── shared/                 # team 级共享文件（任务产出物）
│       └── agents/                 # 每个 agent 在本 team 的运行时
│           ├── alice/
│           │   ├── workspace/      # 工作目录（私有产出物）
│           │   ├── sessions/       # 会话历史
│           │   ├── memory/         # 记忆（本 team 内）
│           │   └── logs/           # 日志
│           └── bob/
│               └── ...（同结构）
│
├── skills/                         # 全局 skill
└── logs/                           # 全局日志
```

### 4.2 删除的目录

| 删除项 | 原因 |
|--------|------|
| `~/.see-agent/profiles/` | agent 定义天然替代了 profiles |
| `~/.see-agent/memory/` | memory 下沉到 team/agent 级 |
| `~/.see-agent/sessions/` | session 下沉到 team/agent 级 |
| `~/.see-agent/SOUL.md` | 不再有全局 SOUL，每个 agent 有自己的 |

---

## 5. 配置体系

### 5.1 全局配置 `config.json`

```jsonc
{
  // ── LLM 配置 ──
  "llm": {
    "base_url": "https://matrixllm.alipay.com/v1",
    "api_key": "sk-...",
    "model": "claude-opus-4-6"
  },

  // ── 通用配置 ──
  "language": "zh",
  "max_steps": 50,
  "max_images": 4,
  "screenshot_interval_ms": 800,
  "tool_delay_ms": 200,
  "scaling_enabled": true,
  "scaling_match": "pixel_count",
  "show_overlay": true,

  // ── 全局 Skill ──
  "skills_dirs": ["~/.see-agent/skills", "~/.openclaw/skills"],

  // ── 全局 MCP ──
  "mcp_servers": {
    "tavily": {
      "type": "stdio",
      "command": "npx",
      "args": ["tavily-mcp@latest"],
      "env": { "TAVILY_API_KEY": "..." }
    },
    "github": {
      "type": "stdio",
      "command": "...",
      "env": { "GITHUB_TOKEN": "..." }
    }
  },

  // ── Memory 配置（可插拔）──
  "memory": {
    "enabled": true,
    "provider": "file",           // "file" | "mem0" | 第三方
    "file": {},                   // provider 特定配置
    "mem0": { "llm_base_url": "...", ... }
  },

  // ── Context Engine 配置（可插拔）──
  "context_engine": "legacy",     // "legacy" | 第三方

  // ── Context Compaction ──
  "compact": {
    "enabled": true,
    "context_window": 128000,
    "target_ratio": 0.75,
    "keep_recent": 8,
    "summary_model": ""
  }
}
```

### 5.2 Agent 配置 `agents/<id>/agent.json`

```jsonc
{
  "name": "Alice",
  "role": "前端操作员，擅长 UI 交互和表单填写",

  // ── 覆盖全局配置（不写则继承）──
  "llm": { "model": "claude-sonnet-4-5" },
  "max_steps": 30,

  // ── Tool 权限控制 ──
  "tools": {
    // 方式一：白名单
    "allowed": ["screenshot", "click", "type_text", "scroll", "drag", "wait"],
    // 方式二：黑名单（与 allowed 互斥）
    "denied": ["shell", "hotkey"]
  },

  // ── Skill 控制 ──
  "skills": {
    "disabled": ["coding-agent"]     // 禁用特定全局 skill
  },

  // ── MCP 控制 ──
  "mcp_servers": {
    "enabled": ["tavily"],           // 只启用特定 MCP
    // 或 "disabled": ["github"]     // 排除法
  },

  // ── Memory 覆盖 ──
  "memory": {
    "provider": "file"               // agent 级可用不同 provider
  }
}
```

### 5.3 Team 配置 `teams/<team_id>/team.json`

```jsonc
{
  "id": "weekly-report-20260309",
  "name": "周报任务",
  "created_at": "2026-03-09T14:30:00+08:00",
  "status": "running",              // created → running → completed → failed

  // ── 成员 ──
  "members": ["leader", "alice", "bob"],
  "leader": "leader",               // null = 无 leader，用户手动协调

  // ── Team 级配置 ──
  "screen_mode": "serial",          // "serial"（串行锁屏）
  "communication": {
    "mode": "open"                   // "open" | "leader_hub"
  }
}
```

### 5.4 配置继承链

```
config.json（全局默认）
    ↓ 被覆盖
agents/<id>/agent.json（agent 级）
    ↓ 被覆盖
环境变量 SEE_AGENT_*
```

---

## 6. 插件扩展体系

### 6.1 六种扩展方式

| 扩展方式 | 说明 | 进提示词？ | v2.5 实现？ |
|---------|------|-----------|------------|
| **register_tool** | 给 agent 加工具 | ✅ 是 | ✅ 已有 ToolRegistry |
| **register_channel** | 接入消息平台 | ❌ 否 | 🔶 定义接口，不实现 IM |
| **register_http_route** | 暴露 HTTP 端点 | ❌ 否 | ✅ 已有 FastAPI |
| **on (hooks)** | 生命周期钩子 | ❌ 否 | ✅ 4 个核心钩子 |
| **register_context_engine** | 替换 context 管理（独占） | ❌ 否 | ✅ 接口 + Legacy |
| **register_command** | 加 CLI 命令 | ❌ 否 | 🔶 预留 |

### 6.2 插件发现机制

**阶段 1（v2.5）**：内置 `_BUILTIN` dict，配置切换

```python
_BUILTIN_MEMORY = {
    "file": "see_agent.memory.file_backend:FileMemory",
    "mem0": "see_agent.memory.mem0_backend:Mem0Memory",
}
```

**阶段 2（未来）**：Python `entry_points` 标准机制

```python
# 第三方 pip install see-agent-memory-chroma 后自动发现
for ep in entry_points(group="see_agent.memory"):
    providers[ep.name] = ep.load()
```

**阶段 3（未来）**：CLI 安装命令

```bash
see-agent plugin install see-agent-memory-chroma
```

### 6.3 PluginApi 接口

```python
class PluginApi:
    def register_tool(self, tool: BaseTool) -> None
    def register_memory(self, name: str, cls: type[BaseMemory]) -> None
    def register_context_engine(self, name: str, cls: type[BaseContextEngine]) -> None
    def register_channel(self, name: str, channel: BaseChannel) -> None
    def on(self, event: str, handler: Callable) -> None
    @property
    def config(self) -> dict
```

### 6.4 生命周期钩子

v2.5 实现的 4 个核心钩子：

| 事件 | 触发时机 | 用途 |
|------|---------|------|
| `before_task` | 任务开始前 | 注入记忆、加载外部数据 |
| `after_task` | 任务结束后 | 保存记忆、清理资源 |
| `before_compact` | context 压缩前 | 先存重要内容 |
| `after_compact` | context 压缩后 | 更新计数器 |

### 6.5 Memory Provider 接口

```python
class BaseMemory(ABC):
    @abstractmethod
    async def search(self, query: str, agent_id: str, limit: int = 5) -> list[str]: ...
    
    @abstractmethod
    async def add(self, content: str, agent_id: str, metadata: dict | None = None) -> None: ...
    
    @abstractmethod
    async def clear(self, agent_id: str) -> None: ...
```

**内置实现**：

| Provider | 依赖 | 说明 |
|---------|------|------|
| `file` | 零依赖 | JSONL 文件存储，默认 |
| `mem0` | mem0ai | 向量检索，可选安装 |

Mem0 从源码中移除硬编码，改为可选插件（`pip install see-agent[memory]`）。

### 6.6 ContextEngine 接口

```python
class BaseContextEngine(ABC):
    @abstractmethod
    async def assemble(self, ctx: ConversationContext) -> list[dict]: ...
    
    @abstractmethod
    async def compact(self, ctx: ConversationContext) -> None: ...
    
    @abstractmethod
    async def after_turn(self, ctx: ConversationContext, result: dict) -> None: ...
    
    @property
    def owns_compaction(self) -> bool:
        """True 则关闭内置自动压缩"""
        return False
```

**内置实现**：`LegacyContextEngine` — 包装现有的 `_maybe_compact` + `apply_compaction` 逻辑，零行为变化。

---

## 7. Tool 统一管理

### 7.1 Tool 来源

所有 tool 全局注册，不区分来源。team/agent 通过配置选择可用的 tool。

| 来源 | 注册时机 | 示例 |
|------|---------|------|
| 内置 | 启动时自动注册 | screenshot, click, shell, type_text, send_message, claim_task, create_task... |
| MCP | MCP 连接后动态注册 | tavily_search, github_pr_list... |
| Plugin | 插件 init 时注册 | voice_call, ... |

**注意**：`send_message`、`claim_task`、`create_task` 等协作类 tool 也是全局注册的内置 tool。它们依赖 TeamBus/TaskBoard 实例——在非 team 模式下，这些实例不存在，tool 调用会返回错误提示。team/agent 配置中可以通过 `allowed`/`denied` 控制哪些 agent 能用哪些 tool（比如只有 leader 能用 `create_task`）。

### 7.2 统一注册

所有 tool 注册到同一个 `ToolRegistry`，带 `source` 标记：

```python
class ToolRegistry:
    def register(self, tool: BaseTool, source: str = "builtin") -> None:
        self._tools[tool.name] = RegisteredTool(tool=tool, source=source)
    
    def get_filtered(self, agent_config: dict) -> list[BaseTool]:
        """根据 agent 配置过滤可用 tool"""
        allowed = agent_config.get("tools", {}).get("allowed")
        denied = agent_config.get("tools", {}).get("denied", [])
        
        result = []
        for name, rt in self._tools.items():
            if allowed is not None and name not in allowed:
                continue
            if name in denied:
                continue
            result.append(rt.tool)
        return result
```

### 7.3 提示词中不区分来源

发给 LLM 的 `tools[]` 数组中，所有 tool 平等排列，不标注来源。LLM 只需要知道 name + description + parameters。

### 7.4 配置化过滤

team 级和 agent 级都可以控制 tool 开关：

```jsonc
// agents/alice/agent.json — agent 级（前端操作员，禁用 shell）
{
  "tools": {
    "denied": ["shell", "create_task"]
    // alice 不能用 shell，也不能创建任务（只有 leader 能）
  }
}

// agents/leader/agent.json — leader 级
{
  "tools": {
    "denied": ["click", "type_text", "scroll"]
    // leader 不操作屏幕，只做协调
  }
}
```

MCP tool 和 Skill 也同理，全局定义，team/agent 级选择性开关：

```jsonc
{
  "mcp_servers": {
    "enabled": ["tavily"],       // 只启用 tavily
    "disabled": ["github"]       // 或排除法
  },
  "skills": {
    "disabled": ["coding-agent"]
  }
}
```

---

## 8. Agent Team 协作

### 8.1 核心组件

| 组件 | 文件 | 说明 |
|------|------|------|
| **TeamManager** | team/manager.py | 创建 team，启动多个 AgentLoop |
| **TeamBus** | team/bus.py | asyncio.Queue 通信总线 |
| **TaskBoard** | team/board.py | 共享任务列表（JSON 文件） |
| **Team Tools** | hand/tools/team_*.py | agent 间通信和任务管理的 tool |
| **Screen Lock** | asyncio.Lock | 同时只有一个 agent 操作屏幕 |

### 8.2 通信机制

**异步消息队列 + Tool 调用**

agent 间通信通过 `send_message` tool 实现：

```python
# Agent A 调用
send_message(to="bob", content="API 接口字段是什么？")
# → 消息放入 Bob 的 message_queue
# → Tool 立即返回："消息已发送给 bob"
# → A 继续做自己的事（不阻塞）
```

接收方通过 `_drain_queue` 机制读取（复用已有的 `user_queue`）：

```python
# Bob 在下一步 drain 时读到
# 注入为 user message，带前缀：
"[teammate alice]: API 接口字段是什么？"
```

**消息流向**：

```
A 调 send_message tool → TeamBus.send(to, msg)
                           ↓
                       Bob 的 message_queue (asyncio.Queue)
                           ↓
                       Bob 的 _drain_queue（下一步执行前）
                           ↓
                       注入为 user message（role: user, 带 [teammate] 前缀）
                           ↓
                       Bob 的 LLM 看到并处理
                           ↓
                       Bob 调 send_message(to="alice", content="...")
```

**审计**：所有 agent 间消息同步写入 `teams/<team_id>/messages.jsonl`。

### 8.3 共享任务列表

#### 数据结构

```jsonc
// teams/<team_id>/tasks.json
[
  {
    "id": "task_001",
    "title": "从 git log 提取本周提交摘要",
    "description": "执行 git log --since='1 week ago'，总结主要变更",
    "status": "done",
    "assigned_to": "bob",
    "depends_on": [],
    "result": "本周 23 个 commit，主要改了...",
    "created_at": "2026-03-09T14:30:00+08:00",
    "updated_at": "2026-03-09T14:35:00+08:00"
  },
  {
    "id": "task_002",
    "title": "写周报",
    "status": "pending",
    "assigned_to": null,
    "depends_on": ["task_001"],
    "result": null
  }
]
```

#### 状态机

```
pending → claimed → in_progress → done
                                → failed
```

#### Task Tools

所有 task/message tool 是全局注册的内置 tool，通过 agent 配置的 `tools.allowed`/`tools.denied` 控制权限：

| Tool | 建议配置给 | 说明 |
|------|-----------|------|
| `create_task` | Leader | 创建新任务 |
| `assign_task` | Leader | 指派任务给某个 agent |
| `list_tasks` | 所有人 | 查看任务列表和状态 |
| `claim_task` | Worker | 领取未分配的任务 |
| `update_task` | Worker | 更新任务状态 |
| `complete_task` | Worker | 完成任务并提交结果 |

示例配置：
```jsonc
// agents/leader/agent.json — leader 有管理权限
{ "tools": { "denied": ["click", "type_text", "scroll", "drag"] } }

// agents/alice/agent.json — worker 没有管理权限
{ "tools": { "denied": ["shell", "create_task", "assign_task"] } }
```

### 8.4 屏幕锁

v2.5 采用**串行锁**，同一时间只有一个 agent 操作屏幕：

```python
screen_lock = asyncio.Lock()

async def execute_tool_with_lock(self, tool, args):
    if tool.requires_screen:  # screenshot, click, type, scroll, drag
        async with screen_lock:
            return await tool.run(args)
    else:  # shell, send_message, list_tasks 等不需要屏幕
        return await tool.run(args)
```

未来方向：Mac 多桌面（Spaces）隔离，暂不在 v2.5 范围。

### 8.5 提示词变化

team 模式下，system prompt 新增 Team Context 段：

```
## Team Context
- 你的身份：alice（前端操作员）
- Team：weekly-report
- Leader：leader
- 队友：bob（后端工程师）

## 当前任务列表
- [done] task_001: 从 git log 提取摘要（bob）
- [pending] task_002: 写周报（待领取，依赖 task_001）

## 协作规则
- 用 send_message 工具和队友沟通
- 用 claim_task 领取任务，complete_task 完成任务
- 收到队友消息（[teammate xxx]: ...）时优先处理
- 把重要产出物写到 shared/ 目录
```

队友消息注入为 user message（role: user），带前缀：
```
[teammate bob]: API 字段是 name, email, phone
```

---


## 9. 源码结构变化

### 9.1 新增文件

```
see_agent/
├── team/                           # 📌 新增：Agent Team 模块
│   ├── __init__.py
│   ├── bus.py                      # TeamBus（asyncio.Queue 通信总线）
│   ├── board.py                    # TaskBoard（共享任务列表）
│   ├── manager.py                  # TeamManager（创建 team、启动多 AgentLoop）
│   └── config.py                   # team/agent 配置加载 + 继承链
│
├── plugin/                         # 📌 新增：插件系统
│   ├── __init__.py
│   ├── api.py                      # PluginApi 注册接口
│   ├── registry.py                 # 插件注册表（_BUILTIN + 未来 entry_points）
│   └── hooks.py                    # 生命周期事件总线
│
├── agent/
│   ├── context_engine.py           # 📌 新增：BaseContextEngine 接口
│   └── legacy_engine.py            # 📌 新增：LegacyContextEngine
│
├── memory/
│   └── file_backend.py             # 📌 新增：FileMemory（零依赖默认实现）
│
├── hand/tools/
│   ├── send_message.py             # 📌 新增：team tool
│   ├── list_tasks.py               # 📌 新增：team tool
│   ├── claim_task.py               # 📌 新增：team tool
│   ├── complete_task.py            # 📌 新增：team tool
│   ├── update_task.py              # 📌 新增：team tool
│   ├── create_task.py              # 📌 新增：leader tool
│   └── assign_task.py              # 📌 新增：leader tool
│
└── server/routes/
    └── team.py                     # 📌 新增：team API 路由
```

### 9.2 修改文件

| 文件 | 改动内容 |
|------|---------|
| `agent/loop.py` | 加 agent_id 参数、team_context 注入、screen_lock、team message drain |
| `agent/context.py` | 支持 team messages 注入 |
| `brain/prompts.py` | 新增 team_context 参数、Team Context 模板 |
| `hand/tool.py` | ToolRegistry 加 source 标记、get_filtered 方法 |
| `hand/mcp.py` | 支持 enabled/disabled 过滤 |
| `memory/base.py` | 完善 BaseMemory 抽象接口 |
| `skill/loader.py` | 支持 skills.disabled 配置 |
| `session/store.py` | 路径从全局改为 team/agent 下 |
| `config.py` | 删 profiles、加 agent/team 配置加载、新目录结构 |
| `cli/main.py` | 重写：删 profiles/resume、加 agent/team 子命令 |

### 9.3 删除文件

| 文件 | 原因 |
|------|------|
| `memory/mem0_backend.py` | 移出为可选插件（pip install see-agent[memory]） |

### 9.4 新增/修改行数估算

| 类别 | 文件数 | 估计行数 |
|------|--------|---------|
| 新增 | ~16 个 | ~1200 行 |
| 修改 | ~10 个 | ~500 行改动 |
| 删除 | ~1 个 | -109 行 |
| 测试 | ~10 个 | ~800 行 |
| **合计** | | **~2500 行** |

---

## 10. CLI 变化

### 10.1 核心理念变化

**v2.5 不再支持"裸" agent 运行。** 所有操作都在 team 中进行。单 agent 使用就是"一个 agent 的 team"。

### 10.2 新增命令

```bash
# Agent 定义管理
see-agent agent create <id> --role "角色描述"    # 创建 agent 定义
see-agent agent list                             # 列出所有 agent
see-agent agent show <id>                        # 查看 agent 配置
see-agent agent edit <id>                        # 编辑 agent 配置

# Team 管理
see-agent team create --name "任务名" --members alice,bob --leader leader
see-agent team run <team_id> "任务描述"           # 给 team 分配任务并启动
see-agent team chat <team_id>                     # 与 team 交互式对话
see-agent team status <team_id>                   # 查看 team 状态和任务列表
see-agent team list                               # 列出所有 team
see-agent team stop <team_id>                     # 停止 team
see-agent team clean [--before N days]            # 清理旧 team

# 快捷方式（单人 team 的语法糖）
see-agent quick run "打开钉钉发消息"              # 自动用 default agent 创建临时 team 并执行
see-agent quick chat                              # 自动用 default agent 创建临时 team 并交互
```

### 10.3 删除/替代命令

| 旧命令 | 处理 | 原因 |
|--------|------|------|
| `see-agent chat` | 替代为 `see-agent quick chat` | 单 agent 模式不再存在 |
| `see-agent run` | 替代为 `see-agent quick run` | 同上 |
| `see-agent resume` | 删除 | 被 team 机制替代 |
| `see-agent config init` | 删除 | 工作目录自动初始化 |
| `--profile` 参数 | 删除 | 被 agent 定义替代 |

### 10.4 保留命令

```bash
see-agent serve                    # API 服务器（team API + 原有 API）
see-agent config show              # 查看全局配置
see-agent sessions list            # 查看会话（按 team/agent 过滤）
see-agent mcp list/add/remove      # 全局 MCP 管理
see-agent setup install/check      # 依赖检查
```

---

## 11. 实施计划

### Phase 1：插件接口化（基础设施）

**目标**：让核心模块可替换

| 任务 | 说明 |
|------|------|
| Memory 接口化 | BaseMemory + FileMemory（内置默认） |
| Mem0 解耦 | 移为可选依赖 `pip install see-agent[memory]` |
| ContextEngine 接口化 | BaseContextEngine + LegacyContextEngine |
| PluginApi 骨架 | 6 种注册方法 + _BUILTIN 注册表 |
| 生命周期钩子 | before_task / after_task / before_compact / after_compact |
| ToolRegistry 升级 | source 标记 + get_filtered |
| 配置扩展 | memory.provider + context_engine + tools.allowed/denied |

### Phase 2：工作目录重构

**目标**：从单 agent 升级到多 agent + team 目录结构

| 任务 | 说明 |
|------|------|
| 新目录结构 | agents/ + teams/ |
| 配置继承链 | config.json → agent.json → 环境变量 |
| agent 定义 | agent.json + SOUL.md |
| team 定义 | team.json + tasks.json |
| Session 迁移 | 路径从全局改为 team/agent 下 |
| Memory 迁移 | 路径从全局改为 team/agent 下 |
| Skill 开关 | skills.disabled 配置 |
| MCP 开关 | mcp_servers.enabled/disabled 配置 |
| 删除 profiles | 删除代码和目录 |

### Phase 3：Agent Team 核心

**目标**：同进程多 AgentLoop 协作

| 任务 | 说明 |
|------|------|
| AgentLoop 改造 | 加 agent_id、team_context 参数 |
| TeamBus | asyncio.Queue 通信 + messages.jsonl 落盘 |
| TaskBoard | JSON 文件持久化 + 状态机 |
| Team Tools（Worker） | send_message、list_tasks、claim_task、complete_task、update_task（全局注册，agent 配置控制权限） |
| Team Tools（Leader） | create_task、assign_task（全局注册，agent 配置控制权限） |
| TeamManager | 创建 team、启动多 AgentLoop、屏幕锁 |
| 提示词 | Team Context 段 + 协作规则 |
| CLI | agent create/list + team create/run/status/list/stop |

### Phase 4：测试与文档

现有 243 个测试需要大量改动，以下是测试策略：

| 类别 | 说明 |
|------|------|
| **需要重写的测试** | session/store 相关（路径从全局改为 team/agent 下）、config 加载（删 profiles、加 agent/team 继承链）、CLI 相关（命令重构） |
| **需要修改的测试** | AgentLoop 测试（加 agent_id 参数）、memory 测试（接口化后 mock 方式变化）、tool 测试（ToolRegistry.get_filtered） |
| **可保留的测试** | brain/openai_client、eye/scaling、overlay、环境检测等未改动模块 |
| **新增测试** | |
| - team/bus.py | TeamBus 通信：发送、接收、广播、队列排序 |
| - team/board.py | TaskBoard：创建、领取、完成、依赖检查、状态机 |
| - team/manager.py | TeamManager：多 AgentLoop 启动、屏幕锁、配置继承 |
| - team/config.py | 配置继承链：全局 → agent → 环境变量 |
| - plugin/registry.py | 插件注册表：内置发现、工厂函数 |
| - plugin/hooks.py | 生命周期钩子：触发顺序、错误隔离 |
| - memory/file_backend.py | FileMemory：search、add、clear、持久化 |
| - agent/context_engine.py | ContextEngine 接口：Legacy 包装正确性 |
| - hand/tool.py | ToolRegistry.get_filtered：allowed/denied 过滤 |
| - team tools | send_message、claim_task 等 tool 的单元测试 |
| - CLI | agent/team 命令的集成测试 |

**测试脚本**：现有的 `Makefile` 中的 `make test`（pytest）保持不变，可能需要新增：

```makefile
test:           ## 全量测试
	$(VENV)/bin/pytest tests/ -v

test-team:      ## 仅 team 相关测试
	$(VENV)/bin/pytest tests/test_team*.py tests/test_bus.py tests/test_board.py -v

test-plugin:    ## 仅 plugin 相关测试
	$(VENV)/bin/pytest tests/test_plugin*.py tests/test_memory_file.py -v
```

| 任务 | 说明 |
|------|------|
| 重写受影响测试 | 预估 ~50 个测试需要修改 |
| 新增测试 | 预估 ~80 个新测试 |
| 更新 PRD | 覆盖 v2.0 PRD |
| 更新 README | 安装、配置、使用说明 |
| 更新 CLAUDE.md | CC 的工作指引（新目录结构、新命令） |

---

## 12. 不在 v2.5 范围内

| 特性 | 原因 |
|------|------|
| IM Channel（Telegram/钉钉） | 接口预留，未来插件实现 |
| entry_points 第三方插件发现 | 先用 _BUILTIN |
| Web UI | 太重 |
| 跨进程/跨机器 team | 先同进程 |
| L3 组织级编排 | 更远的方向 |
| Mac 多桌面隔离 | API 受限，暂用串行锁 |
| see-agent mcp add/list/remove | 已有但可能需要调整 |

---

## 13. 设计约束

1. **team 是唯一运行单元**：不再支持"裸" agent 运行。单 agent 使用通过 `see-agent quick` 快捷方式（自动创建单人 team）
2. **agent 定义与实例 1:1**：一个 agent 定义只能在一个 team 中运行
3. **目录即数据库**：所有状态用 JSON/JSONL 文件持久化，与 OpenClaw 哲学一致
4. **tool 全局注册、配置过滤**：内置 tool、MCP tool、plugin tool 统一注册，team/agent 通过 allowed/denied 控制
5. **Python 3.11+**：保持最低版本要求不变
6. **零必须外部依赖**：FileMemory + LegacyContextEngine 不依赖任何第三方库
7. **配置驱动**：所有行为差异通过配置文件控制，为未来前端界面留好数据基础
