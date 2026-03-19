# Bugfix Report: Inbox Cursor 初始化 + Session Restore 缺失

## 报告人
草莓🍓 | 2026-03-19

## 严重程度：🔴 Critical
这两个 bug 导致 **Agent 完全无法回复消息**。

---

## Bug 1: Worker 读到历史 Shutdown 消息后立即退出

### 现象
前端给 Agent 发消息 → Agent 无回复，状态始终 Sleeping。

### 根因
1. `create_agent()` 没有初始化 `inbox_cursor.json`
2. `inbox.jsonl` 有 `append_jsonl` 追加写入，从不清理
3. Worker 启动时如果没有 cursor 文件，`drain_inbox_split()` 从第 0 行读全部历史
4. 历史中包含之前的 shutdown 消息 → Worker 读到后立即 `return` 退出
5. Worker 退出 → supervisor 的 `is_running()` 检测到死亡 → 清理 → 下次 `send_to` 又自动启动 → 又读到 shutdown → 无限循环

### 复现
```bash
# inbox 里有旧的 shutdown 消息（msg_id 1, 3, 5, 8）
cat ~/.see-agent-corp/agents/coder/inbox.jsonl | grep shutdown
# 没有 cursor 文件
ls ~/.see-agent-corp/agents/coder/inbox_cursor.json  # 不存在

# 手动启动 worker
./target/release/see-agent-corp worker coder ~/.see-agent-corp
# 输出：
# INFO worker starting agent="coder"
# INFO entering inbox loop
# INFO received shutdown, exiting  ← 读到历史 shutdown，立即退出
```

### 修复方案

#### 改动 1：`create_agent()` 初始化 cursor 文件

**文件：** `see-agent-corp/src/agent/definition.rs`

在 `create_agent()` 函数中，在 `write_text(&agent_dir.memory_md(), "")?;` 之后，新增：

```rust
// Initialize inbox and cursor so worker starts clean
write_text(&agent_dir.inbox(), "")?;
write_json(&agent_dir.inbox_cursor(), &serde_json::json!({"line": 0}))?;
```

#### 改动 2：`drain_inbox_split()` 首次无 cursor 时跳过历史

**文件：** `see-agent-corp/src/supervisor/inbox.rs`

在 `drain_inbox_split()` 读取 cursor 的逻辑中，如果 cursor 文件不存在，不要从 0 开始读，而是：

```rust
// 如果 cursor 文件不存在（说明是老 agent 或从未初始化），
// 跳过全部历史消息，只处理之后新到的
let cursor = if cursor_path.exists() {
    read_cursor(cursor_path)?
} else {
    let all_messages: Vec<Message> = read_jsonl(inbox_path)?;
    let skip_to = all_messages.len();
    write_cursor(cursor_path, skip_to)?;
    skip_to
};
```

#### 改动 3：Worker 的 shutdown 检测应该忽略历史 shutdown

**文件：** `see-agent-corp-app/src/cli/worker.rs`

这是额外保险。在 drain 循环中，shutdown 检测之前增加判断——只有在 Worker 本次启动后收到的 shutdown 才退出（即 cursor 是 Worker 启动后推进的）。

但如果改动 1+2 做了，这个就不需要了。二选一即可。

---

## Bug 2: Worker 重启后上下文丢失（Session Restore 未接入）

### 现象
Worker 被 kill（supervisor.stop_agent）后重新启动，之前的对话上下文完全丢失。Agent 从空白状态开始，不记得之前说了什么。

### 根因

`worker.rs` 第 82-83 行：

```rust
let mut conv_ctx =
    ConversationContext::new(&system_prompt, config.agent.max_images as usize, None);
```

**永远创建空白 context**。虽然 `SessionStore` 有 `read_for_restore()` 方法，`ConversationContext` 有 `for_restore()` + `inject_summary()` + `push_raw()` 方法，但 **worker.rs 从未调用它们**。

### 修复方案

