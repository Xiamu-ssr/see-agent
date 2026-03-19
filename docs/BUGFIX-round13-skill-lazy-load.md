# Round 13: Skill 按需加载 + skills:dirs 改良 + finished 删除 + Team 消息 + 前端

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 67 (P1): Skill 改为按需加载（参考 OpenClaw）

### 现状
`build_system_prompt` 把每个 skill 的 SKILL.md **全文**拼进 system prompt。10 个 skill 可能占几万 token。

### 修复
System prompt 中只注入 skill 的 name + description + location：

```
## Skills

以下是你可以使用的 Skills。需要时用 read 工具读取 SKILL.md 获取详细指南。

<available_skills>
  <skill>
    <name>system-management</name>
    <description>See-Agent-Corp 系统管理指南，包括 Agent 创建/删除、Team 管理等。</description>
    <location>~/.see-agent-corp/agents/system/skills/system-management/SKILL.md</location>
  </skill>
  ...
</available_skills>

使用规则：
- 如果恰好有一个 skill 适用：用 read 工具读取其 SKILL.md，然后按照指南执行
- 如果多个可能适用：选最具体的一个读取
- 如果没有适用的：不要读取任何 SKILL.md
```

**改动位置：** `brain/prompts.rs` 中 skill 注入部分，从拼全文改为拼摘要。`load_skills` 返回的 `Skill` 结构体需要包含 `location` 字段（SKILL.md 的完整路径）。

---

## Bug 68 (P1): skills:dirs 改为额外发现目录

### 现状
`config.json` 和 `agent.json` 的 `skills.dirs` 承担了"默认路径"的职责，覆盖机制导致 system agent 看不到全局 skill。

### 修复

**内置两个固定的默认 skill 发现目录（硬编码在代码中）：**
1. `~/.see-agent-corp/skills/` — 全局 skill（所有 agent 可见）
2. `~/.see-agent-corp/agents/{agent_id}/skills/` — agent 专属 skill

**`skills.dirs` 改为"额外发现目录"**——在默认路径之外追加的。不再覆盖。

加载逻辑变为：
```rust
fn resolve_skill_dirs(agent_id: &str, config: &Config, agent_config: Option<&SkillsConfig>) -> Vec<PathBuf> {
    let mut dirs = vec![
        // 固定默认路径
        workspace.skills(),                          // ~/.see-agent-corp/skills/
        workspace.agent(agent_id).skills(),          // ~/.see-agent-corp/agents/{id}/skills/
    ];
    // 追加全局额外目录
    dirs.extend(config.skills.dirs.iter().map(PathBuf::from));
    // 追加 agent 额外目录（如果有）
    if let Some(agent_cfg) = agent_config {
        dirs.extend(agent_cfg.dirs.iter().map(PathBuf::from));
    }
    dirs
}
```

**同时：**
- 删除 system agent 的 `agent.json` 中 `skills.dirs` 配置（不再需要，默认路径已包含）
- `config.json` 中 `skills.dirs` 默认值改为空数组 `[]`（不需要额外目录）
- `list_agent_skills_handler` API 也用同样的 `resolve_skill_dirs` 逻辑

---

## Bug 69 (P1): 删除 finished tool

### 现状
`run_one_turn` 已经支持"无 tool_calls 就 break"。finished tool 多余。

### 修复
1. 删除 `FinishedTool` 实现和注册
2. 删除 `run_one_turn` 中 `if tc.name == "finished"` 的特殊处理
3. 更新 `templates/AGENTS.md`：删掉"任务完成后必须调用 finished"的指令
4. 如果还有 `call_user` tool，也一并检查是否需要删除

---

## Bug 70 (P2): send_message 同时写入 Team 公共消息

### 现状
`send_message` 只写目标 agent 的 inbox，team 的 `messages.jsonl` 是空壳。

### 修复
`SendMessageTool::execute` 中，如果发送者和接收者属于同一个 team，额外 append 一条到 `teams/{team_id}/messages.jsonl`：

```rust
// 查找发送者的 team
if let Some(team_id) = find_agent_team(&self.ctx.workspace, &self.ctx.agent_id) {
    // 检查接收者也在同一 team
    let team_dir = self.ctx.workspace.team(&team_id);
    let team_def: TeamDefinition = read_json(&team_dir.team_json())?;
    if team_def.members.iter().any(|m| m.id == to) {
        // 写入 team 公共消息
        let team_msg = json!({
            "from": self.ctx.agent_id,
            "to": to,
            "content": content,
            "ts": chrono::Utc::now().to_rfc3339()
        });
        append_jsonl(&team_dir.messages(), &team_msg)?;
    }
}
```

---

## Bug 71 (P2): Team Shared tab 做成文件浏览器

### 现状
Team 详情页 Shared tab 为空或简单展示。

### 修复
复用 Agent Details Files tab 的文件浏览器组件，指向 `teams/{id}/shared/` 目录。支持目录浏览、文件预览、路径复制。

后端需要新增或复用：`GET /api/teams/{id}/files?path=...`

---

## 实施约束

1. 顺序：Bug 68 → Bug 67 → Bug 69 → Bug 70 → Bug 71
2. Bug 68 先改，因为 Bug 67 的 skill location 路径依赖于发现逻辑
3. Bug 69 删 finished 后确保所有测试更新
4. 每步 cargo test
5. 更新 templates/ 中受影响的文件
6. 最后 trunk build --release + git commit + git push
