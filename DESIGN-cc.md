# AGENTS.md — see-agent 工作目录与配置规范

> 本文档是 see-agent 的权威设计文档。定义工作目录结构和所有配置文件的全量字段。
> 代码修改必须与本文档保持一致。

---

## 一、完整目录结构

根目录：`~/.see-agent/`（可通过 `SEE_AGENT_HOME` 环境变量覆盖）

```
~/.see-agent/
├── config.json                          # 全局配置文件（最低优先级）
├── dev.see-agent.server.plist           # launchd 服务定义文件
│
├── agents/                              # 所有 Agent 的数据根目录
│   └── {agent_id}/                      # 单个 Agent 的全部数据
│       ├── agent.json                   # Agent 身份 + 配置覆盖
│       ├── SOUL.md                      # 人格/系统提示词
│       ├── AGENTS.md                    # 操作指南（注入 system prompt）
│       ├── memory/                      # 记忆目录
│       │   ├── MEMORY.md               # 长期记忆（精华摘要）
│       │   └── YYYY-MM-DD.md           # 日记（按天记录）
│       └── session/                    # 会话目录
│               ├── meta.json            # 会话元数据
│               ├── messages.jsonl       # 原始对话记录（完整不删）
│               ├── summary.jsonl       # 会话摘要记录（压缩后）
│               ├── session.log          # 会话级日志
│               ├── system_prompt_log.md # system prompt 变更审计
│               └── screenshots/         # 截图目录
│                   └── step_NNN.webp    # 每步截图
│
├── teams/                               # 所有 Team 的数据根目录
│   └── {team_id}/                       # 单个 Team
│       ├── team.json                    # Team 配置
│       └── messages.jsonl               # Team 消息流水
│
├── skills/                              # Skill 目录（内置 + ClawHub）
│   └── {skill_name}/
│       └── SKILL.md
│
├── logs/                                # 全局日志目录
│   └── YYYY-MM-DD.log                  # 按天滚动日志
│
└── run/                                 # 运行时临时目录（可重建）
    └── agents/
        └── {agent_id}/
            ├── config.json              # 合并后的运行时配置快照
            ├── inbox.jsonl              # 消息收件箱（过渡方案）
            └── agent.sock               # UDS socket
```

> **设计原则**：一个 Agent 的所有持久数据（身份、配置、记忆、会话）都在 `agents/{id}/` 下。没有 `workspace/` 子目录层级。

---

## 二、agents/ — Agent 数据目录

每个 Agent 独占 `agents/{agent_id}/` 目录，包含这个 Agent 的一切。

### 2.1 agent.json — Agent 配置

Agent 的身份信息和配置覆盖。前端对 Agent 的所有设置修改（tools 开关、skills 开关、MCP 开关、参数调整）都写回此文件。

```jsonc
{
    // ── 身份信息（必填）──
    "id": "xxxx4",                       // Agent ID，与目录名一致
    "name": "A",                         // 显示名
    "role": "general assistant",         // 角色描述

    // ── 全局配置覆盖（可选）──
    // 字段级 deep merge 到 config.json 之上
    // 可覆盖 llm、max_steps、memory、compact 等任意全局字段
    "config_overrides": {
        "llm": { "model": "claude-opus-4-6" },
        "max_steps": 70,
        "compact": { "enabled": true }
    },

    // ── 工具开关（可选）──
    // 前端 tools tab 切换开关时必须修改此字段
    "tools_config": {
        "disabled": ["shell", "drag"]    // 禁用的工具名列表
    },

    // ── Skill 开关（可选）──
    "skills_config": {
        "disabled": ["clawhub"]          // 禁用的 skill 名列表
    },

    // ── MCP 配置（可选）──
    "mcp_config": {
        "servers": {                     // Agent 独有的 MCP server
            "my-mcp": {
                "type": "stdio",
                "command": "node",
                "args": ["server.js"]
            }
        },
        "disabled": ["tavily"]           // 禁用的全局 MCP server
    },

    // ── 沙箱配置（可选）──
    "sandbox": {
        "profile": "default",
        "extra_read": ["/Users/x/Docs"],
        "extra_write": ["/tmp/output"]
    },

    // ── 自定义 SOUL 路径（可选）──
    "soul_path": null                    // null = 默认读 agents/{id}/SOUL.md
}
```

