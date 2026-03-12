# DESIGN-openclaw.md — see-agent 工作目录与配置规范

> 本文档是 see-agent 的权威设计文档。
> 定义工作目录结构、所有配置文件的全量字段、消息流和记忆机制。
> 代码修改必须与本文档保持一致。

---

## 一、完整目录结构

根目录：`~/.see-agent/`（可通过 `SEE_AGENT_HOME` 环境变量覆盖）

```
~/.see-agent/
├── config.json                          # 全局配置（最低优先级）
├── dev.see-agent.server.plist           # launchd 服务定义
│
├── agents/                              # Agent 数据根目录
│   └── {agent_id}/
│       ├── agent.json                   # Agent 配置（覆盖全局配置）
│       ├── SOUL.md                      # 人格提示词
│       ├── AGENTS.md                    # 操作指南
│       ├── IDENTITY.md                  # 身份（名字/emoji/头像，Agent 可自改）
│       ├── inbox.jsonl                  # 收件箱（系统写入，系统消费）
│       ├── inbox_cursor.json            # 已读游标
│       ├── memory/                      # 记忆目录
│       │   ├── MEMORY.md               # 长期记忆
│       │   └── YYYY-MM-DD.md           # 日记
│       └── session/                     # 单会话（一个 Agent 一个会话）
│           ├── meta.json                # 会话元数据
│           ├── messages.jsonl           # 对话历史（含 compact summary）
│           ├── session.log              # 会话级日志
│           ├── system_prompt_log.md     # system prompt 审计
│           └── screenshots/
│               └── step_NNN.webp
│
├── teams/
│   └── {team_id}/
│       ├── team.json                    # Team 配置
│       ├── messages.jsonl               # Team 通信流水（全员视角）
│       ├── tasklist.json                # Team 任务列表（Agent 自主查看/领取）
│       └── shared/                      # Team 共享目录
│
├── skills/
│   └── {skill_name}/
│       └── SKILL.md
│
└── logs/
    └── YYYY-MM-DD.log
```

> sock 文件放 `/tmp/see-agent-{agent_id}.sock`，不在工作目录内。

---

## 二、agents/ — Agent 数据目录

### 2.1 agent.json

Agent 身份 + 配置覆盖。与 config.json 同构，直接 deep merge 覆盖。

```jsonc
{
    // ── 身份（agent.json 独有，config.json 中没有）──
    "id": "xxxx4",
    "name": "A",

    // ── 以下字段与 config.json 同构，写了就覆盖 ──
    "llm": {
        "model": "claude-opus-4-6"       // 只写差异字段
    },
    "agent": {
        "max_steps": 70
    },
    "tools": {
        "disabled": ["shell"]
    },
    "skills": {
        "disabled": ["clawhub"]
    },
    "mcp": {
        "servers": {                     // Agent 独有的 MCP server
            "my-mcp": { "type": "stdio", "command": "node", "args": ["server.js"] }
        },
        "disabled": ["tavily"]           // 禁用全局 MCP server
    },
    "sandbox": {
        "profile": "default",
        "extra_read": ["/Users/x/Docs"],
        "extra_write": ["/tmp/output"]
    }
}
```

**agent.json 独有字段**（config.json 中没有）：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | string | ✅ | Agent ID，与目录名一致 |
| `name` | string | ✅ | 显示名（Agent 可通过修改 IDENTITY.md 自改） |

其余字段与 config.json 同构，写了就覆盖，没写就继承全局值。

### 2.2 IDENTITY.md / SOUL.md / AGENTS.md — Prompt 注入文件

直接放在 `agents/{id}/` 下。`build_system_prompt()` 按以下顺序注入：

| 顺序 | 文件 | 用途 | Agent 可自改 |
|------|------|------|-------------|
| 1 | `IDENTITY.md` | 身份：名字、emoji、头像 | ✅ |
| 2 | `AGENTS.md` | 操作指南：工具规则、消息处理、团队协作 | ❌ 系统管理 |
| 3 | `SOUL.md` | 人格/性格/核心提示词 | ✅ |
| 4 | `memory/MEMORY.md` | 长期记忆 | ✅ Agent 通过 write_memory 写 |

创建 Agent 时从 `see_agent/templates/` 复制模板。

截断限制：每文件 20,000 字符，总计 100,000 字符。

### 2.3 inbox.jsonl + inbox_cursor.json — 消息收件箱

**设计原则**：消息是"推"的——系统写入、系统消费，Agent 无感知。

