# see-agent 代码优化报告

> 日期：2026-03-06
> 排查人：蓝莓🫐（测试助手）
> 触发：设计师🍓 代码 review 反馈

---

## 一、排查结论总览

| # | 问题 | 严重度 | 是否属实 | 处理方式 |
|---|------|--------|---------|---------|
| 1 | 包名 `src` 应改为 `see_agent` | 🔴 高 | ✅ 属实 | Phase 1 改 |
| 2 | 90MB 旧日志文件 + 日志打 message body | 🔴 高 | ✅ 属实 | Phase 1 删 + 瘦身 |
| 3 | 44 个空任务目录 | 🟡 低 | ✅ 属实（实测 44 个，非 51 个） | Phase 3 出 CLI 清理 |
| 4 | config.json 明文 API key | ⚪ 忽略 | ✅ 属实 | 不处理，同 CC 做法 |
| 5 | PRD.md 在 git 仓库 | 🟡 中 | ✅ 属实 | 管理者手动处理 |
| 6 | workspace/config.json 模板缺字段 | 🟡 中 | ✅ 属实 | Phase 1 改 |
| 7 | 缺 `__main__.py` | 🟡 低 | ✅ 属实 | Phase 1 加 |
| 8 | 当前日志仍有万字行 | 🟡 中 | ✅ 属实（205 行 >10K 字符，但不含 base64） | Phase 2 随会话化解决 |
| 9 | PNG/WebP 截图混合 | ⚪ 低 | ✅ 属实（16 PNG + 473 WebP） | 不影响功能，不处理 |
| 10 | 无截图自动清理机制 | 🟡 中 | ✅ 属实（77 任务 216MB） | Phase 3 出 CLI |

**总结：设计师提的问题全部属实，无误报。**

---

## 二、现有架构分析

### 调用链

```
CLI run/chat ──→ AgentLoop.run(task) ──→ ConversationContext（内存 list[dict]）
                                              │
API POST /api/chat ──→ _run_agent() ─────────┘
                       （每次新建 AgentLoop，app.state.tasks 跟踪状态）
```

### 核心特征

- `AgentLoop` 是一次性的，`run()` 结束即消亡
- `ConversationContext._messages` 是纯内存 `list[dict]`，不持久化
- CLI `chat` 模式下每次输入都新建 `AgentLoop`，上轮 context 丢失
- API 的 `tasks` 是内存 dict，进程重启全部丢失
- 截图按时间戳存到 `~/.see-agent/screenshots/task_YYYYMMDD_HHMMSS/`，跟"会话"无关联
- 日志是唯一的"持久化"渠道，导致 message body 被打进日志（当前 `_summarise_messages` 会截断文本和替换 image_url，但序列化后仍有万字行）

### 工作目录现状

```
~/.see-agent/
├── config.json                           # 含明文 API key
├── SOUL.md
├── logs/
│   ├── 2026-03-05.log                   # 6MB，205 行 >10K 字符
│   └── 2026-03-05.log.1                 # 90MB，最长行 821 万字符（openai SDK debug 残留）
└── screenshots/
    ├── task_20260305_104415/             # 空目录 ×44
    ├── task_20260305_134335/             # 8 张 webp
    └── ...（共 77 个目录，216MB）
```

---

## 三、改造方案

### Phase 1：卫生清理（小改动，立即可做）

#### 1.1 包名 `src` → `see_agent`

**原因：** `src` 是全 Python 生态最容易撞名的包名，语义不清，发布 PyPI 几乎不可能。项目仅 13 个 commit，现在改代价最小。

**改动：**

```bash
# 目录重命名
mv src/ see_agent/

# pyproject.toml
packages = ["see_agent"]            # 原: ["src"]
see-agent = "see_agent.cli.main:app"  # 原: "src.cli.main:app"
```

**全局替换所有 import：**

```python
# 之前
from src.agent.loop import AgentLoop
from src.brain.openai_client import OpenAIBrain
from src.config import load_config

# 之后
from see_agent.agent.loop import AgentLoop
from see_agent.brain.openai_client import OpenAIBrain
from see_agent.config import load_config
```

**涉及文件（全量列表）：**