**全量字段表**：

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | string | ✅ | — | Agent ID，与目录名一致 |
| `name` | string | ✅ | — | 显示名 |
| `role` | string | ❌ | `"general assistant"` | 角色描述 |
| `config_overrides` | object | ❌ | `{}` | 覆盖 config.json 任意字段（deep merge） |
| `tools_config.disabled` | string[] | ❌ | `[]` | 禁用的工具名列表 |
| `skills_config.disabled` | string[] | ❌ | `[]` | 禁用的 skill 名列表 |
| `mcp_config.servers` | object | ❌ | `{}` | Agent 独有 MCP server |
| `mcp_config.disabled` | string[] | ❌ | `[]` | 禁用的全局 MCP server 名 |
| `sandbox.profile` | string | ❌ | `"default"` | 沙箱 profile |
| `sandbox.extra_read` | string[] | ❌ | `[]` | 额外读权限路径 |
| `sandbox.extra_write` | string[] | ❌ | `[]` | 额外写权限路径 |
| `soul_path` | string\|null | ❌ | `null` | 自定义 SOUL.md 路径 |

### 2.2 Prompt 注入文件 — SOUL.md / AGENTS.md / 其他 md

这些 md 文件直接放在 `agents/{id}/` 下（**不是** `agents/{id}/workspace/` 下）。

`build_system_prompt()` 按以下顺序读取并注入到 system prompt：

| 顺序 | 文件 | 用途 | 必须 |
|------|------|------|------|
| 1 | `IDENTITY.md` | 身份描述（名字、性格、emoji） | 可选 |
| 2 | `AGENTS.md` | 操作指南（工具使用规则、消息处理、团队协作） | 推荐 |
| 3 | `SOUL.md` | 人格/性格/提示词核心 | 推荐 |
| 4 | `TOOLS.md` | 工具使用备忘（环境特定的笔记） | 可选 |
| 5 | `USER.md` | 用户信息（了解用户的笔记） | 可选 |
| 6 | `memory/MEMORY.md` | 长期记忆（自动注入到 prompt） | 自动生成 |

截断限制：每个文件最大 20,000 字符，总计最大 100,000 字符。

创建 Agent 时，从 `see_agent/templates/` 复制模板文件到 `agents/{id}/` 下。

### 2.3 memory/ — 记忆目录

位置：`agents/{agent_id}/memory/`

```
memory/
├── MEMORY.md            # 长期记忆：Agent 自主维护的精华摘要
├── 2026-03-10.md        # 日记：当天发生的事
├── 2026-03-11.md
└── ...
```

**文件规则**：
- 只允许两种文件名：`MEMORY.md` 和 `YYYY-MM-DD.md`
- 其他文件名会被 `write_memory` tool 拒绝

**MEMORY.md — 长期记忆**：
- Agent 跨 session 的精华知识库
- 自动注入到 system prompt（见 2.2 的第 6 项）
- Agent 通过 `write_memory` tool 主动写入
- 内容应该是精炼的、有价值的——不是流水账

**YYYY-MM-DD.md — 日记**：
- 按天记录的原始笔记
- 不注入 system prompt（太长了）
- Agent 通过 `memory_search` tool 搜索历史日记
- 写入时机：
  1. Agent 主动调用 `write_memory` tool（自觉）
  2. context compact 触发时，自动将 summary 写入当天日记

**搜索机制**：
- BM25 关键词搜索（`MarkdownMemoryBackend`）
- 对 `memory/` 下所有 `*.md` 文件的段落建索引
- 中文用字符 bigram 分词，英文用空格分词

### 2.4 sessions/ — 会话目录

位置：`agents/{agent_id}/sessions/`

每次 `AgentLoop.run()` 创建一个新 session，目录名格式：`YYYYMMDD_HHMMSS_{6位hex}`

