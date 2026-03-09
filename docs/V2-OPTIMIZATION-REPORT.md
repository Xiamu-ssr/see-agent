# see-agent v2 优化报告：Bug 修复 + 三大功能增强

> 生成日期：2026-03-09
> 作者：蓝莓（测试/Review Agent）
> 目标：修复已知 Bug + 实现 Skill 生态接入、异步用户消息、上下文压缩

---

## Part A: Bug 修复（优先完成）

### Bug 1 🔴 P0: session.log 始终为空（日志分层失效）

**现象**：`sessions/<id>/session.log` 文件始终 0 字节。

**根因**：`config.py` `setup_logging()` 第 202-206 行把 session 相关 logger level 设为 WARNING：

```python
for _name in ("see_agent.agent", "see_agent.brain", "see_agent.eye", "see_agent.hand"):
    logging.getLogger(_name).setLevel(logging.WARNING)
```

`Session.setup_logging()` 随后给这些 logger 加了 DEBUG FileHandler，但 logger 自身 level 仍是 WARNING，Python logging 先过 logger level 再到 handler，所以 DEBUG/INFO 消息在 logger 层就被拦截。

**修复**：`session/store.py` `setup_logging()` 中保存并降低 logger level，`teardown_logging()` 中恢复：

```python
def setup_logging(self) -> None:
    handler = logging.FileHandler(self.dir / "session.log", encoding="utf-8")
    handler.setLevel(logging.DEBUG)
    handler.setFormatter(logging.Formatter(
        "%(asctime)s  %(levelname)-8s  %(name)s  %(message)s", datefmt="%H:%M:%S",
    ))
    self._log_handler = handler
    self._original_levels: dict[str, int] = {}
    for name in self._SESSION_LOGGERS:
        lgr = logging.getLogger(name)
        self._original_levels[name] = lgr.level
        lgr.setLevel(logging.DEBUG)
        lgr.addHandler(handler)

def teardown_logging(self) -> None:
    handler = self._log_handler
    if handler is None:
        return
    for name in self._SESSION_LOGGERS:
        lgr = logging.getLogger(name)
        lgr.removeHandler(handler)
        if name in getattr(self, '_original_levels', {}):
            lgr.setLevel(self._original_levels[name])
    handler.close()
    self._log_handler = None
```

**验证**：跑 `see-agent chat` → 检查 `session.log` 包含 "Step"/"Tool call"/"Thought" → 全局 `logs/` 仍只有生命周期日志。

---

### Bug 2 ⚠️ P1: Skill 幻觉（MCP 失败但 skill 描述仍注入 prompt）

**现象**：MCP tavily 连接失败，工具未注册到 registry，但 `<SKILLS>` 段仍列出 `tavily-search` 描述，agent 以为能用。

**修复**：`build_system_prompt()` 时，对每个 skill 检查其依赖是否满足（详见 Part B Feature 1 的 gating 机制）。不满足的 skill 要么不注入 `<SKILLS>`，要么加标注 `⚠️ (unavailable: missing dependency)`。

---

### Bug 3 ⚠️ P2: Memory/MCP 启动状态不可见

**现象**：mem0 或 mcp 初始化失败时只在日志里打 Warning，CLI 不提示。

**修复**：`_build_components()` 完成后，打印功能状态摘要：

```
🤖 see-agent v0.1 已启动
📋 会话 ID: 20260309_xxx
✅ Memory: active (mem0, qdrant)
✅ MCP: tavily (3 tools registered)
❌ Skill: goplaces (blocked: missing bin:goplaces, env:GOOGLE_PLACES_API_KEY)
```

---

### Bug 4 ⚠️ P1: _save_memory 只在 finished 路径触发

**现象**：只有 agent 调 `finished` tool 正常完成时才存 mem0。max_steps 超限、连续错误、Ctrl+C 中断等路径全不存。

**修复**：
1. `_fail_result()` 里加 `self._save_memory(ctx, session.id)`
2. `_run_loop` 的 `finally` 块里加兜底 `_save_memory`（处理 Ctrl+C / 异常崩溃）
3. max_steps 循环结束后的 return 前加 `_save_memory`

---

## Part B: 功能增强

### Feature 1: Skill 生态接入（ClawhHub 兼容）

#### 1.1 统一 Skill 目录

see-agent 不区分 builtin 和外部 skill，统一以 `skills_dirs` 配置的目录为准。默认：

```json
"skills_dirs": ["~/.see-agent/skills", "~/.openclaw/skills"]
```

