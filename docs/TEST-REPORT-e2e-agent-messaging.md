# 端到端测试报告：Agent 消息流转

## 测试人
草莓🍓 | 2026-03-19 10:55~11:00

## 测试环境
- commit: `456e4908` (Bug 1/2/3 修复后)
- `see-agent-corp serve --port 28789`
- 全部 agent/team 删除后从零开始

---

## 测试结果总览

| # | 测试项 | 结果 | 说明 |
|---|--------|------|------|
| 1 | create_agent 初始化文件完整性 | ⚠️ 部分通过 | inbox + cursor ✅，messages.jsonl 不在创建时生成 |
| 2 | 发消息→inbox 写入 | ✅ 通过 | |
| 3 | Worker 自动启动 | ✅ 通过 | supervisor 正确 spawn |
| 4 | **Worker 被 SIGUSR1 杀死** | 🔴 **失败** | **竞态条件：Worker 还没注册 signal handler 就收到 SIGUSR1** |
| 5 | 手动启动 Worker 处理消息 | ✅ 通过 | Worker 正常 drain inbox → 调用 LLM → 回复写入 messages.jsonl |
| 6 | Session restore | ✅ 通过 | 日志 `restoring session (no compact summary) kept=2` |
| 7 | inbox_cursor 推进 | ✅ 通过 | 从 0 推进到 1 |
| 8 | send_message_handler 双写 | ✅ 已修复 | handler 不再直接写 messages.jsonl |
| 9 | Bug 1（历史 shutdown 杀 worker） | ✅ 已修复 | cursor 初始化 + 历史跳过 |

---

## 🔴 Bug 4（新发现）：Worker 启动竞态——SIGUSR1 在 handler 注册前到达

### 严重程度：🔴 Critical（阻塞所有 Agent 回复）

### 现象
通过 API 给 agent 发消息 → supervisor 自动启动 worker → **worker 立即被 SIGUSR1 杀死**（exit status 30 = signal 30 = SIGUSR1）

### 根因

`supervisor.send_to()` 的执行顺序：

```
1. start_agent(id)      → tokio::process::Command::spawn()
2. send_to_inbox_with_id()  → 写 inbox.jsonl
3. signal_process(pid)  → kill(pid, SIGUSR1)    ← 这里太早了！
```

Worker 进程启动后的初始化链路（需要时间）：

```
main() → run()
  → WorkspaceDir::new()
  → find_agent_team()
  → load_agent_config()          ← 配置合并
  → OpenAiBrain::new()           ← 建 HTTP client
  → create_eye()
  → ToolRegistry + register_builtin_tools()
  → ConversationContext::new()   ← or restore
  → SessionStore::new()
  → tokio::signal::unix::signal(SignalKind::user_defined1())  ← 第 185 行，这里才注册 handler！
  → drain loop 开始
```

从 spawn 到 handler 注册可能需要 **几十到几百毫秒**。supervisor 在 spawn 后**立即**发 SIGUSR1，Worker 大概率还没执行到 handler 注册，此时 SIGUSR1 的默认行为是 **终止进程**。

### 证据

```
server.log:
  INFO worker process started agent="charlie" pid=50436
  # 无后续日志——Worker 被信号杀死了

Worker 无任何输出：
  - 没有 "worker starting" 日志
  - 没有 worker_error.log
  - 没有 messages.jsonl
  - inbox_cursor 未推进（仍为 0）

手动启动 Worker：完美运行
  - "worker starting"
  - "no previous session, starting fresh"
  - "processing 1 inbox messages"
  - "finished tool called in mode B"
  - "turn complete, returning to idle"
  - messages.jsonl 正确写入
  - cursor 推进到 1
```

### 修复方案

**方案 A（推荐）：刚启动的 Worker 不需要 SIGUSR1**

