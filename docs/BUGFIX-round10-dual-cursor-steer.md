# Round 10: Steer 实时注入——双 Cursor 机制

## 报告人
草莓🍓 | 2026-03-19

---

## 背景

当前 Worker 的 `run_one_turn` 推理循环执行期间，新到的 steer 消息只能等整个 turn 结束后才被消费。这不符合 steer 的语义——steer 应该在**下一次 LLM 推理前**被注入。

## 设计：双 Cursor

### inbox_cursor.json 新格式

```json
{
  "collect": 10,
  "steer": 15
}
```

- `collect` cursor：外层主循环用，读取所有消息（collect + steer）
- `steer` cursor：推理循环内用，只读取 steer 消息

两个 cursor 独立推进，steer cursor >= collect cursor。

### 新增函数

```rust
/// 读取 steer cursor 之后的所有消息，只返回 steer 消息，只推进 steer cursor。
/// collect 消息跳过不处理，也不推进 collect cursor。
pub fn drain_steer_only(inbox_path: &Path, cursor_path: &Path) -> Result<Vec<Message>> {
    let all = read_jsonl(inbox_path)?;
    let cursors = read_cursors(cursor_path)?;  // {collect: N, steer: M}
    let steer_cursor = cursors.steer;
    
    let mut steer_msgs = Vec::new();
    let mut new_steer_cursor = steer_cursor;
    
    for (i, msg) in all.iter().enumerate().skip(steer_cursor) {
        if msg.priority == "steer" {
            steer_msgs.push(msg.clone());
        }
        new_steer_cursor = i + 1;
    }
    
    // 只推进 steer cursor
    write_cursors(cursor_path, cursors.collect, new_steer_cursor)?;
    Ok(steer_msgs)
}
```

### 修改 drain_inbox_split

```rust
/// 原有函数改造：读取 collect cursor 之后的所有消息，推进两个 cursor 到同一位置。
pub fn drain_inbox_split(inbox_path: &Path, cursor_path: &Path) -> Result<(Vec<Message>, Vec<Message>)> {
    let all = read_jsonl(inbox_path)?;
    let cursors = read_cursors(cursor_path)?;
    // 从 collect cursor 开始读（steer cursor 可能已经超前了）
    let start = cursors.collect;
    
    let mut steer = Vec::new();
    let mut collect = Vec::new();
    
    for msg in all.iter().skip(start) {
        if msg.priority == "steer" {
            // 如果 steer cursor 已经读过这条，跳过
            // （已经在推理循环中被注入过了）
            // 但仍然要推进 collect cursor
        } else {
            collect.push(msg.clone());
        }
    }
    
    // 两个 cursor 都推进到末尾
    let end = all.len();
    write_cursors(cursor_path, end, end)?;
    Ok((steer, collect))
}
```

**关键点：** `drain_inbox_split` 中，已经被 `drain_steer_only` 消费过的 steer 消息不要重复返回。判断方式：如果消息 index < steer_cursor，说明已被推理循环消费过。

### Worker 主循环改造

```
loop {
    drain_inbox_split()  → (steer[], collect[])
    // steer[] 中只包含 steer_cursor 没读过的 steer 消息
    // collect[] 包含所有未读的 collect 消息
    
    if steer + collect 都为空 → 等待 SIGUSR1
    
    注入所有消息到 context
    
    run_one_turn() 内部的推理循环:
        loop {
            drain_steer_only() → new_steer[]
            if 有新 steer → 注入 context
            
            LLM 推理
            if 无 tool_calls → break
            执行 tools
        }
}
```

### cursor 文件兼容

旧格式 `{"line": N}` → 自动迁移为 `{"collect": N, "steer": N}`。

### create_agent 初始化

```json
{"collect": 0, "steer": 0}
```

---

---

## Bug 54: Worker 启动时忽略历史 shutdown 消息

### 现象
每次 stop/kill 后 start，worker 读到上一世的 shutdown 消息立即退出。反复出现。

### 根因
`stop_all()` 往 inbox 写 shutdown 消息，worker 退出后 shutdown 永久留在 inbox。新 worker 启动 drain 时读到 → 自杀。

### 修复
Worker 启动后**第一次** drain 时，filter 掉所有 `metadata.shutdown == "true"` 的消息。后续 drain 正常处理 shutdown（因为那是给"这一世"的）。

```rust
// worker.rs 主循环，第一次 drain 后：
let (steer_msgs, collect_msgs) = drain_inbox_split(&inbox_path, &cursor_path)?;

if is_first_drain {
    // 过滤掉历史 shutdown（上一世的遗言）
    let steer_msgs: Vec<_> = steer_msgs.into_iter().filter(|m| !m.is_shutdown()).collect();
    let collect_msgs: Vec<_> = collect_msgs.into_iter().filter(|m| !m.is_shutdown()).collect();
    is_first_drain = false;
}

// 后续的 shutdown 检查正常执行（给这一世的）
for msg in steer_msgs.iter().chain(collect_msgs.iter()) {
    if msg.is_shutdown() { ... }
}
```

---

## Bug 55: /tools、/config、/logs 页面超出浏览器高度无滚动

### 现象
这些页面内容超出视口高度后，看不到下面的内容（滚动条消失）。Chat 页面修好了但其他页面没改。

### 修复
所有页面的主内容区域统一用 `overflow-y-auto` + `h-screen`（或 flex 撑满），确保有内部滚动。

---

## Bug 56: /logs 一直显示 No log entries

### 现象
`/logs` 页面永远空。`server.log` 确实有日志但 API 返回空。

### 修复
`get_logs_handler` 需要正确读取 `server.log` 和 `agents/*/worker.log`。检查之前 Round 3 的改动是否真的生效了。

---

## 前端优化

### Bug 52: /tools 全局页面按分组展示
当前 `/tools` 页面像 Wiki 列表。改为和 agent detail Tools tab 一样按 group 分组展示（core / memory / team / control），不需要开关。

### Bug 53: Agent Detail - Skills tab 加开关
类似 Tools tab 的开关。切换时写入 agent.json 的 `skills.disabled` 数组。后端已有 `toggle_agent_skill_handler`，前端接入即可。

---

## 实施约束

1. 先改 `inbox.rs` 的 cursor 读写 + `drain_steer_only` + 改造 `drain_inbox_split`
2. 再改 `worker.rs` 主循环 + `run_one_turn` 内部添加 `drain_steer_only` 调用
3. 再改 `create_agent` 初始化新格式 cursor
4. 然后前端：Bug 52 → Bug 53
5. 每步 cargo test
6. 旧格式 cursor 自动迁移，不报错
7. 最后 trunk build --release + git commit + git push
