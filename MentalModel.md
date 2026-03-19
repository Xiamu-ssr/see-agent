# MentalModel.md — see-agent 心智模型

> 我（lanxuan）和草莓对齐用的设计文档。只有我说改才能改。

---

# （一）工作目录

## 一、完整目录结构

根目录 `~/.see-agent-corp/`（`SAC_HOME` 可覆盖）

```
~/.see-agent-corp/
├── config.json                          # 全局配置（最低优先级）
│
├── agents/system/                       # 内置系统 Agent（首次启动自动创建，不可删除）
├── agents/{agent_id}/
│   ├── agent.json                       # Agent 配置（覆盖全局）
│   ├── IDENTITY.md                      # 身份（Agent 可自改）
│   ├── SOUL.md                          # 人格（Agent 可自改）
│   ├── AGENTS.md                        # 操作指南（系统管理）
│   ├── inbox.jsonl                      # 收件箱（系统写入/消费）
│   ├── inbox_cursor.json                # 双游标 {"collect": N, "steer": M}
│   ├── worker.pid                       # Worker 进程 PID
│   ├── worker.log                       # Worker 日志
│   ├── skills/{skill_name}/SKILL.md     # Agent 专属 Skills
│   ├── memory/
│   │   ├── MEMORY.md                    # 长期记忆（注入 prompt）
│   │   └── YYYY-MM-DD.md               # 日记
│   └── session/                         # 单会话
│       ├── messages.jsonl               # 对话历史（含 compact summary）
│       ├── last_llm_call.json           # 最近一次 LLM 调用快照
│       └── screenshots/step_NNN.webp
│
├── teams/{team_id}/
│   ├── team.json
│   ├── messages.jsonl                   # 纯记录，不参与投递
│   ├── tasklist.json                    # Agent 通过 tool 自主操作
│   └── shared/
│
├── skills/{skill_name}/SKILL.md
├── server.pid                           # 服务进程 PID
└── server.log                           # 服务日志
```

本地 IPC：Worker 进程由 supervisor 通过 SIGUSR1 唤醒。

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

- 挂载管理类工具（通过 CLI 命令管理 agent/team），不挂载 screen 类工具
- 用户通过 UI 或 CLI 与它对话来管理配置、创建 agent/team 等
- 拥有专属 skill（system-management），描述了系统设计和 CLI 命令
- 模板从 `templates/` 复制

### 2.3 IDENTITY.md / SOUL.md / AGENTS.md

创建 Agent 时从 templates/ 复制。

### 2.4 inbox.jsonl + inbox_cursor.json

系统写入、系统消费，Agent 无感知。

```jsonc
{"msg_id": 1, "sender": "user",  "content": "你好",      "priority": "collect", "ts": "..."}
{"msg_id": 2, "sender": "alice", "content": "去测试登录", "priority": "steer",   "ts": "..."}
```

- **collect**：下一轮 loop 开始前批量注入
- **steer**：推理循环中下一次 LLM 调用前注入（通过 drain_steer_only + steer cursor）

双游标机制：
- `collect cursor`：外层主循环用，读取所有消息
- `steer cursor`：推理循环内用，只读取 steer 消息
- 格式：`{"collect": N, "steer": M}`

游标保证进程恢复后从断点继续，不丢消息。Worker 启动时第一次 drain 会 filter 掉历史 shutdown 消息。

### 2.5 memory/

Agent 自觉维护。系统提供 `memory_search`（BM25）和 `memory_get`（按行号精确读取）两个 tool。写入靠通用 `write` tool。

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

- skills：内置两个固定发现路径（`~/.see-agent-corp/skills/` 全局 + `agents/{id}/skills/` 专属），`skills.dirs` 为额外追加目录，`skills.disabled` 按名字禁用
- logs：server.log（全局）+ agents/{id}/worker.log（各 agent）

> plugins 系统已废弃，扩展能力统一通过 MCP servers 提供。

---

## 五、config.json

config / team / agent 三份 JSON 同构，deep merge。dict 递归合并，数组直接覆盖。

```jsonc
{
    "node": {
        "id": "",                        // 本机标识（默认用 hostname）
        "listen": ""                     // 对外监听地址（空 = 纯本地模式）
    },
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o"
    },
    "agent": {
        "max_steps": 50,                 // 单次任务最大步数
        "compact": {                     // 四层压缩机制
            "context_window": 200000,    // 上下文窗口 tokens
            "microcompact_ratio": 0.30,  // Layer 2：清旧 tool_result
            "full_compact_ratio": 0.95,  // Layer 3：LLM summarize
            "keep_recent": 8,            // 压缩后保留条数
            "summary_model": "",         // 空 = 用主模型
            "image_high_count": 3,       // 图片消退 Level 1（detail: high）
            "image_low_count": 3         // 图片消退 Level 2（detail: low）
        }
    },
    "skills": {
        "dirs": [],                      // 额外 skill 发现目录（追加到默认路径）
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
发消息 → 同 team 则写 team/messages.jsonl → 写目标 agent/inbox.jsonl → 发 SIGUSR1 唤醒 Worker
  → Agent Worker drain inbox（collect=外层循环批量，steer=推理循环内即时注入）
  → 更新 inbox_cursor.json（双 cursor）
```

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
6. <SKILLS>（按需加载：只注入 name + description + location，Agent 用 read tool 读全文）
7. <TEAM_CONTEXT>（可选：来自 templates/team_leader_prompt.md 或 team_member_prompt.md，{{变量}} 替换）
```

进入 Team 后：多了 `<TEAM_CONTEXT>`，inbox 多来源消息（`[用户] content` / `[agent_id] content`），多 team tool。其余不变。

### 四层压缩机制

```
Layer 1: Tool 输出截断（写入时立即生效）
  └── shell: 30,000 字符
  └── read:  50,000 字符

Layer 2: Microcompact（microcompact_ratio 触发）
  └── 旧 tool_result 内容 → "[tool output cleared — microcompact]"
  └── 只改内存，不改 JSONL

Layer 3: Full Compact（full_compact_ratio 触发）
  └── 第一次：警告 system_hint，让 agent 先写记忆
  └── 第二次：LLM summarize → messages = [system, summary, 最近 keep_recent 条]

Layer 4: 图片消退（get_messages_for_llm 时应用，不改内存）
  └── Level 1: 最新 image_high_count 张 → detail: "high"
  └── Level 2: 次新 image_low_count 张 → detail: "low"
  └── Level 3: 更早的 → "[Screenshot omitted]"
```