```
sessions/
└── 20260311_182530_a1b2c3/
    ├── meta.json                # 会话元数据
    ├── messages.jsonl           # 原始对话记录（核心）
    ├── session.log              # 会话级 DEBUG 日志
    ├── system_prompt_log.md     # system prompt 变更审计
    └── screenshots/
        ├── step_000.webp        # 初始截图
        ├── step_001.webp        # 第 1 步截图
        └── ...
```

#### meta.json — 会话元数据

```jsonc
{
    "id": "20260311_182530_a1b2c3",
    "task": "打开浏览器搜索天气",       // 用户任务描述
    "status": "running",               // running | completed | failed
    "created_at": "2026-03-11T18:25:30+00:00",
    "updated_at": "2026-03-11T18:30:15+00:00",
    "total_steps": 12,
    "elapsed_seconds": 285.3,
    "summary": "已完成搜索，结果显示...", // 完成/失败时的摘要
    "config_snapshot": {                // 创建时的配置快照
        "model": "claude-opus-4-6",
        "max_steps": 70,
        "scaling_enabled": true
    }
}
```

#### messages.jsonl — 原始对话记录

每行一个 JSON 对象。这是完整的、不可变的对话流水账。compact 不会删除旧行，只追加 `type=compact` 标记。

每行公共字段：
- `msg_id`：自增 ID（从 1 开始）
- `ts`：ISO-8601 时间戳
- `type`：消息类型

**消息类型清单**：

| type | 含义 | 关键字段 |
|------|------|----------|
| `system` | system prompt | `text` |
| `user_task` | 用户任务（含截图引用） | `text`, `screenshot`（文件名）, `detail` |
| `assistant` | LLM 回复 | `content`, `tool_calls[{id, name, args}]` |
| `tool_result` | 工具执行结果 | `tool_call_id`, `result` |
| `screenshot` | 独立截图消息 | `screenshot`（文件名）, `detail` |
| `user_reply` | 用户中途回复（call_user 后） | `text` |
| `system_hint` | 系统提示（无进展警告等） | `text` |
| `compact` | 压缩标记 | `summary`, `first_kept_msg_id` |

#### Context Compact 机制

**触发条件**：`compact.enabled=true` 且估算 token > `context_window × target_ratio`

**执行流程**：
1. 取 messages[1:-keep_recent]（跳过 system，保留最近 N 条）
2. 调 `brain.summarize()` 生成摘要
3. 替换上下文中的旧消息为摘要
4. 向 messages.jsonl 追加 `type=compact` 记录（含 summary + first_kept_msg_id）
5. 同时触发一次日记重写 → 写入 `memory/YYYY-MM-DD.md`

**compact 后，下一次推理携带**：
- system prompt（完整）
- compact summary（注入为 `[Conversation Summary]`）
- 最近 `keep_recent` 条原始消息

**恢复已有 session 时**：
- 读 messages.jsonl，找最后一条 `type=compact`
- 跳过 `msg_id < first_kept_msg_id` 的旧消息
- 用 summary + 保留消息重建上下文

---

## 三、teams/ — Team 数据目录

Team 是轻量级的「房间」——成员列表 + leader + 通信记录。Agent 数据不在 team 目录下。

```
teams/{team_id}/
├── team.json            # Team 配置
└── messages.jsonl       # Team 消息流水
```

### 3.1 team.json — Team 配置

```jsonc
{
    "id": "04582c0a",                    // Team ID（8 位 hex）
    "name": "测试部",
    "members": ["xxxx4", "alice"],       // 成员 Agent ID 列表
    "leader": "xxxx4",                   // Leader Agent ID（可选）
    "screen_mode": "serial",            // 屏幕共享模式：serial | parallel
    "status": "created",                // 状态：created | running | stopped
    "created_at": "2026-03-11T10:20:36+00:00"
}
```

**全量字段表**：

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | string | ✅ | — | Team ID（8 位 hex，自动生成） |
| `name` | string | ✅ | — | 显示名 |
| `members` | string[] | ✅ | `[]` | 成员 Agent ID 列表 |
| `leader` | string\|null | ❌ | `null` | Leader Agent ID |
| `screen_mode` | string | ❌ | `"serial"` | `serial`=轮流用屏幕，`parallel`=各自截图 |
| `status` | string | ❌ | `"created"` | 状态 |
| `created_at` | string | ❌ | — | ISO-8601 创建时间 |