```rust
// supervisor/manager.rs — send_to()
pub async fn send_to(&mut self, agent_id: &str, message: Message) -> Result<()> {
    // ...
    let just_started = if !self.is_running(agent_id) {
        self.start_agent(agent_id).await?;
        true
    } else {
        false
    };

    send_to_inbox_with_id(&inbox_path, message)?;

    // 刚启动的 Worker 会自己 drain inbox，不需要 signal
    if !just_started {
        if let Some(handle) = self.processes.get(agent_id) {
            signal_process(handle.pid);
        }
    }

    Ok(())
}
```

**方案 B：Worker 启动时立即注册 signal handler（在 main/tokio 入口处）**

把 SIGUSR1 handler 注册移到 `run()` 的最开头，在任何初始化之前。但这需要改 channel 的生命周期，稍复杂。

**方案 C：Worker 启动时先 block SIGUSR1，初始化完再 unblock**

用 `sigprocmask` 在进程启动时 block SIGUSR1，handler 注册后再 unblock。最安全但需要 unsafe 或 nix crate。

**推荐方案 A**——最简单、无风险，且语义上也对（刚启动的 Worker 不需要被唤醒）。

---

## ⚠️ 其他发现

### 问题 5：messages.jsonl 不在 create_agent 时创建

`create_agent()` 创建了 `session/` 目录和 `session/screenshots/`，但没创建 `session/messages.jsonl`。

这导致前端聊天框在 Agent 第一次收到消息前看到的是空（或 404），而不是空列表。

**影响**：轻微。Worker 启动时会创建。但为了一致性，建议 `create_agent()` 也初始化空的 `messages.jsonl`。

### 问题 6：Supervisor 不捕获 Worker 的 stdout/stderr

Worker 的日志输出到继承的 fd，但 `serve` 进程通常用 `nohup` 启动（stdout 到 nohup.out 或 /dev/null），Worker 的崩溃信息完全丢失。

**建议**：supervisor spawn 时把 Worker 的 stderr 重定向到 `agents/{id}/worker.log`：

```rust
let log_file = std::fs::File::create(agent_dir.path().join("worker.log"))?;
let child = tokio::process::Command::new(&self.binary_path)
    .arg("worker")
    .arg(agent_id)
    .arg(workspace_path)
    .stderr(log_file.try_clone()?)
    .stdout(log_file)
    .spawn()?;
```

### 问题 7：僵尸进程未被及时收割

Worker 崩溃后变成 `<defunct>`（僵尸），直到下次 `is_running()` 调用才被 `try_wait()` 清理。

Supervisor 应该主动注册子进程退出回调（`tokio::spawn` 一个 `child.wait()` task），在 Worker 退出时立即记录日志 + 清理 HashMap。

---

## 修复优先级

| # | 问题 | 优先级 | 原因 |
|---|------|--------|------|
| **4** | SIGUSR1 竞态杀 Worker | 🔴 P0 | 阻塞所有功能 |
| 5 | messages.jsonl 未初始化 | 🟡 P2 | 一致性 |
| 6 | Worker 日志丢失 | 🟡 P2 | 调试困难 |
| 7 | 僵尸进程 | 🟢 P3 | 不影响功能 |

---

## 验证方法（Bug 4 修复后）

```bash
# 1. 清空所有 agent
for id in $(curl -s http://localhost:28789/api/agents | python3 -c "import sys,json; [print(a['id']) for a in json.load(sys.stdin)]"); do
  curl -s -X DELETE http://localhost:28789/api/agents/$id
done

# 2. 重新编译 + 重启 serve
cargo build --release
# 重启 serve

# 3. 创建 agent + 发消息
curl -s -X POST http://localhost:28789/api/agents -H "Content-Type: application/json" -d '{"id":"test"}'
curl -s -X POST http://localhost:28789/api/agents/test/message -H "Content-Type: application/json" -d '{"content":"say hello","priority":"collect"}'

# 4. 等 10 秒
sleep 10

# 5. 验证
cat ~/.see-agent-corp/agents/test/session/messages.jsonl
# 期望：有 user_task + assistant 两条记录
cat ~/.see-agent-corp/agents/test/inbox_cursor.json
# 期望：{"line": 1}
ps aux | grep "worker test" | grep -v grep
# 期望：Worker 进程存活
```