| 文件 | 改动 |
|------|------|
| `pyproject.toml` | packages + entry point |
| `see_agent/main.py` | import path |
| `see_agent/agent/loop.py` | 4 处 import |
| `see_agent/brain/openai_client.py` | import |
| `see_agent/cli/main.py` | 4 处 import |
| `see_agent/server/app.py` | 4 处 import |
| `see_agent/server/routes/chat.py` | 5 处 import |
| `see_agent/server/routes/task.py` | 1 处 import |
| `see_agent/server/routes/ws.py` | 无需改 |
| `see_agent/hand/tools/*.py` | 检查 import |
| `see_agent/eye/*.py` | 检查 import |
| `see_agent/overlay/*.py` | 检查 import |
| `tests/**/*.py` | 全部 import |
| `CLAUDE.md` | 如有路径引用 |

**验证：** 改完后 `ruff check .` + `pytest` + `see-agent --help` 确认无报错。

#### 1.2 删除旧日志

```bash
rm ~/.see-agent/logs/2026-03-05.log.1   # 90MB 废文件
```

#### 1.3 日志瘦身（砍 message body）

**文件：** `see_agent/brain/openai_client.py`

```python
# 之前（第 94-97 行）
logger.info(
    "LLM request messages: %s",
    json.dumps(_summarise_messages(messages), ensure_ascii=False),
)

# 之后
logger.info(
    "LLM request: model=%s messages=%d",
    self._model,
    len(messages),
)
logger.debug(  # 降到 DEBUG，需要时再开
    "LLM request messages: %s",
    json.dumps(_summarise_messages(messages), ensure_ascii=False),
)
```

#### 1.4 workspace/config.json 模板同步

**文件：** `workspace/config.json`

```json
{
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o"
    },
    "language": "zh",
    "max_steps": 50,
    "max_images": 5,
    "screenshot_interval_ms": 800,
    "show_overlay": true,
    "scaling_enabled": true,
    "scaling_match": "aspect_ratio",
    "soul_path": "~/.see-agent/SOUL.md"
}
```

需要与 `config.py` 中的 `DEFAULT_CONFIG` 完全一致。或者更好的做法——**`config init` 直接从 `DEFAULT_CONFIG` 生成**，去掉模板文件的维护负担：

```python
# config.py ensure_workspace() 中
if not CONFIG_PATH.exists():
    CONFIG_PATH.write_text(json.dumps(DEFAULT_CONFIG, indent=4, ensure_ascii=False))
```

#### 1.5 添加 `__main__.py`

**新建文件：** `see_agent/__main__.py`

```python
"""Allow ``python -m see_agent`` to launch the CLI."""
from see_agent.cli.main import app

app()
```

---

### Phase 2：会话持久化（核心改造）

#### 2.1 设计目标

- message 历史持久化到 JSONL，不再依赖日志
- 截图归属到会话目录，天然隔离
- 支持从已有会话恢复（崩溃恢复 / 继续对话）
- 不引入数据库，纯文件方案

#### 2.2 新目录结构

```
~/.see-agent/
├── config.json
├── SOUL.md
├── logs/
│   └── 2026-03-06.log              # 纯运行日志，不含 message body
└── sessions/
    └── <session_id>/
        ├── meta.json                # 会话元数据
        ├── messages.jsonl           # 消息历史（不含 base64）
        └── screenshots/
            ├── step_000.webp
            ├── step_001.webp
            └── ...
```

#### 2.3 meta.json 结构

```json
{
    "id": "20260306_143000_a1b2c3",
    "task": "打开 Safari 搜索天气",
    "status": "completed",
    "created_at": "2026-03-06T14:30:00+08:00",
    "updated_at": "2026-03-06T14:32:15+08:00",
    "total_steps": 8,
    "elapsed_seconds": 135.2,
    "summary": "已完成搜索",
    "config_snapshot": {
        "model": "claude-opus-4-6",
        "max_steps": 70,
        "scaling_enabled": true
    }
}
```

#### 2.4 messages.jsonl 格式

每行一条消息，**不存 base64 图片数据**，只存截图文件引用：