### 3.2 messages.jsonl — Team 消息流水

记录 Team 内所有跨 Agent 通信和用户消息。每行一个 JSON：

```jsonc
{
    "sender": "owner",       // 发送者：owner（用户）| agent_id
    "recipient": "xxxx4",    // 接收者：agent_id
    "content": "你好",
    "ts": "2026-03-11T10:28:24+00:00"
}
```

---

## 四、skills/ — Skill 目录

位置：`~/.see-agent/skills/`

```
skills/
└── {skill_name}/
    └── SKILL.md             # Skill 定义文件
```

- 内置 skill 在 `see_agent/builtin_skills/` 下，首次启动自动复制到 `~/.see-agent/skills/`（不覆盖已有）
- ClawHub 安装的 skill 也放在这里
- skill 搜索路径由 `config.json` 的 `skills_dirs` 配置
- Agent 可通过 `agent.json` 的 `skills_config.disabled` 禁用特定 skill

---

## 五、logs/ — 全局日志目录

位置：`~/.see-agent/logs/`

```
logs/
├── 2026-03-10.log
├── 2026-03-11.log
└── ...
```

- 按天命名：`YYYY-MM-DD.log`
- 使用 `RotatingFileHandler`：每个文件最大 10MB，保留 5 个备份
- 全局日志记录所有 Agent 共享的运行信息
- 详细 DEBUG 日志在 session 级别：`agents/{id}/sessions/{sid}/session.log`

---

## 六、run/ — 运行时临时目录

位置：`~/.see-agent/run/`

**纯运行时数据**，进程重启后可以全部重建。存在意义是隔离持久数据和临时数据，崩溃时可安全 `rm -rf run/`。

```
run/
└── agents/
    └── {agent_id}/
        ├── config.json      # Supervisor 写给 Worker 的合并后配置
        ├── inbox.jsonl      # 消息收件箱（过渡方案）
        └── agent.sock       # UDS socket
```

### 6.1 run/agents/{id}/config.json — 运行时配置快照

Supervisor 启动 Worker 进程前，将全局配置 + agent 覆盖合并后写入此文件。除了正常配置字段外，还包含 `_` 前缀的内部字段：

| 内部字段 | 含义 |
|----------|------|
| `_agent_id` | Agent ID |
| `_session_root` | Session 存储路径 → `agents/{id}/sessions` |
| `_memory_dir` | 记忆目录路径 → `agents/{id}/memory` |
| `_leader_id` | Team leader ID |
| `_denied_tools` | 被禁用的工具列表（来自 tools_config.disabled） |
| `_team_context` | Team 上下文字符串 |
| `_owner_display` | 拥有者显示名 |
| `_result_path` | Worker 完成后写结果的路径 |
| `_screen_access` | 是否有屏幕访问权限 |

### 6.2 inbox.jsonl — 消息收件箱

过渡方案。`Supervisor.send_to()` 将消息写入此文件，Worker 进程读取。等 UDS push 机制完善后将废弃。

---

## 七、全局配置 config.json

位置：`~/.see-agent/config.json`

最低优先级配置。所有字段可被 `agent.json.config_overrides` 覆盖。

```jsonc
{
    // ═══ LLM ═══
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o"
    },

    // ═══ 基本参数 ═══
    "language": "zh",                // zh | en
    "max_steps": 50,                 // 单任务最大步数（1 step = 1 次 LLM 推理，可含多个 tool call）
    "max_images": 5,                 // 上下文滑动窗口保留截图数
    "screenshot_interval_ms": 800,   // 截图采集间隔（ms）
    "tool_delay_ms": 200,            // 同一 step 内连续 tool call 之间的延迟（ms），给 GUI 留反应时间

    // ═══ 缩放 ═══
    "scaling_enabled": true,
    "scaling_match": "aspect_ratio", // aspect_ratio | pixel_count | exact

    // ═══ 上下文引擎 ═══
    "context_engine": "default",

    // ═══ Skill 搜索路径 ═══
    "skills_dirs": ["~/.see-agent/skills"],

    // ═══ 记忆 ═══
    "memory": {
        "enabled": true,
        "search": { "mode": "bm25" }
    },

    // ═══ Context Compact ═══
    "compact": {
        "enabled": true,             // 默认开启
        "context_window": 200000,    // 模型上下文窗口（tokens）
        "target_ratio": 0.75,        // tokens > window × ratio 时触发压缩
        "keep_recent": 8,            // 压缩后保留最近 N 条消息
        "summary_model": ""          // 摘要模型，空 = 使用主模型
    },

    // ═══ 环境变量注入 ═══
    "env": {},

    // ═══ MCP Server ═══
    "mcp_servers": {},

    // ═══ Overlay ═══
    "show_overlay": true
}
```