#### 写入（谁写）

所有发给该 Agent 的消息，由 Server/MessageRouter 写入 `agents/{id}/inbox.jsonl`：

```jsonc
{"msg_id": 1, "source": "user",     "sender": "user",  "content": "你好",         "priority": "normal", "ts": "..."}
{"msg_id": 2, "source": "leader",   "sender": "alice",  "content": "去测试登录",    "priority": "steer",  "ts": "..."}
{"msg_id": 3, "source": "teammate", "sender": "bob",    "content": "我做完了",      "priority": "normal", "ts": "..."}
{"msg_id": 4, "source": "system",   "sender": "system", "content": "Task assigned", "priority": "normal", "ts": "..."}
```

| 字段 | 说明 |
|------|------|
| `msg_id` | 自增 ID |
| `source` | 来源分类：user / leader / teammate / system |
| `sender` | 发送者标识 |
| `content` | 消息内容 |
| `priority` | `normal`（collect）或 `steer`（立即注入） |
| `ts` | ISO-8601 时间戳 |

#### 消费（系统代码控制，Agent 无感知）

Agent loop 在固定位置自动 drain inbox：

- **normal 消息**：当前 LLM 调用 + 所有 tool 执行完毕后，下一轮 loop 开始前，系统批量取出所有 normal 消息注入上下文
- **steer 消息**：当前这一步 tool 执行完毕后，下一次调 LLM 之前，系统立即注入

消费后更新 `inbox_cursor.json`：

```jsonc
{"last_read_id": 4}
```

#### 进程恢复

进程挂掉重启 → 读 `inbox_cursor.json` 获取游标 → 从 `inbox.jsonl` 中 `msg_id > last_read_id` 继续消费 → 不丢消息。

### 2.4 memory/ — 记忆目录

```
memory/
├── MEMORY.md            # 长期记忆（精华，注入 system prompt）
├── 2026-03-10.md        # 日记
└── 2026-03-11.md
```

文件名只允许 `MEMORY.md` 和 `YYYY-MM-DD.md`。

**记忆全靠 Agent 自觉**，系统只提供工具：

| Tool | 用途 |
|------|------|
| `memory_search` | BM25 搜索 memory/ 下所有 md 的段落 |
| `write_memory` | 向指定文件追加内容 |

- `MEMORY.md` 注入 system prompt（见 2.2），日记不注入（太长）
- AGENTS.md 中引导 Agent：开始任务先 search，完成重要任务后 write
- 搜索：BM25，中文 bigram 分词，英文空格分词

### 2.5 session/ — 单会话

一个 Agent 永远只有一个会话，不再有多 session 目录。

```
session/
├── meta.json
├── messages.jsonl           # 对话历史（含 compact summary）
├── session.log
├── system_prompt_log.md
└── screenshots/
    └── step_NNN.webp
```

#### meta.json

```jsonc
{
    "status": "running",                 // running | idle | failed
    "created_at": "2026-03-11T18:25:30+00:00",
    "updated_at": "2026-03-11T18:30:15+00:00",
    "total_steps": 12,
    "elapsed_seconds": 285.3,
    "current_task": "打开浏览器搜索天气",
    "config_snapshot": {
        "model": "claude-opus-4-6",
        "max_steps": 70
    }
}
```

#### messages.jsonl

每行一个 JSON，公共字段：`msg_id`（自增）、`ts`、`type`。

完整对话流水账，compact 不删旧行，只追加 `type=compact` 标记。

| type | 含义 | 关键字段 |
|------|------|----------|
| `system` | system prompt | `text` |
| `user_task` | 用户任务（含截图） | `text`, `screenshot`, `detail` |
| `assistant` | LLM 回复 | `content`, `tool_calls[{id, name, args}]` |
| `tool_result` | 工具结果 | `tool_call_id`, `result` |
| `screenshot` | 独立截图 | `screenshot`, `detail` |
| `user_reply` | 用户中途回复 | `text` |
| `system_hint` | 系统提示 | `text` |
| `compact` | 压缩标记 | `summary`, `first_kept_msg_id` |
| `inbox` | 来自收件箱的消息（系统注入） | `source`, `sender`, `content` |

#### Context Compact

**触发前**：系统注入静默提醒：
> [系统提示] 上下文即将达到窗口上限，请立即用 write_memory 保存重要信息，下一轮将执行上下文压缩。

**触发条件**：估算 token > `compact.context_window × compact.target_ratio`

