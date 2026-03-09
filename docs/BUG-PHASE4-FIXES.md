# Bug Report: Phase 1-4 遗留问题修复

> 生成日期：2026-03-09
> 基于 commit faac448 的 Review

---

## Bug 1 🔴 P0: 纯文本回复被误报 "Max steps (70) reached"

### 现象
用户输入"你好"，基模正确回复纯文本（无 tool call），但 CLI 显示：
```
❌ [Step 0] aborted: Max steps (70) reached. Task may be incomplete.
```

### 根因
`_run_loop` 主循环中，基模返回纯文本时 `break` 退出循环（第 462 行），但 `break` 后 fall through 到循环末尾的 `_fail_result("Max steps reached")`（第 666-669 行）。

```python
for step in range(1, self._max_steps + 1):
    ...
    if not response.tool_calls:
        logger.info("No tool calls returned -- ending loop")
        ctx.add_assistant(response.raw)
        break                          # ← break 退出
    ...

# ← break 后直接到这里
logger.warning("Max steps (%d) reached", self._max_steps)
return self._fail_result(...)          # ← 误判
```

### 修复
用 Python `for...else` 语法区分 break 退出和循环耗尽：

```python
for step in range(1, self._max_steps + 1):
    ...
    if not response.tool_calls:
        logger.info("No tool calls returned -- ending loop")
        ctx.add_assistant(response.raw)
        break
    ...
else:
    # 只有循环正常耗尽（没有 break）才是 "Max steps"
    logger.warning("Max steps (%d) reached", self._max_steps)
    return self._fail_result(
        session, final_step, t0,
        f"Max steps ({self._max_steps}) reached. Task may be incomplete.",
        ctx=ctx,
    )

# break 退出 = 模型主动结束（纯文本回复或无操作）
thought = response.content or ""
summary = thought[:200] if thought else "Task completed (no action needed)."
self._save_memory(ctx, session.id)
elapsed = time.monotonic() - t0
session.update_meta(
    status="completed", total_steps=final_step,
    elapsed_seconds=round(elapsed, 1), summary=summary,
)
return RunResult(
    summary=summary,
    task_dir=str(session.dir),
    total_steps=final_step,
    elapsed_seconds=elapsed,
    session_id=session.id,
)
```

### 验证
1. `see-agent chat` → 输入"你好" → 应显示 `✅` 正常完成，不是 `❌ Max steps`
2. 真正跑满 70 步时仍然报 "Max steps reached"

---

## Bug 2 🔴 P0: `see-agent setup install` 装到 .venv 而非当前 Python 环境

### 现象
在 conda base 环境下运行 `see-agent setup install`，输出显示安装成功，但 `see-agent chat` 启动时报 `Memory enabled but mem0ai not installed`。

### 根因
`setup_install()` 优先使用 `uv pip install`：

```python
installer = "uv" if shutil.which("uv") else "pip"
if installer == "uv":
    cmd = ["uv", "pip", "install", "-e", f".[{spec}]"]
```

`uv pip install` 默认操作项目目录下的 `.venv`，不管当前终端激活的是什么环境。如果用户在 conda/system Python 下跑 see-agent，依赖装到 .venv 里但 see-agent 进程用的是 conda/system Python，找不到。

### 修复
始终使用 `sys.executable` 对应的 pip，确保装到当前运行 see-agent 的同一个 Python 环境：

```python
@setup_app.command("install")
def setup_install(...) -> None:
    extras: list[str] = [...]
    spec = ",".join(extras)

    # 始终用当前 Python 的 pip，保证装到运行 see-agent 的同一环境
    cmd = [sys.executable, "-m", "pip", "install", "-e", f".[{spec}]"]

    typer.echo(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, check=False)
    raise typer.Exit(code=result.returncode)
```

### 验证
1. 在 conda base 下运行 `see-agent setup install` → `pip list | grep mem0` 能找到
2. 在 .venv 下运行 `see-agent setup install` → `.venv/bin/pip list | grep mem0` 能找到
3. `see-agent chat` 启动后显示 `Memory: active`

---

## Bug 3 ⚠️ P1: `_save_memory` 没有 finally 兜底

### 现象
Ctrl+C 中断或异常崩溃时，memory 不会保存。当前只有 `_fail_result` 和 `finished` 路径调了 `_save_memory`。

### 修复
在 `run()` 方法的 `finally` 块里加兜底：

```python
# agent/loop.py run() 方法
try:
    return await self._run_loop(session, ctx, scaled, step_offset, t0)
finally:
    # 兜底：如果上面的路径都没存（比如 Ctrl+C），这里补一次
    # _save_memory 内部有 try/except，不会抛异常
    if ctx is not None:
        self._save_memory(ctx, session.id)
    session.teardown_logging()
```

注意：如果 `_run_loop` 内部已经调过 `_save_memory`（finished/fail 路径），重复调一次也没问题——mem0 的 `add()` 是幂等的（相同内容再加一次不会出错，最多多一条重复记忆）。

---

## Bug 4 ⚠️ P1: CLI `chat` 命令没有接入 Queue