用户可以通过 ClawhHub CLI 安装 skill 到 see-agent 自己的目录：

```bash
clawhub install tavily-search --workdir ~/.see-agent
# → 安装到 ~/.see-agent/skills/openclaw-tavily-search/SKILL.md
```

或设置环境变量 `CLAWHUB_WORKDIR=~/.see-agent` 后直接 `clawhub install <slug>`。

#### 1.2 预置 ClawhHub Skill

在 `~/.see-agent/skills/` 下预置一个 `clawhub/SKILL.md`（`see-agent setup` 时自动创建）：

```markdown
---
name: clawhub
description: Install and manage AI agent skills from the ClawhHub registry. Use when the user wants to add new capabilities, search for skills, or update installed skills.
---
# ClawhHub - Skill Registry

## Install a skill
clawhub install <skill-slug> --workdir ~/.see-agent

## Search skills
clawhub search "calendar"

## Update all installed skills
clawhub update --all --workdir ~/.see-agent

## List installed skills
ls ~/.see-agent/skills/

## Notes
- After installing a skill, restart see-agent to load it.
- Skills are SKILL.md files with frontmatter (name, description, metadata).
- Browse available skills at https://clawhub.com
```

这让 see-agent 知道怎么帮用户安装新 skill。

#### 1.3 解析 SKILL.md frontmatter（兼容 OpenClaw/AgentSkills 规范）

改造 `skill/loader.py`，解析 `metadata` JSON 字段：

```yaml
---
name: goplaces
description: Query Google Places API...
metadata: {"openclaw": {"requires": {"bins": ["goplaces"], "env": ["GOOGLE_PLACES_API_KEY"]}, "primaryEnv": "GOOGLE_PLACES_API_KEY"}}
---
```

**新增 `SkillInfo` 字段：**

```python
@dataclass
class SkillInfo:
    name: str
    description: str
    body: str
    path: Path
    requires_bins: list[str] = field(default_factory=list)    # 新增
    requires_env: list[str] = field(default_factory=list)     # 新增
    requires_any_bins: list[str] = field(default_factory=list) # 新增
    blocked: bool = False                                      # 新增
    block_reason: str = ""                                     # 新增
```

**`_parse_skill()` 中解析 metadata：**

```python
metadata_str = meta.get("metadata", "")
if metadata_str:
    try:
        metadata = json.loads(metadata_str)
        oc = metadata.get("openclaw", metadata.get("clawdbot", {}))
        requires = oc.get("requires", {})
        skill.requires_bins = requires.get("bins", [])
        skill.requires_env = requires.get("env", [])
        skill.requires_any_bins = requires.get("anyBins", [])
    except json.JSONDecodeError:
        pass
```

**`load_skills()` 末尾加 gating 检查：**

```python
import shutil, os

for skill in skills:
    reasons = []
    for b in skill.requires_bins:
        if not shutil.which(b):
            reasons.append(f"bin:{b}")
    for e in skill.requires_env:
        if not os.environ.get(e) and not config_env.get(e):
            reasons.append(f"env:{e}")
    if skill.requires_any_bins and not any(shutil.which(b) for b in skill.requires_any_bins):
        reasons.append(f"anyBin:{'/'.join(skill.requires_any_bins)}")
    if reasons:
        skill.blocked = True
        skill.block_reason = ", ".join(reasons)
```

**`build_system_prompt()` 中过滤 blocked skill：**

```python
# prompts.py
skill_lines = []
for s in skills:
    if s.blocked:
        continue  # 不注入 blocked 的 skill
    skill_lines.append(f"- **{s.name}**: {s.description}")
```

**涉及文件**：`skill/loader.py`、`brain/prompts.py`、`cli/main.py`（启动状态打印）

---

### Feature 2: 异步用户消息（Queue 注入 ReAct）

#### 2.1 核心机制

在 AgentLoop 运行期间，用户可以随时发消息，消息进入队列，在下一次调基模前自动注入到 context。**不通过 tool，不打断当前操作。**

#### 2.2 架构

```
用户终端 stdin ──→ asyncio.Queue ──→ AgentLoop._run_loop()
                                      │
                                      ├─ 每次调 brain.chat() 前
                                      │  drain queue → ctx.add_user_reply()
                                      │
CLI 显示 ←──────── queue 状态（pending count）
```

#### 2.3 AgentLoop 改造

**构造函数新增参数：**

