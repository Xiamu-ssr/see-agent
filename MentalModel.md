# DESIGN-openclaw.md — see-agent 工作目录与配置规范

> 本文档是 see-agent 的权威设计文档。
> 定义工作目录结构、所有配置文件的全量字段、消息流和记忆机制。
> 代码修改必须与本文档保持一致。

---

# （一）工作目录

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
│       ├── SOUL.md                      # 人格提示词（Agent 可自改）
│       ├── AGENTS.md                    # 操作指南（系统管理）
│       ├── IDENTITY.md                  # 身份：名字/emoji/头像（Agent 可自改）
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
│       ├── messages.jsonl               # Team 通信流水（纯记录，不参与投递）
│       ├── tasklist.json                # 任务列表（Agent 通过 tool 自主操作）
│       └── shared/                      # Team 共享目录
│
├── skills/
│   └── {skill_name}/
│       └── SKILL.md
│
├── plugins/                             # 插件目录
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
    // ── 身份（agent.json 独有）──
    "id": "xxxx4",                       // Agent ID，与目录名一致（必填）

    // ── 以下字段与 config.json 同构，写了就覆盖 ──
    "llm": {
        "model": "claude-opus-4-6"
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
        "servers": {
            "my-mcp": { "type": "stdio", "command": "node", "args": ["server.js"] }
        },
        "disabled": ["tavily"]
    },
    "sandbox": {
        "profile": "default",
        "extra_read": ["/Users/x/Docs"],
        "extra_write": ["/tmp/output"]
    }
}
```

**agent.json 独有字段**：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | string | ✅ | Agent ID，与目录名一致 |

> 显示名、emoji、头像等由 `IDENTITY.md` 管理，Agent 可自行修改。

其余字段与 config.json 同构，写了就覆盖，没写就继承全局值。

### 2.2 IDENTITY.md / SOUL.md / AGENTS.md — Prompt 注入文件

直接放在 `agents/{id}/` 下。注入到 system prompt（详见第二部分）。

| 文件 | 用途 | Agent 可自改 |
|------|------|-------------|
| `IDENTITY.md` | 身份：名字、emoji、头像 | ✅ |
| `AGENTS.md` | 操作指南：工具规则、消息处理、团队协作 | ❌ 系统管理 |
| `SOUL.md` | 人格/性格/核心提示词 | ✅ |
| `memory/MEMORY.md` | 长期记忆 | ✅ Agent 通过 write_memory 写 |

创建 Agent 时从 `see_agent/templates/` 复制模板。截断限制：每文件 20,000 字符，总计 100,000 字符。

### 2.3 inbox.jsonl + inbox_cursor.json — 消息收件箱

消息是"推"的——系统写入、系统消费，Agent 无感知。

#### 写入

所有发给该 Agent 的消息，由 Server/MessageRouter 写入：

```jsonc
{"msg_id": 1, "source": "user",     "sender": "user",  "content": "你好",      "priority": "normal", "ts": "..."}
{"msg_id": 2, "source": "leader",   "sender": "alice", "content": "去测试登录", "priority": "steer",  "ts": "..."}
```

| 字段 | 说明 |
|------|------|
| `msg_id` | 自增 ID |
| `source` | user / leader / teammate / system |
| `sender` | 发送者标识 |
| `content` | 消息内容 |
| `priority` | `normal`（collect）或 `steer`（立即注入） |
| `ts` | ISO-8601 |

#### 消费（系统代码控制）

- **normal**：当前轮 LLM + tool 全部完成后，下一轮开始前批量注入
- **steer**：当前 tool 执行完后，下次调 LLM 前立即注入

消费后更新 `inbox_cursor.json`：`{"last_read_id": 4}`

进程挂掉重启 → 读游标 → 继续消费，不丢消息。

### 2.4 memory/ — 记忆目录

```
memory/
├── MEMORY.md            # 长期记忆（注入 system prompt）
└── YYYY-MM-DD.md        # 日记（不注入，通过 memory_search 检索）
```

记忆全靠 Agent 自觉，系统提供 `memory_search`（BM25 搜索）和 `write_memory`（追加写入）两个 tool。

### 2.5 session/ — 单会话

一个 Agent 永远只有一个会话。

```
session/
├── meta.json
├── messages.jsonl
├── session.log
├── system_prompt_log.md
└── screenshots/
    └── step_NNN.webp