### 现象
`AgentLoop` 已支持 `user_queue` 参数，API 层也有 queue 注入，但 CLI 的 `chat()` 命令没有创建 queue，也没有后台 stdin reader。用户在 agent 运行期间输入的内容会被丢弃。

### 修复
`cli/main.py` 的 `chat()` 命令改造：

```python
import asyncio
import threading

def chat(...) -> None:
    ...
    user_queue: asyncio.Queue[str] = asyncio.Queue()
    loop = _build_components(config, ..., user_queue=user_queue)
    ...

    try:
        while True:
            _flush_stdin()
            task = _safe_input("> ")
            if not task:
                continue

            # 在后台线程中持续读 stdin，写入 queue
            stop_reader = threading.Event()
            reader_thread = threading.Thread(
                target=_stdin_reader_thread,
                args=(user_queue, stop_reader),
                daemon=True,
            )
            reader_thread.start()

            try:
                result = asyncio.run(loop.run(task, session_id=session.id))
            finally:
                stop_reader.set()
                reader_thread.join(timeout=0.5)

            _print_task_result(result)
    ...

def _stdin_reader_thread(queue: "asyncio.Queue[str]", stop: threading.Event) -> None:
    """后台线程：运行期间读 stdin，写入 queue。"""
    import select
    while not stop.is_set():
        # 非阻塞检查 stdin 是否有数据
        if select.select([sys.stdin], [], [], 0.3)[0]:
            line = sys.stdin.readline().strip()
            if line:
                queue.put_nowait(line)
                typer.echo(f"📨 Queued ({queue.qsize()} pending)")
```

**同时在 step 完成输出中显示 queue 状态：**
在 `_run_loop` 的 step 日志中（或 `on_step` 回调里），每步结束后如果 queue 非空，打印 `📨 Queue: N`。

---

## Bug 5 ⚠️ P1: Compact 的 `first_kept_msg_id` 计算不准

### 现象
`_maybe_compact()` 用 `session._msg_counter - keep_recent` 计算 `first_kept_msg_id`，但 `_msg_counter` 包括所有写入 JSONL 的 entry（system、compact、screenshot 等），不只是 context 中的消息。导致 `first_kept_msg_id` 指向的位置可能不对。

### 修复
从实际保留的 messages 中取 msg_id：

```python
# _maybe_compact() 中
# 替换:
#   first_kept_msg_id = max(session._msg_counter - keep_recent, 0)
# 改为:

# 读 JSONL 找最近 keep_recent 条非 compact/system 消息的最小 msg_id
recent_entries = session.read_messages()
# 过滤出 context 消息（不含 compact/system）
context_entries = [
    m for m in recent_entries
    if m.get("type") not in ("compact", "system", "screenshot")
]
if len(context_entries) > keep_recent:
    first_kept_msg_id = context_entries[-keep_recent].get("msg_id", 0)
else:
    first_kept_msg_id = context_entries[0].get("msg_id", 0) if context_entries else 0
```

---

## Bug 6 ⚠️ P2: Compact 的 `keep_recent` 硬编码为 4

### 现象
`_maybe_compact()` 第 234 行 `keep_recent = 4` 硬编码，忽略了 config 中的配置。

### 修复

```python
# 替换:
#   keep_recent = 4
# 改为:
keep_recent = compact_cfg.get("keep_recent", 8)
```

同时 `DEFAULT_CONFIG` 里补上：

```python
"compact": {
    "enabled": False,
    "context_window": 128000,
    "target_ratio": 0.75,
    "keep_recent": 8,       # ← 新增
    "summary_model": "",
},
```

---

## Bug 7 ⚠️ P2: Compact summary 的 role 应为 system

### 现象
`apply_compaction()` 和 `inject_summary()` 中，summary 用 `role: "user"`：

```python
summary_msg = {"role": "user", "content": f"[Conversation Summary]\n{summary}"}
```

基模会以为这是用户说的话，可能尝试回复这个 "用户消息"。

### 修复

```python
# context.py apply_compaction() 和 inject_summary() 中
summary_msg = {"role": "system", "content": f"[Conversation Summary]\n{summary}"}
```

注意：如果 LLM API 不支持多个 system message（部分 API 只允许第一条是 system），则改为 `role: "assistant"`（让基模以为是自己之前说的）或用 `role: "user"` 但加明确标注 `[系统注入的对话摘要，非用户消息]`。Claude API 支持多个 system message，所以直接用 `system` 没问题。

---

## 执行优先级

| Bug | 优先级 | 预估改动 |
|-----|--------|---------|
| Bug 1: 纯文本回复误报 Max steps | 🔴 P0 | ~20 行（for...else） |
| Bug 2: setup install 环境错误 | 🔴 P0 | ~5 行 |
| Bug 3: _save_memory finally 兜底 | ⚠️ P1 | ~5 行 |
| Bug 4: CLI queue 缺失 | ⚠️ P1 | ~40 行 |
| Bug 5: first_kept_msg_id 计算 | ⚠️ P1 | ~10 行 |
| Bug 6: keep_recent 硬编码 | ⚠️ P2 | ~3 行 |
| Bug 7: summary role | ⚠️ P2 | ~2 行 |

做完跑 `scripts/check.sh` 确保全过。