**全量字段表**：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `llm.base_url` | string | `https://api.openai.com/v1` | LLM API 地址 |
| `llm.api_key` | string | `""` | API Key |
| `llm.model` | string | `gpt-4o` | 模型名 |
| `language` | string | `zh` | 语言 |
| `max_steps` | int | `50` | 最大步数（1 step = 1 次推理） |
| `max_images` | int | `5` | 上下文保留截图数 |
| `screenshot_interval_ms` | int | `800` | 截图间隔 |
| `tool_delay_ms` | int | `200` | tool call 间延迟 |
| `scaling_enabled` | bool | `true` | 缩放开关 |
| `scaling_match` | string | `aspect_ratio` | 缩放策略 |
| `context_engine` | string | `default` | 上下文引擎 |
| `skills_dirs` | string[] | `["~/.see-agent/skills"]` | Skill 路径 |
| `memory.enabled` | bool | `true` | 记忆开关 |
| `memory.search.mode` | string | `bm25` | 搜索模式 |
| `compact.enabled` | bool | `true` | 压缩开关 |
| `compact.context_window` | int | `200000` | 上下文窗口 |
| `compact.target_ratio` | float | `0.75` | 压缩阈值 |
| `compact.keep_recent` | int | `8` | 保留条数 |
| `compact.summary_model` | string | `""` | 摘要模型 |
| `env` | object | `{}` | 环境变量 |
| `mcp_servers` | object | `{}` | MCP 配置 |
| `show_overlay` | bool | `true` | Overlay 开关 |

**环境变量覆盖（最高优先级）**：

| 环境变量 | 覆盖字段 |
|----------|---------|
| `SEE_AGENT_HOME` | 工作目录根路径 |
| `SEE_AGENT_BASE_URL` | `llm.base_url` |
| `SEE_AGENT_API_KEY` | `llm.api_key` |
| `SEE_AGENT_MODEL` | `llm.model` |

---

## 八、配置优先级

从低到高：

```
DEFAULT_CONFIG（代码硬编码）
    ↓ 被覆盖
config.json（用户全局配置）
    ↓ 被覆盖
team.json.config_overrides（Team 级，预留未实现）
    ↓ 被覆盖
agent.json.config_overrides（Agent 级）
    ↓ 被覆盖
环境变量（SEE_AGENT_BASE_URL / SEE_AGENT_API_KEY / SEE_AGENT_MODEL）
```

合并规则：字段级 deep merge。嵌套 dict 递归合并，非 dict 值直接覆盖。

---

## 九、Plist 服务

| 项 | 值 |
|----|-----|
| 位置 | `~/.see-agent/dev.see-agent.server.plist` |
| Label | `dev.see-agent.server` |
| 默认端口 | 28789 |
| 自动重启 | 异常退出时重启 |
| 启动 | `see-agent start`（写 plist + `launchctl bootstrap`） |
| 停止 | `see-agent stop`（`launchctl bootout` + 删 plist + 清 run/） |

---

## 十、前端 API 同步约定

前端所有开关操作必须通过 `PUT /api/agents/{agent_id}` 写回 `agent.json`：

| 前端 Tab | 修改的 agent.json 字段 |
|----------|----------------------|
| Tools | `tools_config.disabled` |
| Skills | `skills_config.disabled` |
| MCP | `mcp_config.disabled` / `mcp_config.servers` |
| Config | `config_overrides` |
