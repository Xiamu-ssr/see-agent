# MentalModel.md — see-agent 心智模型

> 我（lanxuan）和草莓对齐用的设计文档。只有我说改才能改。

---

# （一）工作目录

## 一、完整目录结构

根目录 `~/.see-agent/`（`SEE_AGENT_HOME` 可覆盖）

```
~/.see-agent/
├── config.json                          # 全局配置（最低优先级）
│
├── agents/system/                       # 内置系统 Agent（首次启动自动创建，不可删除）
├── agents/{agent_id}/
│   ├── agent.json                       # Agent 配置（覆盖全局）
│   ├── IDENTITY.md                      # 身份（Agent 可自改）
│   ├── SOUL.md                          # 人格（Agent 可自改）
│   ├── AGENTS.md                        # 操作指南（系统管理）
│   ├── inbox.jsonl                      # 收件箱（系统写入/消费）
│   ├── inbox_cursor.json                # 已读游标
│   ├── memory/
│   │   ├── MEMORY.md                    # 长期记忆（注入 prompt）
│   │   └── YYYY-MM-DD.md               # 日记
│   └── session/                         # 单会话
│       ├── meta.json
│       ├── messages.jsonl               # 对话历史（含 compact summary）
│       ├── session.log
│       ├── system_prompt_log.md
│       └── screenshots/step_NNN.webp
│
├── teams/{team_id}/
│   ├── team.json
│   ├── messages.jsonl                   # 纯记录，不参与投递
│   ├── tasklist.json                    # Agent 通过 tool 自主操作
│   └── shared/
│
├── skills/{skill_name}/SKILL.md
└── logs/YYYY-MM-DD.log
```

本地 IPC：sock 文件放 `/tmp/see-agent-{agent_id}.sock`。
远程通信：通过 HTTP/WebSocket 连接目标 node 的监听地址。

---

## 二、agents/

### 2.1 agent.json

与 config.json 同构，写了就覆盖，没写就继承。

```jsonc
{
    "id": "xxxx4",                       // 独有字段，与目录名一致
    // 以下与 config.json 同构，只写差异
    "llm": { "model": "claude-opus-4-6" },
    "agent": { "max_steps": 70 },
    "tools": { "disabled": ["shell"] },
    "skills": { "disabled": ["clawhub"] },
    "mcp": { // deep merge 递归到最深 dict：两边都是 dict→递归，否则→右边覆盖。新增的 server 会加入，不会冲掉全局已有的
        "servers": { "my-mcp": { "type": "stdio", "command": "node", "args": ["server.js"] } },
        "disabled": ["tavily"]
    },
    "sandbox": { "profile": "default", "extra_read": [], "extra_write": [] }
}
```

### 2.2 系统 Agent（agents/system/）

首次启动自动创建，不可删除。与普通 Agent 使用相同的基础设施（loop、inbox、memory），区别：

- 挂载管理类工具（manage_agent、manage_team、manage_config），不挂载 screen 类工具
- 用户通过 UI 或 CLI 与它对话来管理配置、创建 agent/team 等
- 模板从 `templates/system/` 复制

### 2.3 IDENTITY.md / SOUL.md / AGENTS.md

创建 Agent 时从 templates/ 复制。

### 2.4 inbox.jsonl + inbox_cursor.json

系统写入、系统消费，Agent 无感知。

```jsonc
{"msg_id": 1, "sender": "user",  "content": "你好",      "priority": "collect", "ts": "..."}
{"msg_id": 2, "sender": "alice", "content": "去测试登录", "priority": "steer",   "ts": "..."}
```

- **collect**：下一轮 loop 开始前批量注入
- **steer**：当前 tool 完成后立即注入

游标 `{"last_read_id": N}`，进程恢复后从游标继续，不丢消息。

### 2.5 memory/

Agent 自觉维护。系统提供 `memory_search`（BM25）和 `write_memory` 两个 tool。

### 2.6 session/

单会话。messages.jsonl 是完整流水账，compact 不删旧行。