**文件：** `see-agent-corp-app/src/cli/worker.rs`

将 step 8（Create conversation context）替换为：

```rust
// 8. Create conversation context (with restore if previous session exists)
let mut conv_ctx = {
    let session_dir_for_restore = agent_dir.session();
    let mut restore_store = SessionStore::new(session_dir_for_restore);

    if restore_store.dir().messages().exists() {
        match restore_store.read_for_restore() {
            Ok((Some(summary), kept_msgs)) if !kept_msgs.is_empty() => {
                info!(
                    agent = agent_id,
                    kept = kept_msgs.len(),
                    "restoring session from disk"
                );
                let mut ctx = ConversationContext::for_restore(
                    config.agent.max_images as usize,
                );
                // System prompt as first message
                ctx.push_raw(serde_json::json!({
                    "role": "system",
                    "content": &system_prompt
                }));
                // Inject compact summary at index 1
                ctx.inject_summary(&summary);
                // Replay kept messages
                for msg in &kept_msgs {
                    if let Some(openai_msg) = session_msg_to_openai(msg) {
                        ctx.push_raw(openai_msg);
                    }
                }
                ctx
            }
            Ok((None, kept_msgs)) if !kept_msgs.is_empty() => {
                info!(
                    agent = agent_id,
                    kept = kept_msgs.len(),
                    "restoring session (no compact summary)"
                );
                let mut ctx = ConversationContext::for_restore(
                    config.agent.max_images as usize,
                );
                ctx.push_raw(serde_json::json!({
                    "role": "system",
                    "content": &system_prompt
                }));
                for msg in &kept_msgs {
                    if let Some(openai_msg) = session_msg_to_openai(msg) {
                        ctx.push_raw(openai_msg);
                    }
                }
                ctx
            }
            _ => {
                info!(agent = agent_id, "no previous session, starting fresh");
                ConversationContext::new(
                    &system_prompt,
                    config.agent.max_images as usize,
                    None,
                )
            }
        }
    } else {
        ConversationContext::new(
            &system_prompt,
            config.agent.max_images as usize,
            None,
        )
    }
};
```

还需要新增一个辅助函数 `session_msg_to_openai()`，将 `SessionMessage` 转换回 OpenAI 格式的 `Value`：

```rust
/// Convert a SessionMessage back to OpenAI chat format for context restore.
fn session_msg_to_openai(msg: &see_agent_corp::types::SessionMessage) -> Option<serde_json::Value> {
    use see_agent_corp::types::SessionMessageType;
    match msg.msg_type {
        SessionMessageType::UserTask | SessionMessageType::UserReply => {
            let text = msg.data.get("text")
                .or_else(|| msg.data.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(serde_json::json!({"role": "user", "content": text}))
        }
        SessionMessageType::Assistant => {
            let mut m = serde_json::json!({"role": "assistant"});
            if let Some(content) = msg.data.get("content") {
                m["content"] = content.clone();
            }
            // Note: tool_calls stored in flattened form, not full OpenAI format.
            // Omit them on restore — LLM doesn't need to see old tool_calls.
            Some(m)
        }
        SessionMessageType::ToolResult => {
            // Tool results without matching tool_calls confuse the API.
            // Skip on restore — the assistant response already captured the outcome.
            None
        }
        SessionMessageType::SystemHint => {
            let text = msg.data.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(serde_json::json!({"role": "user", "content": text}))
        }
        SessionMessageType::Screenshot => {
            // Screenshots are path-ref based, may not exist after restart.
            // Skip on restore.
            None
        }
        _ => None,
    }
}
```

**重要：** restore 完成后要激活 `on_append` 回调，否则新消息不会持久化到 messages.jsonl。在 restore 块的最后加上 `ctx.set_on_append(...)` 调用（需要和后面 step 6c 的 session_store 协调）。

---

## Bug 3: `send_message_handler` 的双写不一致