```python
class AgentLoop:
    def __init__(self, ..., user_queue: asyncio.Queue | None = None):
        self._user_queue = user_queue
```

**`_run_loop` 主循环中，调 `brain.chat()` 前 drain queue：**

```python
async def _run_loop(self, session, ctx, scaled, step_offset, t0):
    for step in range(self._max_steps):
        # ★ drain 用户消息队列
        injected = 0
        while self._user_queue and not self._user_queue.empty():
            try:
                user_msg = self._user_queue.get_nowait()
            except asyncio.QueueEmpty:
                break
            # 加上下文标注，让基模知道这是用户插入的消息
            tagged_msg = f"[用户插入消息] {user_msg}"
            ctx.add_user_reply(tagged_msg)
            logger.info("Injected queued user message: %s", user_msg[:100])
            injected += 1
        if injected:
            logger.info("Injected %d queued message(s) before step %d", injected, step)
        
        # 正常调基模
        response = await self._brain.chat(ctx.get_messages(), ...)
        ...
```

#### 2.4 消息标注与分类

**主动输入 vs 被动回复区分：**

| 场景 | 标注 | 来源 |
|------|------|------|
| 用户主动输入（queue） | `[用户插入消息] xxx` | `_user_queue` |
| `call_user` 的回复 | `User replied: xxx` | `on_user_input` callback |

两者走不同路径，不会混淆。`call_user` 仍是同步等待回复，queue 是异步插入。

**System prompt Rules 新增：**

```
13. 每步开始前可能有 [用户插入消息]，这是用户在你操作过程中发来的新指令或反馈。
    优先响应这些消息——如果用户要求改变方向，立即调整；如果是补充信息，纳入当前任务。
```

#### 2.5 CLI 改造

**后台读取 stdin：**

```python
async def _stdin_reader(queue: asyncio.Queue, prompt_event: asyncio.Event):
    """后台协程：持续读 stdin，写入 queue。call_user 时暂停。"""
    loop = asyncio.get_event_loop()
    while True:
        # call_user 期间暂停（让 call_user 独占 stdin）
        await prompt_event.wait()
        line = await loop.run_in_executor(None, sys.stdin.readline)
        if line.strip():
            await queue.put(line.strip())
```

**CLI 显示 queue 状态：**

```
> 打开浏览器看新闻

⏳ [Step 1] 正在执行...
> 换成打开钉钉          ← 用户随时输入
📨 Queued (1 pending)

✅ [Step 1] finished: 打开了 Safari
⏳ [Step 2] 正在执行...（已注入 1 条用户消息）

✅ [Step 2] finished: 收到用户新指令，切换到打开钉钉
📨 Queue: 0
```

每个 step 完成后显示 `📨 Queue: N`，有新消息入 queue 时即时显示 `📨 Queued (N pending)`。

#### 2.6 API 层

HTTP `POST /api/chat/{session_id}/message` 也写入同一个 queue，实现 API 和 CLI 统一。

#### 2.7 已知限制和注意事项

- **上下文膨胀**：如果用户疯狂发消息，会快速撑大 context → 配合 Feature 3（Compact）缓解
- **step 中间不响应**：消息在下一次 `brain.chat()` 前才注入，如果某个 tool 执行很久（比如 `wait(10)`），用户消息要等到下一步才被看到
- **`call_user` stdin 冲突**：用 `prompt_event` 信号量控制，`call_user` 开始时 clear event（暂停 queue 读取），结束后 set event（恢复）

**涉及文件**：`agent/loop.py`、`cli/main.py`、`brain/prompts.py`（Rules 新增）、`app.py`（API 层）

---

### Feature 3: 上下文压缩（Compact）

#### 3.1 问题

当前 context 管理只有截图滑动窗口（`max_images`），文本消息无限增长。长任务（70 步）会爆 context window。

#### 3.2 四层架构

```
Layer 1: 内存 messages（全量，当前实现）
Layer 2: 截图滑动窗口（保留最近 max_images 张，当前实现）
Layer 3: Context Compact（文本压缩，本次新增）
Layer 4: Mem0（跨会话长期记忆，当前实现）
```

#### 3.3 触发时机

每次调基模前，估算 context token 数：

```python
def _estimate_tokens(self, messages: list[dict]) -> int:
    """粗估 token 数：字符数 / 4"""
    return sum(len(str(msg)) for msg in messages) // 4
```

当 `estimated_tokens > context_window * compact_threshold`（默认 0.75）时触发 compact。

**新增 config 字段：**

