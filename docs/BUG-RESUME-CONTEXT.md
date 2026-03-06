# BUG REPORT: Session Resume 不恢复对话历史

> 日期：2026-03-06
> 严重度：🔴 Critical（核心功能不可用）
> 发现方式：手动测试 `see-agent resume`

---

## 复现步骤

```bash
# 1. 用 chat 模式执行一个任务
see-agent chat
> 搜索智谱股价走势
# agent 完成任务，输出会话 ID: 20260306_143919_df7744

# 2. 退出后 resume
see-agent resume 20260306_143919_df7744
> 不用操作，把刚才你得到的数据说一下，我忘记了

# 3. 预期：agent 知道之前搜了智谱股价，复述数据
# 4. 实际：agent 答非所问，完全不知道之前的对话
```

## 根因分析

`AgentLoop.run()` 第 215-221 行，**无论是否传了 `session_id`，都新建 `ConversationContext`**：

```python
# 当前代码（see_agent/agent/loop.py 第 215-221 行）
system_prompt = build_system_prompt(self._config)
ctx = ConversationContext(
    system_prompt,
    max_images=self._max_images,
    on_append=session.append_message,   # ← 写到同一个 JSONL
)
```

这导致两个问题：

### 问题 1：LLM 看不到历史对话

`ConversationContext.__init__` 新建空的 `_messages` 列表，只含 system prompt。resume 后 LLM 只看到 system prompt + 当前截图 + 新任务，**完全不知道之前做过什么**。

### 问题 2：JSONL 里出现重复 system 消息

`ConversationContext.__init__` 中 `on_append` 会把 system prompt 追加到 JSONL。resume 时又追加了一条，导致 `messages.jsonl` 里有两条 `type: system`：

```
第  1 行: {"type": "system", ...}       ← 首次创建
第  2 行: {"type": "user_task", ...}     ← 首次任务
...（70 行正常对话）...
第 72 行: {"type": "system", ...}       ← resume 时重复写入 ❌
第 73 行: {"type": "user_task", ...}     ← resume 的新任务
第 74 行: {"type": "assistant", ...}     ← agent 答非所问（没有历史）
```

### 缺失实现

REFACTOR-REPORT.md 中设计的 `session.restore_context()` 方法没有实现。原始设计：

```python
# REFACTOR-REPORT.md §2.7 设计
if session_id:
    session = SessionStore.load(session_id)
    ctx = session.restore_context()   # ← 从 JSONL + screenshots 重建
```

---

## 修复方案

### 改动 1：`Session` 新增 `restore_context()` 方法

**文件：** `see_agent/session/store.py`

```python
def restore_context(
    self,
    system_prompt: str,
    max_images: int = 5,
    on_append: Callable[[dict], None] | None = None,
) -> ConversationContext:
    """从 messages.jsonl + 截图文件重建 ConversationContext。
    
    逻辑：
    1. 读 messages.jsonl 所有行
    2. 对每行按 type 还原为 OpenAI 格式的 message：
       - system → {"role": "system", "content": ...}
       - user_task → {"role": "user", "content": [text + image_url]}
         其中 image_url 从 screenshots/<ref> 加载 base64
       - assistant → {"role": "assistant", ...} 还原 tool_calls
       - tool_result → {"role": "tool", ...} + 可选截图
       - screenshot → {"role": "user", "content": [image_url]}
       - user_reply → {"role": "user", "content": text}
       - system_hint → {"role": "user", "content": text}
    3. 构建 ConversationContext，将 on_append 设好但 **不回写已有消息**
    4. 返回填充好的 ctx
    """
```

**关键注意事项：**
- 加载截图时只加载最近 `max_images` 张的 base64，其余用 `[Screenshot omitted]` 占位（和现有滑动窗口逻辑一致）
- `on_append` 只在 restore 完成后才生效，避免把旧消息重新写一遍到 JSONL
- 截图文件不存在时优雅降级（log warning，用占位符）

### 改动 2：`AgentLoop.run()` resume 分支

**文件：** `see_agent/agent/loop.py`

```python
# ── 3. Build conversation context ─────────────────────────────
from see_agent.brain.prompts import build_system_prompt

system_prompt = build_system_prompt(self._config)

if session_id:
    # Resume：从 JSONL + 截图恢复历史对话
    ctx = session.restore_context(
        system_prompt,
        max_images=self._max_images,
        on_append=session.append_message,
    )
    # 只追加新的 user task（不重写 system prompt）
    task_text = f"{env_block}\n\n{task}" if env_block else task
    ctx.add_user_task(
        task_text, scaled.base64, scaled.detail,
        mime_type=scaled.mime_type,
        screenshot_ref=f"step_{next_step:03d}.webp",
    )
else:
    # 新建会话
    ctx = ConversationContext(
        system_prompt,
        max_images=self._max_images,
        on_append=session.append_message,
    )
    task_text = f"{env_block}\n\n{task}" if env_block else task
    ctx.add_user_task(
        task_text, scaled.base64, scaled.detail,
        mime_type=scaled.mime_type,
        screenshot_ref="step_000.webp",
    )
```

### 改动 3：截图步数续接

resume 时需要知道上次截到了哪一步，避免覆盖旧截图：

```python
if session_id:
    # 找到已有截图的最大步数
    existing = list(session.screenshots_dir.glob("step_*.webp"))
    if existing:
        max_existing = max(
            int(f.stem.split("_")[1]) for f in existing
        )
        next_step = max_existing + 1
    else:
        next_step = 0
    # 初始截图用 next_step 编号
    initial_path = task_dir / f"step_{next_step:03d}.webp"
```

### 改动 4：步数续接

`for step in range(1, self._max_steps + 1)` 在 resume 时应该从已完成的步数继续，否则 step 编号和截图编号对不上。或者更简单的做法：step 编号独立于截图编号，只用于预算控制。

---

## 需要同时修的测试

在 `docs/TEST-GAPS-REPORT.md` 中已有对应测试用例，标注为 P0：

- `test_resume_restores_conversation_history` — resume 后 LLM 收到的 messages 包含历史
- `test_resume_no_duplicate_system_message` — JSONL 不出现重复 system 消息
- `test_resume_screenshot_numbering_continues` — 截图编号接着上次的来
- `test_restore_context_with_missing_screenshots` — 截图文件缺失时优雅降级
- `test_restore_context_respects_max_images` — 只加载最近 N 张截图的 base64

---

## 验证方法

修完后手动测试：

```bash
# 1. 新建会话，执行一个有数据输出的任务
see-agent chat
> 搜索今天的天气

# 2. 退出后 resume，问历史问题
see-agent resume --last
> 刚才搜到的温度是多少？

# 3. 检查 JSONL
# - 只有一条 system 消息
# - resume 后的消息接在旧消息后面
# - 截图编号不重叠

# 4. 检查 LLM 的回复是否引用了历史数据
```
