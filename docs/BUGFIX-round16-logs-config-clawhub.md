# Round 16: Logs 修复 + Config 嵌套 + 内置 ClawHub Skill

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 78 (P1): /logs 页面始终空

### 现象
Logs 页面 "No log entries"。server.log 有内容但 API 返回空。

### 排查方向
检查 `get_logs_handler` 是否正确读取 `~/.see-agent-corp/server.log` + `agents/*/worker.log`。可能路径拼错、文件为空、或者解析 tracing 格式的正则不匹配。

### 修复
确保 get_logs_handler：
1. 读取 server.log（workspace 根目录下）
2. 读取所有 agents/*/worker.log
3. 按时间倒序合并
4. 每条加 source 字段（"server" / "agent:system"）
5. 日志等级着色（前端 ERROR 红 / WARN 橙 / INFO 默认 / DEBUG 灰）

---

## Bug 79 (P2): Config 页面嵌套对象没展开

### 现象
Config 页面的 `compact` 字段显示为空文本框，没展开子字段（context_window、microcompact_ratio 等）。

### 修复
前端 Config 渲染递归处理 JSON Schema 的 `type: "object"` + `properties`。遇到嵌套对象时递归渲染子字段，用缩进或折叠区域。

---

## Bug 80 (P1): 内置 ClawHub Skill

### 设计
初始化 workspace 时在 `~/.see-agent-corp/skills/` 下创建内置的 `clawhub` skill。

`~/.see-agent-corp/skills/clawhub/SKILL.md`:
```markdown
---
name: clawhub
description: 从 ClawHub 搜索和安装 Skills 到 agent 的专属目录。
---

# ClawHub Skill

## 用途
ClawHub 是 Claw Race 的 Skill 市场。你可以搜索、浏览和安装 Skills。

## 安装 Skill 到 Agent
安装的 Skill 放到 agent 的专属目录：`~/.see-agent-corp/agents/{agent_id}/skills/`

## 搜索
访问 https://clawhub.com 浏览可用的 Skills。

## 手动安装
将 SKILL.md 文件放到对应目录即可：
- 全局（所有 agent 可用）：`~/.see-agent-corp/skills/{skill_name}/SKILL.md`
- Agent 专属：`~/.see-agent-corp/agents/{id}/skills/{skill_name}/SKILL.md`
```

### 实现
在 `ensure_workspace()` 中，如果 `skills/clawhub/` 不存在，创建并写入 SKILL.md。模板从 `templates/clawhub-skill/SKILL.md`（需要新建）读取。

---

## 实施约束

1. 顺序：Bug 78 → Bug 79 → Bug 80
2. Bug 78 重点排查 get_logs_handler 的文件路径和 tracing 格式解析
3. Bug 79 需要递归渲染 JSON Schema
4. 每步 cargo test
5. 最后 trunk build --release + git commit + git push
