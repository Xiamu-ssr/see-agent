# v4 Agent 进程修复 Report

> 给 CC 或草莓在新 session 中执行。改之前先读 MentalModel.md。

## 根本原因

Agent 进程在等 LLM 回复时收到 SIGUSR1（新消息通知），信号打断了 httpx 的网络请求，
导致 OpenAI SDK 抛异常。`loop.run()` 捕获异常后 abort return，进程退出主循环。

另外 `loop.run()` 是一次性任务模式（跑完就 return），不适合常驻进程。
每条消息都创建新 session、新 system prompt，这完全不对。

## 需要修的

### 1. SIGUSR1 信号安全

SIGUSR1 handler 里只能 set asyncio.Event，不能打断正在进行的 IO。
当前代码用 `signal.signal(SIGUSR1, handler)` 是同步信号处理，会打断 httpx。

**修法**：改用 `loop.add_signal_handler(SIGUSR1, handler)` — asyncio 安全的信号处理，
不会打断正在进行的 await。

```python
# 旧（不安全）
signal.signal(signal.SIGUSR1, _on_sigusr1)

# 新（asyncio 安全）
asyncio.get_running_loop().add_signal_handler(signal.SIGUSR1, wake_event.set)
```

### 2. 删除 loop.run()，统一用 run_one_turn()

`see_agent/agent/loop.py`:

- `run(task)` 是一次性模式：创建 session → 截图 → 注入 task → 跑 loop → return
- `run_one_turn(messages)` 是增量模式：注入消息到已有上下文 → 跑一步

问题：`run_one_turn` 在 `_active_ctx is None` 时会调 `run(task)` 回到一次性模式。

**修法**：
- `run_one_turn` 在 `_active_ctx is None` 时自己初始化 session（不调 run）
- 将 `run()` 里的初始化逻辑（创建 session、构建 system prompt、初始化 context）
  提取成 `_ensure_session()` 方法
- `run_one_turn` 调 `_ensure_session()` 确保 session 存在
- `run()` 可以保留但标记为 deprecated，内部也用 `_ensure_session()` + `_run_loop()`
- 单会话：session 目录固定在 `agents/{id}/session/`，只在首次创建

### 3. Agent 进程不退出

`see_agent/agent/worker.py`（或改名为 `agent_process.py`）:

- while True 循环永远不退出
- runtime.handle_message() 的异常全部 catch + log
- LLM 异常：记错误日志，回到 idle 等下一条消息
- 进程级异常：catch + log + 继续循环（除非是 KeyboardInterrupt/SystemExit）

### 4. 文件改名（可选）

- `worker.py` → `agent_process.py`
- 日志里 "Worker" → "Agent process"
- CLI 参数里 "worker" → "agent-process"

## 执行顺序

1. 先修信号处理（第 1 项）— 最关键，防止 SIGUSR1 打断 httpx
2. 再改 loop 架构（第 2 项）— 提取 _ensure_session，run_one_turn 自己初始化
3. 最后加防护（第 3 项）— while True 永不退出
4. 改名（第 4 项）— 最后做

每步跑 `bash scripts/check.sh`。
