# Bugfix Round 7: 综合修复

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 32 (P1): 聊天框整页滚动

### 现象
消息多了整个页面出浏览器滚动条，聊天框不是固定容器+内部滚动。

### 修复
Chat 视图的外层容器需要 `h-[calc(100vh-XXpx)]` 或 flex 布局撑满剩余高度。消息区域 `overflow-y: auto`，输入框固定在底部。

关键 CSS 结构：
```
AgentsPage (flex h-screen)
├── 左侧 sidebar (w-56, overflow-y-auto) ← 已有
└── 右侧 panel (flex-1, flex flex-col, min-h-0, overflow-hidden) ← 关键：overflow-hidden 不是 overflow-y-auto
    ├── Header (shrink-0)
    └── Content (flex-1, flex flex-col, min-h-0)
        ├── Messages (flex-1, overflow-y-auto, min-h-0) ← 只有这里有滚动条
        └── Input (shrink-0)
```

当前问题：右侧 panel 有 `overflow-y-auto`，应该改为 `overflow-hidden`。让内部的 messages 区域自己滚动。

---

## Bug 33 (P1): Skill 内容未注入 system prompt

### 现象
`build_system_prompt()` 没有读 Skill 的 SKILL.md，skill 对 agent 行为没有任何影响。

### 修复
在 `build_system_prompt()` 中，拼接完 SOUL + AGENTS + IDENTITY + team 段落后，追加 skill 段落：

```rust
// 5. Skills
let skills = load_skills_for_agent(&agent_dir, &config);
for skill in &skills {
    prompt.push_str(&format!("\n\n## Skill: {}\n{}", skill.name, skill.content));
}
```

需要在 worker.rs 中把加载好的 skills 传给 `build_system_prompt()`。

---

## Bug 34 (P1): 心跳应该也检查 inbox

### 现象
Worker 心跳每 300 秒超时后只检查 TaskBoard（`check_task_board`），不检查 inbox。如果有人给 agent 发了消息但没触发 SIGUSR1（比如 send_message tool 的 wake 失败），消息就一直积在 inbox 里。

### 修复
Worker 心跳超时后，除了 team agent 检查 TaskBoard，所有 agent 都应该 drain inbox：

```rust
// 在 select! 的 timeout 分支中：
// 1. 先 drain inbox（所有 agent）
let (steer, collect) = drain_inbox_split(&inbox_path, &cursor_path)?;
if !steer.is_empty() || !collect.is_empty() {
    // 有新消息，处理
}
// 2. team agent 额外检查 TaskBoard
if is_team_agent {
    check_task_board(...);
}
```

---

## Bug 35 (P2): Team 提示词从硬编码提取到 templates

### 现象
`build_system_prompt()` 中的 team leader/member 提示词是硬编码中文字符串。

### 修复
在 `templates/` 下新增：
- `team_leader_prompt.md`：
```
你是团队 {{team_name}} 的领导（leader）。

团队成员：
{{members}}

职责：
- 使用 create_task 创建任务并分配给成员
- 使用 send_message 与成员沟通
- 监督任务进度，确保按时完成
- 有新任务时主动通知相关成员
```
- `team_member_prompt.md`：
```
你是团队 {{team_name}} 的成员，角色：{{role}}。

团队领导：{{leader_id}}
团队成员：
{{members}}

职责：
- 使用 claim_task 领取任务
- 使用 complete_task 完成任务
- 向领导 {{leader_id}} 汇报进度
- 遇到问题用 send_message 沟通
```

`build_system_prompt()` 读取模板后替换 `{{变量}}`。

---

## Bug 36 (P2): Agent 创建 API 支持 name 和 emoji 参数

### 现象
`POST /api/agents` 创建 agent 时传了 `name` 和 `emoji` 参数但没生效——这些值是从 IDENTITY.md 解析的，API 没有写入。

### 修复
`create_agent_handler` 收到 name 和 emoji 后，写入 IDENTITY.md：

```markdown
# Identity

**Name:** {name}
**Emoji:** {emoji}
```

如果没传 name，用 id 作为默认名。如果没传 emoji，用 🤖。

---

## Bug 37 (P3): 前端小修

1. `AgentsPage` 的右侧 panel 外层 `overflow-y-auto` 改为 `overflow-hidden`
2. 新消息到达时自动滚到底部（messages 容器 scrollTop = scrollHeight）
3. 空聊天时的提示文案改为中文"还没有消息。发送消息开始对话。"

---

## 实施约束

1. **顺序：** Bug 32 → Bug 33 → Bug 34 → Bug 35 → Bug 36 → Bug 37
2. 每步 cargo test
3. trunk build --release 在最后一步
4. 不做兼容，不保留旧代码
5. Bug 33 需要把 skill loader 和 system prompt builder 串起来，请仔细看 `skill/loader.rs` 和 `brain/prompts.rs`
