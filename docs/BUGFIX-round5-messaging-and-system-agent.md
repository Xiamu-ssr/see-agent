# Bugfix Round 5: 消息身份 + send_message 唤醒 + System Agent Skill + 前端分组

## 报告人
草莓🍓 | 2026-03-19

---

## 🔴 Bug 23 (Critical): send_message tool 不唤醒目标 Agent 的 Worker

### 现象
小明用 send_message 给小红和小李发消息，消息写入了对方 inbox.jsonl，但对方 Worker 没有被唤醒。小红最终是靠 300 秒心跳超时后检查 TaskBoard 才干活的（不是因为收到了消息）。小李则完全没反应。

### 根因
`see-agent-corp/src/tool/builtin/team.rs` 第 108 行，`SendMessageTool::execute()` 只调用了 `send_to_inbox_with_id()` 写文件，**没有发 SIGUSR1 唤醒对方 Worker**。

对比 `supervisor.send_to()` 的流程：写 inbox + signal_process(pid)——tool 缺了第二步。

### 修复
SendMessageTool 需要能触发目标 Worker 的唤醒。两种方案：

**方案 A（推荐）：** 在 ToolContext 中持有 supervisor 的引用（或一个唤醒 channel），tool 执行后通过它发信号。

**方案 B：** 写一个 `.wake` 信号文件到目标 agent 目录，Worker 的 drain loop 定期检查这个文件。（不推荐，延迟大）

**方案 C：** 通过 HTTP API 回调 `POST /api/agents/{to}/wake`，让 supervisor 发信号。SendMessageTool 需要知道 server 地址。

**最简方案：** 在 ToolContext 中加一个 `signal_fn: Option<Arc<dyn Fn(&str) + Send + Sync>>` 回调，supervisor 启动 worker 时注入。tool 执行后调用 `(signal_fn)(target_agent_id)`。

---

## 🟡 Bug 24: 消息进入 context 时缺少发送者标识

### 现象
小明收到用户消息时不知道是谁发的，问"你是谁"。

### 根因
`run_one_turn` 第 430 行：

```rust
let formatted = if !sender.is_empty() && sender != "user" {
    format!("[{sender}] {text}")
} else {
    text.to_owned()
};
```

用户消息 sender="user"，不加前缀——对于单 agent 没问题，但对于 team agent，收到"user"的消息和收到队友的消息格式一样，无法区分。

### 修复
所有消息都加发送者前缀：

```rust
let label = match sender {
    "user" => "[用户]",
    "system" | "supervisor" => "[系统]",
    s => &format!("[{s}]"),
};
let formatted = format!("{label} {text}");
```

同时 `messages.jsonl` 的 `user_reply` 类型也需要记录 sender 字段。

---

## 🟡 Bug 25: System Agent 缺少专属 Skill 和 Skill 路径配置

### 现象
System Agent 被初始化了（`is_system: true`），但没有专属 skill，也没有配置 skill 发现路径。

### 修复

1. **创建 skill 目录和 SKILL.md：**

在 `templates/` 下新增 `system-skill/SKILL.md`：

```markdown
---
name: system-management
description: See-Agent-Corp 系统管理指南，包括 Agent 创建/删除、Team 管理、任务分配、配置修改等操作。
---

# System Management Skill

## 你是谁
你是 See-Agent-Corp 的系统管理 Agent。你负责维护整个平台，帮助用户创建 Agent、组建 Team、分配任务。

## 可用的管理操作

### Agent 管理
- 创建 Agent：告诉用户通过前端或 CLI `see-agent-corp agent create <id>` 创建
- 删除 Agent：`see-agent-corp agent delete <id>`
- 查看列表：`see-agent-corp agent list`

### Team 管理
- 创建 Team：通过前端或 CLI
- 添加/移除成员
- 设置 Leader

### 系统配置
- 配置文件：`~/.see-agent-corp/config.json`
- Agent 配置：`~/.see-agent-corp/agents/<id>/agent.json`
- Team 配置：`~/.see-agent-corp/teams/<id>/team.json`

### 目录结构
（描述完整的 workspace 目录结构）
```

2. **`ensure_workspace()` 初始化 system agent 时：**
   - 创建 `agents/system/skills/` 目录
   - 复制 `system-skill/SKILL.md` 到 `agents/system/skills/system-management/SKILL.md`

3. **System agent 的 `agent.json` 增加 skill 路径配置：**

```json
{
  "id": "system",
  "is_system": true,
  "skills": {
    "dirs": ["~/.see-agent-corp/agents/system/skills"],
    "disabled": []
  }
}
```

这样只有 system agent 能看到这个 skill（其他 agent 的 skills.dirs 不包含这个路径）。

---

## 🟡 Bug 26: 前端 Agent 列表按 Team 分组

### 现状
Agent 列表平铺展示，没有分组。

### 修复

前端 Agent 列表按以下顺序分组：

```
⚙️ System
─────────────────
📋 产品团队
  🔬 小明 (leader)
  🎨 小红 (designer)
  💻 小李 (developer)
─────────────────
📋 无 Team
  🤖 其他agent...
```

**后端辅助：** `GET /api/agents` 返回的每个 agent 已经有 `team_id` 字段（可能需要检查是否正确填充）。前端根据 team_id 分组。

如果 team_id 目前没正确填充，需要在 `list_agents()` 中用 `find_agent_team()` 查找并填入。

---

## 🟡 Bug 27: 前端 Details-Files tab 增加复制路径按钮

Agent 详情页 Files tab 中，每个文件名旁边加一个 📋 图标按钮，点击复制该文件的完整路径到剪贴板。

---

## 🟢 Bug 28: Mode A (run) 是否可以废弃

Mode A 是旧的 `see-agent-corp run "task"` 命令——一次性任务执行模式，截图→LLM→tool→loop→finished 退出。

**不建议现在删除**——它仍然是有用的 CLI 模式（快速执行单个屏幕任务）。但可以把它标记为"legacy"，核心开发集中在 Mode B（Worker/inbox 模式）。

暂不处理。

---

## 🟢 Bug 29: templates 目录维护

当前 `templates/` 包含：
- `AGENTS.md` — agent 行为规范模板
- `IDENTITY.md` — 身份模板
- `SOUL.md` — 人格模板

缺少：
- system agent 的专属模板（见 Bug 25）
- `agent.json` 默认模板（当前硬编码在 `AgentDefinition::new()`）

需要在 templates 中维护所有默认文件模板，`create_agent()` 从 templates 复制。

---

## 实施约束

1. **Bug 23 最先修**——Critical，agent 间通信断裂
2. **Bug 24 次之**——用户体验问题
3. **Bug 25、26、27 可以并行**
4. 每步 cargo test
5. 不做兼容，不保留旧代码
6. Bug 23 的唤醒方案需要 supervisor 协作，请仔细设计 ToolContext 的扩展