### 现象
轻微问题。前端发消息时 `send_message_handler` 直接写 `session/messages.jsonl`，但 Worker 的 `on_append` 回调也会在处理 inbox 消息时写同一条消息，导致**重复记录**。

### 根因

`see-agent-corp-app/src/server/routes/agents.rs` 的 `send_message_handler()`：

```rust
// Write user message to session store so it's immediately visible in chat UI
let _ = session_store.append_message(
    SessionMessageType::UserTask,
    serde_json::json!({ "content": content }),
);
```

同时，Worker 在 `run_one_turn()` 中处理这条消息时，通过 `add_user_task_text_only()` 又触发 `on_append` 写入一条 `user_task`。

### 修复方案

两种选择：

**方案 A（推荐）：** 在 Worker 端去重。Worker 处理 inbox 消息时，如果 messages.jsonl 最后一条已经是相同内容的 `user_task`，跳过 `on_append` 写入。

**方案 B：** `send_message_handler` 不直接写 messages.jsonl，改为只写 inbox。前端在 Worker 处理前看到空消息是可接受的（loading 状态）。

---

## 实施约束（给 CC）

1. **不做兼容**：对已存在但没有 `inbox_cursor.json` 的 agent，首次 Worker 启动时自动创建 cursor 跳过历史。不写迁移脚本。
2. **必须删旧代码**：如果改了 `drain_inbox_split()` 的 cursor 初始化逻辑，删掉旧的 fallback 路径。
3. **每步跑 check.sh**：改一个 bug 就 `cargo test`，不要攒到最后。
4. **先修 Bug 1，再修 Bug 2**：Bug 1 不修的话 Worker 根本跑不起来，Bug 2 无从验证。
5. **Bug 3 可以放后面**：不阻塞核心功能。

## 验证步骤

### Bug 1 验证
```bash
# 1. 删掉 coder 的 inbox_cursor.json（如果存在）
rm -f ~/.see-agent-corp/agents/coder/inbox_cursor.json

# 2. 确保 inbox.jsonl 里有旧 shutdown 消息
grep shutdown ~/.see-agent-corp/agents/coder/inbox.jsonl

# 3. 启动 worker
./target/release/see-agent-corp worker coder ~/.see-agent-corp

# 期望：Worker 启动，跳过历史，进入 idle 等待
# 而不是立即退出

# 4. 发一条新消息
echo '{"msg_id":null,"sender":"user","content":"测试","priority":"collect","metadata":{},"ts":"2026-03-19T03:00:00Z"}' >> ~/.see-agent-corp/agents/coder/inbox.jsonl
kill -USR1 <worker_pid>

# 期望：Worker 处理消息，调用 LLM，回复出现在 session/messages.jsonl
```

### Bug 2 验证
```bash
# 1. Worker 跑了一会有对话后，kill 掉
kill <worker_pid>

# 2. 重新启动
./target/release/see-agent-corp worker coder ~/.see-agent-corp

# 期望日志：INFO restoring session from disk agent="coder" kept=N
# 发一条新消息，Agent 应该还记得之前的对话
```

---

## 补充说明：当前消息系统的内存 vs 文件关系

```
                     ┌──────────────────────────────────┐
                     │   ConversationContext (内存)       │
                     │   messages: Vec<Value>            │
                     │   ← 运行时上下文，LLM 调用用这个    │
                     │   ← microcompact 只改这里          │
                     │   ← compact 改这里 + 写 JSONL 标记  │
                     └──────────────┬───────────────────┘
                                    │ on_append 回调
                                    ▼
                     ┌──────────────────────────────────┐
                     │   session/messages.jsonl (磁盘)    │
                     │   ← 只追加，永不修改已有行          │
                     │   ← 前端 2s 轮询读取展示           │
                     │   ← restore 时读取恢复上下文       │
                     └──────────────────────────────────┘
```

**要点：messages.jsonl 是日志，不是运行时数据源。运行时真正的上下文在内存的 `ConversationContext.messages` 中。**