```json
{
    "compact": {
        "enabled": true,
        "threshold": 0.75,
        "keep_recent": 8,
        "model": null
    }
}
```

- `threshold`：context 使用率超过此值时触发（0.75 = 75%）
- `keep_recent`：compact 时保留最近 N 条消息不压缩
- `model`：用于 compact 的 LLM 模型（null = 使用主模型，也可设为 `gpt-4.1-nano` 等便宜模型）

#### 3.4 Compact 流程

```python
async def _maybe_compact(self, ctx: ConversationContext, session: Session) -> None:
    """检查 context 大小，必要时触发 compact。"""
    messages = ctx.get_messages()
    estimated = self._estimate_tokens(messages)
    context_window = self._config.get("context_window", 200000)
    threshold = self._config.get("compact", {}).get("threshold", 0.75)
    
    if estimated < context_window * threshold:
        return
    
    keep_recent = self._config.get("compact", {}).get("keep_recent", 8)
    
    # 分离：要压缩的旧消息 + 要保留的新消息
    # messages[0] 是 system prompt，跳过
    # messages[1:] 中，最后 keep_recent 条保留，其余压缩
    old_messages = messages[1:-keep_recent] if len(messages) > keep_recent + 1 else []
    
    if not old_messages:
        return
    
    # 单独一次 LLM 调用做摘要（不是当前 agent turn）
    compact_model = self._config.get("compact", {}).get("model") or self._config["llm"]["model"]
    summary = await self._brain.summarize(old_messages, model=compact_model)
    
    # 确定 first_kept_msg_id
    recent_messages = messages[-keep_recent:]
    first_kept_id = ...  # 从 recent_messages 中取最早的 msg_id
    
    # 1. 追加 compact entry 到 JSONL（不重写旧数据）
    session.append_message({
        "type": "compact",
        "summary": summary,
        "first_kept_msg_id": first_kept_id,
        "tokens_before": estimated,
    })
    
    # 2. 重建内存中的 context（替换，不重写文件）
    ctx.apply_compaction(summary, keep_recent)
    
    logger.info("Compacted: %d tokens → %d tokens, kept %d recent messages",
                estimated, self._estimate_tokens(ctx.get_messages()), keep_recent)
```

#### 3.5 Brain 新增 summarize 方法

```python
# brain/openai_client.py
async def summarize(self, messages: list[dict], model: str | None = None) -> str:
    """单独一次 LLM 调用，压缩旧消息为摘要。"""
    formatted = self._format_messages_for_summary(messages)
    response = await self._client.chat.completions.create(
        model=model or self._model,
        messages=[
            {"role": "system", "content": (
                "你是一个对话摘要助手。请将以下对话历史压缩为简洁的摘要。\n"
                "保留：已完成的步骤、关键操作结果、发现的 UI 规律、未完成的目标。\n"
                "丢弃：具体的坐标数值、重复的截图描述、中间的试错过程。\n"
                "用中文输出。"
            )},
            {"role": "user", "content": formatted},
        ],
        max_tokens=2000,
    )
    return response.choices[0].message.content
```

#### 3.6 ConversationContext 新增 apply_compaction

```python
# agent/context.py
def apply_compaction(self, summary: str, keep_recent: int) -> None:
    """用 compact summary 替换旧消息，只保留最近 keep_recent 条。"""
    system_msg = self._messages[0]  # system prompt
    recent = self._messages[-keep_recent:] if len(self._messages) > keep_recent else self._messages[1:]
    
    self._messages = [
        system_msg,
        {"role": "system", "content": f"[Conversation Summary]\n{summary}"},
        *recent,
    ]
```

#### 3.7 JSONL 存储（append-only，不重写）

Compact 只在 `messages.jsonl` 末尾追加一条 `type: "compact"` entry：

```jsonl
{"ts":"...", "type":"user_task", "text":"打开钉钉", "msg_id":1}
{"ts":"...", "type":"assistant", "content":"好的", "msg_id":2}
...
{"ts":"...", "type":"assistant", "content":"会议室选好了", "msg_id":50}
{"ts":"...", "type":"compact", "summary":"用户要求打开钉钉预定会议室...", "first_kept_msg_id":45, "tokens_before":150000}
{"ts":"...", "type":"user_task", "text":"继续选时间", "msg_id":52}
```

旧消息（msg_id 1-44）**不动、不删、不改**，留在文件中做审计。

#### 3.8 Resume 时读取 Compact

`session/store.py` 的 `restore_context()` 改造：