```

#### messages.jsonl

完整对话流水账，compact 不删旧行，只追加 `type=compact` 标记。

| type | 含义 |
|------|------|
| `system` | system prompt |
| `user_task` | 用户任务（含截图） |
| `assistant` | LLM 回复 |
| `tool_result` | 工具结果 |
| `screenshot` | 独立截图 |
| `user_reply` | 用户中途回复 |
| `system_hint` | 系统提示 |
| `compact` | 压缩标记（含 summary + first_kept_msg_id） |
| `inbox` | 来自收件箱的消息（系统注入） |

#### Context Compact

始终开启，不可关闭。

触发前注入静默提醒让 Agent 先存记忆，然后下一轮执行压缩。

compact 后推理携带：system prompt + summary（`[Conversation Summary]`）+ 最近 `keep_recent` 条消息。

---

## 三、teams/ — Team 数据目录

```
teams/{team_id}/
├── team.json
├── messages.jsonl           # 纯记录/流水账，不参与消息投递
├── tasklist.json            # Agent 通过 tool 自主操作
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
    "status": "created",
    "created_at": "2026-03-11T10:20:36+00:00"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | string | ✅ | Team ID（8 位 hex） |
| `name` | string | ✅ | 显示名 |
| `members` | array | ✅ | `[{id, role}]`，leader 通过 role 体现 |
| `status` | string | ❌ | created / running / stopped |
| `created_at` | string | ❌ | ISO-8601 |

### 3.2 messages.jsonl

纯记录。发消息时同时写两处：team 流水（这里）+ 目标 agent 的 inbox。

### 3.3 tasklist.json

Agent 通过 tool 自主操作（list_tasks / claim_task / complete_task / create_task）。

### 3.4 shared/

Team 成员共享文件的目录。

---

## 四、skills/ — Skill 目录

- 内置 skill 首次启动自动复制，不覆盖已有
- 搜索路径由 `skills.dirs` 配置
- Agent 通过 `skills.disabled` 禁用特定 skill

---

## 五、logs/ — 全局日志

按天滚动 `YYYY-MM-DD.log`，10MB/5 backups。全局只记 WARNING，详细 DEBUG 在 `session/session.log`。

---

## 六、config.json — 全局配置

config.json / team.json / agent.json 同构，直接 deep merge。数组直接覆盖（不合并），dict 递归合并。

```jsonc
{
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o"
    },

    "agent": {
        "max_steps": 50,
        "context_engine": "legacy",      // legacy / 未来插件可扩展
        "compact": {                     // 始终开启，不可关闭
            "context_window": 200000,
            "target_ratio": 0.75,
            "keep_recent": 8,
            "summary_model": ""          // 空 = 用主模型
        }
    },

    "screen": {
        "max_images": 5,
        "screenshot_interval_ms": 800,
        "tool_delay_ms": 200,
        "scaling_enabled": true,
        "scaling_match": "aspect_ratio",
        "show_overlay": true
    },

    "skills": {
        "dirs": ["~/.see-agent/skills"],
        "disabled": []
    },

    "mcp": {
        "servers": {},
        "disabled": []
    },

    "tools": {
        "disabled": []
    },

    "sandbox": {
        "profile": "default",
        "extra_read": [],
        "extra_write": []
    },

    "plugins": {
        "enabled": true,
        "dirs": ["~/.see-agent/plugins"]
    },

    "web": {
        "language": "zh"
    },

    "env": {}
}
```

**全量字段表**：