compact 触发前注入静默提醒让 Agent 先存记忆。compact 后携带：system prompt + summary + 最近 keep_recent 条。

---

## 三、teams/

### team.json

```jsonc
{
    "id": "04582c0a",
    "name": "测试部",
    "members": [
        { "id": "xxxx4", "role": "项目经理" },
        { "id": "alice", "role": "测试工程师" },
        { "id": "bob",   "role": "开发", "endpoint": "192.168.1.5:8080" }
    ],
    "leader": "xxxx4",
    "status": "created",
    "created_at": "..."
}
```

leader 通过 role 体现，同时保留 `"leader": "xxxx4"` 字段给代码用。role 是业务描述（给 prompt 看），leader 是技术标识（给代码判断权限用）。

成员无 `endpoint` 字段 = 本地 agent，有 `endpoint` = 远程 agent（消息通过网络投递到目标 node）。

### messages.jsonl

纯记录。发消息时同时写 team 流水 + 目标 agent 的 inbox。

### tasklist.json

Agent 通过 tool 自主操作。

---

## 四、skills / logs

- skills：内置自动复制，搜索路径 `skills.dirs`，禁用 `skills.disabled`
- logs：按天滚动，全局 WARNING，详细 DEBUG 在 session.log

> plugins 系统已废弃，扩展能力统一通过 MCP servers 提供。

---

## 五、config.json

config / team / agent 三份 JSON 同构，deep merge。dict 递归合并，数组直接覆盖。

```jsonc
{
    "node": {
        "id": "",                        // 本机标识（默认用 hostname）
        "listen": ""                     // 对外监听地址（空 = 不接受远程连接，纯本地模式）
    },
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o"
    },
    "agent": {
        "max_steps": 50,                 // 单次任务最大步数
        "compact": {                     // 始终开启
            "context_window": 200000,    // 上下文窗口 tokens
            "target_ratio": 0.75,        // 触发阈值
            "keep_recent": 8,            // 压缩后保留条数
            "summary_model": ""          // 空 = 用主模型
        }
    },
    "screen": {
        "max_images": 5,                 // 上下文保留截图数
        "screenshot_interval_ms": 800,
        "tool_delay_ms": 200,            // 工具间隔 ms
        "scaling_enabled": true,
        "scaling_match": "aspect_ratio", // aspect_ratio | pixel_count
        "show_overlay": true
    },
    "skills": {
        "dirs": ["~/.see-agent/skills"],
        "disabled": []
    },
    "mcp": {
        "servers": {},                   // MCP server 配置
        "disabled": []
    },
    "tools": {
        "disabled": []                   // 禁用的工具
    },
    "sandbox": {
        "profile": "default",            // shell 工具的文件系统访问限制
        "extra_read": [],
        "extra_write": []
    },
    "web": {
        "language": "zh"                 // 前端 UI 语言
    },
    "env": {}                            // 注入进程的环境变量
}
```

### 配置优先级

config.json → team.json → agent.json（deep merge，数组覆盖）

---

## 六、消息流

```
发消息 → 写 team/messages.jsonl → 判断目标 agent 位置
  → 本地 agent：直接写 agent/inbox.jsonl
  → 远程 agent：HTTP/WebSocket 发到目标 node → 目标 node 写 inbox.jsonl
  → Agent loop drain inbox（collect=下轮批量，steer=立即注入）
  → 更新 inbox_cursor.json
```

Agent loop 的 drain 逻辑不区分消息来源（本地/远程），只读本地 inbox.jsonl。

任务板不走消息流，Agent 通过 tool 操作。

---

# （二）提示词

## System Prompt 组装顺序

```
1. IDENTITY.md
2. AGENTS.md
3. SOUL.md
4. memory/MEMORY.md
5. 约束声明（max_steps + 安全规则，硬编码）
6. <SKILLS>（可选）
7. <TEAM_CONTEXT>（可选：Team 名称、成员及角色、通信规则）
```

进入 Team 后：多了 `<TEAM_CONTEXT>`，inbox 多来源消息（`[sender]: content`），多 team tool。其余不变。