```python
def restore_context(self, system_prompt, max_images, on_append):
    entries = list(self._read_jsonl())
    
    # 找最后一个 compact entry
    compact_summary = None
    first_kept = 0
    for entry in entries:
        if entry.get("type") == "compact":
            compact_summary = entry["summary"]
            first_kept = entry["first_kept_msg_id"]
    
    ctx = ConversationContext(system_prompt, max_images=max_images, on_append=on_append)
    
    if compact_summary:
        ctx.inject_summary(compact_summary)
    
    # 只加载 first_kept 之后的消息
    for entry in entries:
        if entry.get("type") == "compact":
            continue
        msg_id = entry.get("msg_id", 0)
        if first_kept and msg_id < first_kept:
            continue
        # 正常加载到 context...
        self._restore_entry(ctx, entry)
    
    return ctx
```

#### 3.9 messages.jsonl 需要加 msg_id

当前 JSONL entry 没有 msg_id 字段。需要在 `Session.append_message()` 中自增分配：

```python
def append_message(self, entry: dict) -> None:
    self._msg_counter += 1
    entry["msg_id"] = self._msg_counter
    entry["ts"] = _now_iso()
    with open(self.dir / "messages.jsonl", "a") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")
```

`_msg_counter` 在 session 创建时初始化为 0，resume 时从 JSONL 最后一条的 msg_id 恢复。

#### 3.10 System Prompt 更新

`brain/prompts.py` Rules 新增：

```
14. 对话开头可能有 [Conversation Summary]，这是之前对话的压缩摘要。
    基于其中的信息继续任务，不要重复已完成的操作。摘要中提到的 UI 规律和发现可以直接复用。
```

#### 3.11 多次 Compact

支持多次 compact。每次 compact 都覆盖之前的——读取时只看**最后一个** `type: "compact"` entry：

```
messages.jsonl:
  msg 1-44   → 被第一次 compact 压缩
  compact_1  → summary_1, first_kept=45
  msg 45-90  → 被第二次 compact 压缩
  compact_2  → summary_2, first_kept=85  ← 只看这个
  msg 85-90  → 保留（compact_2 的尾巴）
  msg 91-95  → 新消息
```

第二次 compact 的 summary 会包含第一次 compact 的 summary 内容（因为它是对当时完整 context 做的摘要，而当时 context 里有 summary_1）。

---

## Part C: 执行优先级

| 项目 | 优先级 | 依赖 | 预估改动量 |
|------|--------|------|-----------|
| Bug 1: session.log 为空 | 🔴 P0 | 无 | ~20 行 |
| Bug 4: _save_memory 路径不全 | 🔴 P0 | 无 | ~10 行 |
| Feature 3: Compact（msg_id 部分） | 🔴 P0 | 无 | ~30 行（先加 msg_id，后续 compact 依赖它） |
| Bug 2: Skill 幻觉 | ⚠️ P1 | Feature 1 | Feature 1 完成后自动解决 |
| Feature 1: Skill 生态 | ⚠️ P1 | 无 | ~100 行 |
| Bug 3: 启动状态提示 | ⚠️ P1 | Feature 1 | ~40 行 |
| Feature 3: Compact（完整） | ⚠️ P1 | msg_id | ~200 行 |
| Feature 2: 异步用户消息 | ⚠️ P2 | 无 | ~150 行 |

**建议执行顺序**：Bug 1 + Bug 4 + msg_id → Feature 1 + Bug 2 + Bug 3 → Feature 3 Compact → Feature 2 Queue

**涉及的主要文件**：

| 文件 | 改动内容 |
|------|---------|
| `session/store.py` | Bug 1 修复 + msg_id + compact JSONL + restore_context |
| `skill/loader.py` | Feature 1: metadata 解析 + gating |
| `agent/loop.py` | Bug 4 + Feature 2 queue drain + Feature 3 _maybe_compact |
| `agent/context.py` | Feature 3: apply_compaction + inject_summary |
| `brain/openai_client.py` | Feature 3: summarize() |
| `brain/prompts.py` | Feature 1: 过滤 blocked skill + Feature 2/3: Rules 新增 |
| `cli/main.py` | Bug 3 启动状态 + Feature 1 clawhub skill + Feature 2 stdin reader + queue 显示 |
| `config.py` | Feature 3: compact 配置字段 + DEFAULT_CONFIG |

做完跑 `scripts/check.sh` 确保全过。需要为新功能补充对应的测试用例。