| 分组 | 字段 | 类型 | 默认值 | 说明 |
|------|------|------|--------|------|
| `llm` | `base_url` | string | `https://api.openai.com/v1` | API 地址 |
| `llm` | `api_key` | string | `""` | API Key |
| `llm` | `model` | string | `gpt-4o` | 模型名 |
| `agent` | `max_steps` | int | `50` | 单次任务最大步数 |
| `agent` | `context_engine` | string | `legacy` | 上下文引擎 |
| `agent.compact` | `context_window` | int | `200000` | 上下文窗口 tokens |
| `agent.compact` | `target_ratio` | float | `0.75` | 压缩触发阈值 |
| `agent.compact` | `keep_recent` | int | `8` | 压缩后保留条数 |
| `agent.compact` | `summary_model` | string | `""` | 摘要模型（空=主模型） |
| `screen` | `max_images` | int | `5` | 上下文保留截图数 |
| `screen` | `screenshot_interval_ms` | int | `800` | 截图间隔 ms |
| `screen` | `tool_delay_ms` | int | `200` | 工具间隔 ms |
| `screen` | `scaling_enabled` | bool | `true` | 屏幕缩放开关 |
| `screen` | `scaling_match` | string | `aspect_ratio` | 缩放策略 |
| `screen` | `show_overlay` | bool | `true` | 覆盖层开关 |
| `skills` | `dirs` | string[] | `["~/.see-agent/skills"]` | 搜索路径 |
| `skills` | `disabled` | string[] | `[]` | 禁用的 skill |
| `mcp` | `servers` | object | `{}` | MCP server 配置 |
| `mcp` | `disabled` | string[] | `[]` | 禁用的 MCP server |
| `tools` | `disabled` | string[] | `[]` | 禁用的工具 |
| `sandbox` | `profile` | string | `default` | 沙箱 profile |
| `sandbox` | `extra_read` | string[] | `[]` | 额外读权限 |
| `sandbox` | `extra_write` | string[] | `[]` | 额外写权限 |
| `plugins` | `enabled` | bool | `true` | 插件开关 |
| `plugins` | `dirs` | string[] | `["~/.see-agent/plugins"]` | 插件路径 |
| `web` | `language` | string | `zh` | 前端 UI 语言 |
| `env` | — | object | `{}` | 注入进程的环境变量 |

---

## 七、配置优先级

```
config.json（全局默认）
  ↓ deep merge
team.json（Team 级别，可选）
  ↓ deep merge
agent.json（Agent 级别）
```

合并规则：dict 递归合并，数组直接覆盖（不合并），其他值直接覆盖。

---

## 八、消息流总览

```
发消息 → Server (MessageRouter)
  → 写 teams/{id}/messages.jsonl（流水记录）
  → 写 agents/{target}/inbox.jsonl（收件箱）
  → Agent loop 自动 drain inbox
      ├─ normal → 下一轮开始前批量注入
      └─ steer → 当前 tool 完成后立即注入
  → 更新 inbox_cursor.json
```

任务板（tasklist.json）不走消息流，Agent 通过 tool 自主操作。

---

# （二）提示词

## 一、System Prompt 组装顺序

`build_system_prompt()` 按以下顺序拼接：

```
1. 最小身份声明（硬编码："你是一个能操作 Mac 电脑的 AI 助手..."）
2. IDENTITY.md（Agent 身份）
3. AGENTS.md（操作指南）
4. SOUL.md（人格提示词）
5. memory/MEMORY.md（长期记忆）
6. 约束声明（硬编码：max_steps 限制 + 安全规则）
7. <SKILLS>（启用的 skill 列表，可选）
8. <TEAM_CONTEXT>（Team 信息 + 通信规则，仅在 team 中时注入）
```

每个 md 文件截断 20,000 字符，总计 100,000 字符。

## 二、进入 Team 后的变化

- System prompt 多了 `<TEAM_CONTEXT>` 块（Team 名称、成员列表及角色、当前 Agent 角色、通信规则）
- inbox 会收到 leader/teammate 的消息（系统自动注入，格式：`[source sender]: content`）
- 多了 team tool：send_message / list_tasks / claim_task / complete_task / create_task
- 对话历史、记忆、截图、compact 机制不变