```jsonl
{"ts":"2026-03-06T14:30:00Z","type":"system","content":"你是一个能操作 Mac 电脑的 AI 助手..."}
{"ts":"2026-03-06T14:30:01Z","type":"user_task","text":"打开 Safari 搜索天气","screenshot":"step_000.webp","detail":"high"}
{"ts":"2026-03-06T14:30:03Z","type":"assistant","content":"我来帮你打开Safari","tool_calls":[{"id":"tc_001","name":"click","args":{"x":100,"y":50}}]}
{"ts":"2026-03-06T14:30:04Z","type":"tool_result","tool_call_id":"tc_001","result":"ok","screenshot":"step_001.webp"}
{"ts":"2026-03-06T14:30:06Z","type":"assistant","content":"Safari 已打开","tool_calls":[{"id":"tc_002","name":"type_text","args":{"text":"天气"}}]}
{"ts":"2026-03-06T14:30:07Z","type":"tool_result","tool_call_id":"tc_002","result":"ok","screenshot":"step_002.webp"}
```

**恢复逻辑：** 逐行读 JSONL → 对 `screenshot` 字段从本地文件加载 base64 → 重建 `ConversationContext._messages`。

#### 2.5 新增 Session 模块

**新建 `see_agent/session/` 目录：**

```
see_agent/session/
├── __init__.py
├── store.py          # SessionStore：create / load / list / delete
└── models.py         # Session 数据模型
```

**核心接口设计：**

```python
class SessionStore:
    """纯文件的会话存储，基于 ~/.see-agent/sessions/"""

    @staticmethod
    def create(task: str, config: dict) -> Session:
        """创建新会话，建立目录结构，写入 meta.json"""

    @staticmethod
    def load(session_id: str) -> Session:
        """加载已有会话，从 meta.json + messages.jsonl 恢复"""

    @staticmethod
    def list(status: str | None = None, limit: int = 20) -> list[SessionSummary]:
        """列出会话，按时间倒序，支持按状态过滤"""

    @staticmethod
    def delete(session_id: str) -> None:
        """删除会话目录"""

    @staticmethod
    def clean(keep_days: int = 7) -> int:
        """清理过期会话，返回删除数量"""


class Session:
    """单个会话实例，封装消息读写"""

    id: str
    task: str
    status: str
    dir: Path                          # 会话目录路径
    messages: ConversationContext       # 内存中的消息列表

    def append_message(self, msg: dict) -> None:
        """追加消息到内存 + 写一行到 messages.jsonl"""

    def save_screenshot(self, step: int, image_data: bytes) -> Path:
        """保存截图到会话目录，返回文件路径"""

    def update_status(self, status: str, **kwargs) -> None:
        """更新 meta.json 中的状态"""

    def restore_context(self) -> ConversationContext:
        """从 messages.jsonl + 截图文件重建完整的 ConversationContext"""
```

#### 2.6 ConversationContext 改造

**文件：** `see_agent/agent/context.py`

改动思路：每个 `add_*` 方法增加一个可选的回调/钩子，在内存操作之后通知 Session 写 JSONL。

```python
class ConversationContext:
    def __init__(self, system_prompt: str, max_images: int = 5,
                 on_append: Callable[[dict], None] | None = None):
        self._on_append = on_append   # 新增：持久化回调
        ...

    def add_user_task(self, text, screenshot_b64, detail, mime_type="image/webp",
                      screenshot_ref: str | None = None):   # 新增：截图文件引用
        self._messages.append(...)
        if self._on_append:
            self._on_append({
                "type": "user_task", "text": text,
                "screenshot": screenshot_ref, "detail": detail,
            })
```

#### 2.7 AgentLoop 改造

**文件：** `see_agent/agent/loop.py`

```python
class AgentLoop:
    async def run(self, task: str, session_id: str | None = None) -> RunResult:
        # 新增：会话管理
        if session_id:
            session = SessionStore.load(session_id)
            ctx = session.restore_context()
            task_dir = session.dir / "screenshots"
            # 从 meta.json 读取已完成步数，继续编号
        else:
            session = SessionStore.create(task, self._config)
            ctx = ConversationContext(
                system_prompt, max_images=self._max_images,
                on_append=session.append_message,  # 挂钩子
            )
            task_dir = session.dir / "screenshots"

        # 主循环不变，但截图存到 session 目录
        # 每步结束后 session.update_status("running", steps=step)
        # 结束时 session.update_status("completed", summary=..., elapsed=...)
```

#### 2.8 config.py 目录变更