**流程**：
1. 注入静默提醒，等 Agent 回复一轮（给它存记忆的机会）
2. 取 messages[1:-keep_recent]，调 `brain.summarize()` 生成摘要
3. 替换旧消息为摘要
4. 追加 `type=compact` 到 messages.jsonl

**compact 后推理携带**：system prompt + compact summary（`[Conversation Summary]`）+ 最近 `keep_recent` 条消息

**恢复**：读 messages.jsonl 最后一条 `type=compact` → 跳过 `msg_id < first_kept_msg_id` → 重建上下文

---

## 三、teams/ — Team 数据目录

Team 是轻量「房间」——成员列表 + 通信 + 任务板。Agent 数据不在 team 目录下。

```
teams/{team_id}/
├── team.json
├── messages.jsonl           # 全员通信流水
├── tasklist.json            # 任务列表（Agent 通过 tool 自主操作）
└── shared/                  # 共享目录
```

### 3.1 team.json

```jsonc
{
    "id": "04582c0a",
    "name": "测试部",
    "members": [
        { "id": "xxxx4", "role": "项目经理" },
        { "id": "alice", "role": "测试工程师" }
    ],
    "screen_mode": "serial",
    "status": "created",
    "created_at": "2026-03-11T10:20:36+00:00"
}
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | string | ✅ | — | Team ID（8 位 hex） |
| `name` | string | ✅ | — | 显示名 |
| `members` | array | ✅ | `[]` | 成员列表，每项 `{id, role}` |
| `members[].id` | string | ✅ | — | Agent ID |
| `members[].role` | string | ❌ | `""` | 在 team 中的角色 |
| `screen_mode` | string | ❌ | `"serial"` | serial / parallel |
| `status` | string | ❌ | `"created"` | created / running / stopped |
| `created_at` | string | ❌ | — | ISO-8601 |

> 注意：leader 不再是单独字段，通过 `members[].role` 体现（role 含 "leader" 或类似描述即可）。

### 3.2 messages.jsonl — 全员通信流水

```jsonc
{"sender": "owner", "recipient": "xxxx4", "content": "你好", "ts": "..."}
{"sender": "xxxx4", "recipient": "alice", "content": "帮我测一下登录", "ts": "..."}
```

与 `agents/{id}/inbox.jsonl` 的关系：发消息时**同时写两处**——team 流水（全员视角）+ 目标 agent 的 inbox（收件箱）。

### 3.3 tasklist.json — 任务列表

Agent 通过 tool 自主操作（list_tasks / claim_task / complete_task / create_task），系统不强制。

```jsonc
{
    "tasks": [
        {
            "id": "t001",
            "title": "测试登录功能",
            "status": "open",
            "assigned_to": null,
            "created_by": "xxxx4",
            "created_at": "...",
            "completed_at": null,
            "result": null
        }
    ]
}
```

### 3.4 shared/ — 共享目录

Team 成员可以在此目录放共享文件（截图、文档、数据等）。

---

## 四、skills/ — Skill 目录

```
skills/{skill_name}/
└── SKILL.md
```

- 内置 skill（`see_agent/builtin_skills/`）首次启动自动复制到此处，不覆盖已有
- ClawHub 安装的 skill 也放这里
- 搜索路径由 `config.json` 的 `skills.dirs` 配置
- Agent 通过 `agent.json` 的 `skills.disabled` 禁用特定 skill

---

## 五、logs/ — 全局日志

```
logs/YYYY-MM-DD.log
```

- `RotatingFileHandler`，10MB/5 backups
- 全局只记 WARNING 级别的 agent/brain/eye/hand
- 详细 DEBUG 在 session 级别：`agents/{id}/session/session.log`

---

## 六、config.json — 全局配置（重新分组）

所有配置按功能分组。config.json / team.json / agent.json 同构，直接 deep merge。

```jsonc
{
    // ── LLM ──
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o"
    },

    // ── Agent 行为 ──
    "agent": {
        "language": "zh",                // zh | en
        "max_steps": 50,                 // 单次任务最大步数
        "context_engine": "default"      // 上下文引擎
    },

    // ── 屏幕/操作 ──
    "screen": {
        "max_images": 5,                 // 上下文保留截图数
        "screenshot_interval_ms": 800,
        "tool_delay_ms": 200,
        "scaling_enabled": true,
        "scaling_match": "aspect_ratio", // aspect_ratio | pixel_count
        "show_overlay": true
    },

    // ── 记忆 ──
    "memory": {
        "enabled": true,
        "search": { "mode": "bm25" }
    },

    // ── 上下文压缩 ──
    "compact": {
        "enabled": false,
        "context_window": 128000,
        "target_ratio": 0.75,
        "keep_recent": 8,
        "summary_model": ""             // 空 = 用主模型
    },

    // ── Skill ──
    "skills": {
        "dirs": ["~/.see-agent/skills"],
        "disabled": []
    },

    // ── MCP ──
    "mcp": {
        "servers": {
            "tavily": {
                "type": "stdio",
                "command": "npx",
                "args": ["tavily-mcp@latest"],
                "env": { "TAVILY_API_KEY": "xxx" }
            }
        },
        "disabled": []
    },

    // ── 工具 ──
    "tools": {
        "disabled": []                   // 禁用的工具名
    },

    // ── 沙箱 ──
    "sandbox": {
        "profile": "default",
        "extra_read": [],
        "extra_write": []
    },

    // ── 环境变量 ──
    "env": {}
}
```

**全量字段表**：

| 分组 | 字段 | 类型 | 默认值 | 说明 |
|------|------|------|--------|------|
| `llm` | `base_url` | string | `https://api.openai.com/v1` | API 地址 |
| `llm` | `api_key` | string | `""` | API Key |
| `llm` | `model` | string | `gpt-4o` | 模型名 |
| `agent` | `language` | string | `zh` | 语言，影响 system prompt |
| `agent` | `max_steps` | int | `50` | 单次任务最大步数 |
| `agent` | `context_engine` | string | `default` | 上下文引擎 |
| `screen` | `max_images` | int | `5` | 上下文保留截图数 |
| `screen` | `screenshot_interval_ms` | int | `800` | 截图间隔 ms |
| `screen` | `tool_delay_ms` | int | `200` | 工具执行间隔 ms |
| `screen` | `scaling_enabled` | bool | `true` | 屏幕缩放开关 |
| `screen` | `scaling_match` | string | `aspect_ratio` | 缩放策略 |
| `screen` | `show_overlay` | bool | `true` | 覆盖层开关 |
| `memory` | `enabled` | bool | `true` | 记忆开关 |
| `memory` | `search.mode` | string | `bm25` | 搜索模式 |
| `compact` | `enabled` | bool | `false` | 自动压缩开关 |
| `compact` | `context_window` | int | `128000` | 上下文窗口 tokens |
| `compact` | `target_ratio` | float | `0.75` | 压缩触发阈值 |
| `compact` | `keep_recent` | int | `8` | 压缩后保留条数 |
| `compact` | `summary_model` | string | `""` | 摘要模型 |
| `skills` | `dirs` | string[] | `["~/.see-agent/skills"]` | 搜索路径 |
| `skills` | `disabled` | string[] | `[]` | 禁用的 skill |
| `mcp` | `servers` | object | `{}` | MCP server 配置 |
| `mcp` | `disabled` | string[] | `[]` | 禁用的 MCP server |
| `tools` | `disabled` | string[] | `[]` | 禁用的工具 |
| `sandbox` | `profile` | string | `default` | 沙箱 profile |
| `sandbox` | `extra_read` | string[] | `[]` | 额外读权限 |
| `sandbox` | `extra_write` | string[] | `[]` | 额外写权限 |
| `env` | — | object | `{}` | 注入进程的环境变量 |

---

## 七、配置优先级

三份 JSON 同构，直接 deep merge，从低到高：

```
config.json（全局默认）
  ↓ deep merge
team.json（Team 级别覆盖，可选）
  ↓ deep merge
agent.json（Agent 级别覆盖）
```

合并规则：嵌套 dict 递归合并，非 dict 值直接覆盖。
agent.json 只写需要覆盖的字段，不写的继承上层。
team.json 的 config 字段（id/name/members/status/created_at 除外）也参与 merge。

`env` 字段也参与 merge：三层 env 合并后注入 Agent 进程环境变量。

---

## 八、消息流总览

```
用户/Agent 发消息
  → Server (MessageRouter)
  → 写入 teams/{id}/messages.jsonl（Team 流水）
  → 写入 agents/{target}/inbox.jsonl（目标 Agent 收件箱）
  → Agent loop 自动 drain inbox（系统代码，Agent 无感知）
      ├─ priority=normal → 下一轮 loop 开始前批量注入
      └─ priority=steer  → 当前 tool 执行完后立即注入
  → 注入后更新 inbox_cursor.json
```

任务板（tasklist.json）不走消息流，Agent 通过 tool 自主操作。
