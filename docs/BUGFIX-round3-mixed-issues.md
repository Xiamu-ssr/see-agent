# Bugfix Round 3: 混合问题修复

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 8: 记录最近一次 LLM 调用的 system_prompt + tools

### 现状
`brain.chat()` 每次调用时传入 `messages` + `tools`，但调用完就丢了。目前只有 `messages.jsonl` 记录消息数组，没有记录 system_prompt 和 tools schema。`system_prompt_log.md` 路径在 `paths.rs:132` 定义了但全项目无任何代码写入。

### 修复
在 `session/` 下新增 `last_llm_call.json`，每次 `brain.chat()` 调用前覆写：

```json
{
  "timestamp": "2026-03-19T03:00:00Z",
  "model": "anthropic/claude-opus-4.6",
  "system_prompt": "You are...",
  "tools": [...],
  "max_tokens": 4096,
  "message_count": 15,
  "estimated_tokens": 8234
}
```

**实现位置：** `loop_core.rs` 中调用 `brain.chat()` 的地方（有两处：`run_loop` 和 `run_one_turn`），在调用前写文件。需要把 `session_dir` / `SessionStore` 的路径传进来。

**注意：** 只保留最近一次，覆写不追加。system_prompt 可能很大（几千字），但只存一份不会膨胀。

同时删除 `paths.rs` 中 `system_prompt_log()` 的死代码定义。

---

## Bug 9: /skills 返回空数组

### 现状
`config.json` 中 `skills.dirs: []` 为空数组。`load_skills()` 遍历 dirs 扫描 SKILL.md，dirs 空 = 0 个 skill。但 `~/.see-agent-corp/skills/` 目录已经存在。

### 修复
`ensure_workspace()` 初始化 config.json 时，`skills.dirs` 默认值应包含 workspace 下的 skills 目录：

```json
{
  "skills": {
    "dirs": ["~/.see-agent-corp/skills"],
    "disabled": []
  }
}
```

**注意：** 不要破坏已有 config.json。如果用户已经手动配置了 dirs，不要覆盖。只在新建 config 时设默认值。

---

## Bug 10: /logs 返回空数组

### 现状
`get_logs_handler` 从 `workspace.logs()` 目录读文件，但该目录为空。Server 日志写到 `server.log`（workspace 根），Worker 日志写到 `agents/{id}/worker.log`——都不在 `logs/` 目录下。

### 修复
改写 `get_logs_handler`，聚合以下日志源：

1. `~/.see-agent-corp/server.log` — 全局 server/supervisor 日志
2. `~/.see-agent-corp/agents/*/worker.log` — 各 agent 的 worker 日志

读取最后 N 行（`LOG_TAIL_LINES` 常量），按时间排序合并返回。每条加 `source` 字段区分来源。

如果 `logs/` 目录的设计不再需要，可以删掉 `workspace.logs()` 路径定义和空目录创建。

---

## Bug 12: Mode A 首次自动截图删掉

### 现状
`loop_core.rs` 的 `run()` 方法（Mode A）在启动时会自动调 `eye.capture()` 截一张屏幕，拼到第一条 user_task 里：

```rust
// Initial screenshot  ← 这段要删
let screenshot = if has_screen {
    match self.eye.capture().await {
        ...
    }
} else {
    None
};
```

这是旧 computer-use-demo 的设计遗留。按 see-agent-corp 的设计哲学，Agent 应该自主决定什么时候看屏幕（通过调用 `screenshot` tool），系统不应偷偷截图。

### 修复
删除 `run()` 中的 initial screenshot 逻辑。第一条消息永远用 `add_user_task_text_only()`。Agent 如果需要看屏幕，自己调 screenshot tool。

同时删除 `run_loop()` 的 `initial_scaled` 参数（不再需要）。

---

## Bug 13: 前端 Agent 页面新增 Log tab

### 现状
前端 Agent 详情页只有 Chat 面板，没有日志查看能力。Worker 崩溃或 LLM 调用失败时，用户无法从前端看到原因。

### 修复

**后端：** 新增 API `GET /api/agents/{agent_id}/logs`

```rust
async fn get_agent_logs_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<Vec<LogLine>>, StatusCode> {
    let agent_dir = state.workspace().agent(&agent_id);
    let log_path = agent_dir.path().join("worker.log");
    // 读取最后 200 行
    // 返回 Vec<LogLine> { time, level, message }
}
```

路由注册到 agents router。

**前端：** Agent 详情页新增一个 "Logs" tab（和 Chat tab 并列），内容是 `worker.log` 的最新内容，5 秒轮询刷新。用 `<pre>` 或代码块样式展示，自动滚到底部。

---

## 实施约束

1. **顺序：** Bug 12 → Bug 8 → Bug 9 → Bug 10 → Bug 13
2. **每步 cargo test**
3. **不做兼容**，不保留旧代码
4. **Bug 12 删代码时**确保 Mode A 的测试也更新（如果有测试依赖 initial screenshot）
5. **Bug 8** 的文件写入要用原子写（写 tmp → rename），避免读到半写的 JSON
6. **Bug 13 前端**用现有的 DaisyUI tab 组件，保持一致风格