```python
# 新增
SESSIONS_DIR = WORKSPACE_DIR / "sessions"

# ensure_workspace() 中
SESSIONS_DIR.mkdir(exist_ok=True)

# SCREENSHOTS_DIR 保留向后兼容，但新任务不再使用
```

---

### Phase 3：会话管理（CLI + API 扩展）

#### 3.1 CLI 新增命令

```bash
# 列出会话
see-agent sessions list
see-agent sessions list --status completed --limit 10

# 输出示例：
# ID                        TASK                  STATUS     STEPS  TIME     DATE
# 20260306_143000_a1b2c3    打开Safari搜索天气      completed  8      2m15s    03-06 14:30
# 20260306_141500_d4e5f6    截图发给微信            failed     3      0m45s    03-06 14:15
# 20260305_201000_g7h8i9    写一封邮件              completed  15     5m30s    03-05 20:10

# 查看会话详情
see-agent sessions show <session_id>

# 从上次中断处继续
see-agent resume <session_id>
see-agent resume --last                  # 继续最近的会话

# 清理旧会话
see-agent sessions clean                 # 默认清理 7 天前的
see-agent sessions clean --keep 3d       # 清理 3 天前的
see-agent sessions clean --keep 0        # 全部清理
see-agent sessions clean --empty         # 只清空目录（无截图的）

# 输出示例：
# 🧹 Cleaned 23 sessions (180MB freed), kept 12 sessions
```

**CLI 实现：** `see_agent/cli/main.py` 新增 `sessions_app` Typer 子命令组。

#### 3.2 API 新增接口

```
GET  /api/sessions                          # 列出会话
     ?status=completed&limit=20

GET  /api/sessions/<session_id>             # 会话详情 + 步骤历史
     返回: meta.json + messages 摘要（不含截图 base64）

GET  /api/sessions/<session_id>/screenshot/<step>
     返回: 截图文件（WebP）

POST /api/chat
     {"task": "...", "session_id": "xxx"}   # 新增可选字段，继续已有会话

DELETE /api/sessions/<session_id>           # 删除会话
```

**API 实现：** 新建 `see_agent/server/routes/sessions.py`，注册到 `app.py`。

#### 3.3 chat 模式改造

现在 CLI `chat` 模式每次输入都新建 `AgentLoop`，context 丢失。改造后：

```python
@app.command()
def chat():
    # 整个 chat 会话共享一个 Session
    session = SessionStore.create("interactive-chat", config)

    while True:
        task = input("> ")
        # 同一个 session，AgentLoop 从 session 恢复 context
        result = asyncio.run(loop.run(task, session_id=session.id))
```

---

## 四、执行顺序与依赖

```
Phase 1（无依赖，可并行）
├── 1.1 包名重命名 src → see_agent
├── 1.2 删旧日志
├── 1.3 日志瘦身
├── 1.4 模板同步
└── 1.5 添加 __main__.py

Phase 2（依赖 Phase 1 的包名改完）
├── 2.5 新建 session 模块（可先做，无依赖）
├── 2.8 config.py 目录变更
├── 2.6 ConversationContext 改造（依赖 2.5）
├── 2.7 AgentLoop 改造（依赖 2.5 + 2.6）
└── 2.4 日志切换（message body → JSONL 后可彻底砍日志）

Phase 3（依赖 Phase 2）
├── 3.1 CLI sessions 命令
├── 3.2 API sessions 接口
└── 3.3 chat 模式改造
```

---

## 五、不处理的项

| 项 | 理由 |
|----|------|
| config.json 明文 API key | 与 CC 做法一致，不改 |
| PRD.md 位置 | 管理者手动处理 |
| PNG/WebP 混合 | 不影响功能，老截图会被清理掉 |

---

## 六、参考：OpenClaw 的做法

OpenClaw 采用类似的分层设计，可供参考：

| 层 | 文件 | 内容 |
|----|------|------|
| 会话记录 | `agents/<name>/sessions/<uuid>.jsonl` | 每条消息一行 JSON，含 role/content/toolCall/toolResult |
| 网关日志 | `logs/gateway.log` | 纯运行时事件，不含消息内容 |
| 日志格式 | JSONL | 结构化日志，带日期滚动 |
| 诊断 | OTel 导出 | token 用量、耗时、cost（可选） |

**关键原则：日志和会话记录分离。** 日志只记"发生了什么事"，消息内容走专门的会话文件。
