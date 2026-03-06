# see-agent v2 PRD

> **一句话**：自由的、能看见的 AI Agent —— 像 OpenClaw 一样通用，但天然支持多模态视觉和桌面操作。

## 1. 产品定位

### v1 是什么
v1 是一个**截图驱动的桌面操作专才**。每一步都是固定循环：截图 → LLM 看图 → 调 tool → 截图 → …。LLM 的输出被限定在 tool calling 上，不能自由对话，截图硬编码在 loop 里。

### v2 要做什么
v2 是一个**自由的通用 Agent，碰巧能看见屏幕**。核心变化：

- 截图从 loop 的硬编码行为变成一个**普通 tool**，LLM 自己决定什么时候看屏幕
- LLM 可以自由对话、自由思考，不再只能输出 tool calls
- 支持 **Skill 生态**（兼容 ClawHub 格式），通过 prompt 注入扩展能力
- 支持 **MCP 协议**，接入外部工具服务器
- 支持**一次推理多 tool 调用**，串行执行，可配置延时
- 接入 **Mem0 记忆系统**，任务级 search + add
- 支持**多配置 Profile**，快速切换模型和参数

### 设计哲学

| 原则 | 说明 |
|------|------|
| **视觉是能力，不是枷锁** | 截图是 tool 之一，不是 loop 的骨架 |
| **自由 ReAct** | LLM 可以纯文字回复，也可以调 tool，也可以混合 |
| **Skill 即 Prompt** | Skill 不是代码插件，是知识包，通过 prompt 注入 |
| **记忆在任务边界** | 任务开始检索记忆，任务结束存储记忆，loop 中间不碰 |
| **文件即配置** | 参考 OpenClaw，配置、人格、记忆都是文件 |

---

## 2. v1 → v2 变更总览

### 2.1 v1 现状（基线）

**源码结构**（~3800 行 Python，15 个 commit）：

```
see_agent/
├── __init__.py
├── __main__.py
├── main.py                          # if __name__ == "__main__" 入口
├── config.py                        # 配置加载、日志初始化、路径常量
│
├── agent/                           # 🧠 Agent 编排层
│   ├── loop.py                      # AgentLoop — 核心 ReAct 循环
│   ├── context.py                   # ConversationContext — 消息管理 + 截图滑动窗口
│   └── environment.py               # 桌面环境感知（运行应用、分辨率等）
│
├── brain/                           # 🤖 LLM 调用层
│   ├── base.py                      # BaseBrain 抽象 + BrainResponse 数据结构
│   ├── openai_client.py             # OpenAI 协议实现（兼容所有 OpenAI API）
│   └── prompts.py                   # System prompt 构建（函数拼接 + XML 标签）
│
├── eye/                             # 👁️ 视觉感知层
│   ├── base.py                      # BaseEye 抽象 + Screenshot 数据类
│   ├── mac.py                       # macOS 截屏实现（CGWindowListCreateImage）
│   └── scaling.py                   # 坐标缩放（大屏→标准分辨率→反算）
│
├── hand/                            # 🖐️ 工具执行层
│   ├── tool.py                      # Tool 抽象基类 + ToolRegistry
│   └── tools/
│       ├── __init__.py              # create_registry() 工厂
│       ├── click.py                 # 鼠标点击（单击/双击/右键）
│       ├── type_text.py             # 键盘输入（中文走剪贴板）
│       ├── hotkey.py                # 快捷键组合
│       ├── scroll.py                # 滚动
│       ├── drag.py                  # 拖拽
│       ├── shell.py                 # 终端命令
│       ├── screenshot.py            # 手动截屏（当前仅返回文本）
│       ├── wait.py                  # 等待 N 秒
│       ├── finished.py              # 标记任务完成
│       └── call_user.py             # 请求人工介入
│
├── overlay/                         # 🎨 视觉反馈层
│   ├── base.py                      # OverlayRenderer 抽象
│   └── mac_overlay.py               # PyObjC 透明窗口（setSharingType_0 防截图捕获）
│
├── session/                         # 💾 会话持久化
│   └── store.py                     # SessionStore + Session（JSONL + 截图文件引用）
│
├── server/                          # 🌐 HTTP/WebSocket 服务
│   ├── app.py                       # FastAPI app
│   ├── models.py                    # Pydantic 请求/响应模型
│   └── routes/
│       ├── health.py                # GET /health
│       ├── chat.py                  # POST /api/chat
│       ├── task.py                  # POST /api/task
│       ├── sessions.py              # GET/DELETE /api/sessions
│       └── ws.py                    # WebSocket /ws
│
└── cli/                             # ⌨️ CLI 命令
    └── main.py                      # Typer app（chat/run/serve/config/sessions/resume）
```

**工作目录**（`~/.see-agent/`）：

```
~/.see-agent/
├── config.json                      # 主配置文件
├── SOUL.md                          # Agent 人格定义
├── logs/                            # 日志（按天轮转，10MB 上限）
│   └── 2026-03-06.log
├── screenshots/                     # 已清空（v1 遗留目录）
└── sessions/                        # 会话存储
    └── 20260306_143919_df7744/
        ├── meta.json                # 元数据（任务、状态、耗时、config 快照）
        ├── messages.jsonl           # 消息日志（无 base64，只有文件引用）
        └── screenshots/             # WebP 截图文件
            ├── step_000.webp
            ├── step_001.webp
            └── ...
```

**v1 设计原理**：

| 模块 | 原理 |
|------|------|
| **Agent Loop** | 单任务串行循环。每步：LLM 推理 → 取第一个 tool call → 执行 → 等待 UI 稳定 → 截屏 → 注入 context → 下一步。硬编码截图注入。 |
| **Brain** | OpenAI 协议 `stream=False`。一次推理可能返回多个 tool_calls，但 loop 只取第一个（`tc = response.tool_calls[0]`）。 |
| **Context** | 滑动窗口裁剪截图（保留最新 N 张），旧截图替换为 `[Screenshot omitted]` 占位符。所有文本永不裁剪。 |
| **Prompt** | 函数拼接 + XML 标签（`<RULES>`, `<CONSTRAINTS>`, `<ENVIRONMENT>`, `<PERSONALITY>`）。规则 1 写死"每次只调用一个工具"。 |
| **Scaling** | 截屏后缩放到标准分辨率（1280×800 等），LLM 在小图上推理坐标，执行前反算回屏幕空间。 |
| **Overlay** | PyObjC 子进程渲染透明窗口，`setSharingType_(0)` 使 overlay 不被截图捕获。 |
| **Session** | JSONL 持久化（不存 base64，只存截图文件名引用），支持 resume。 |
| **Safety** | 截图 hash 检测无进展、重复动作检测（坐标四舍五入到 10px）、连续错误上限、max_steps 兜底。 |

**v1 config.json 配置项**：

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

---

### 2.2 v2 变更清单

| 领域 | v1 | v2 |
|------|----|----|
| **Loop 模式** | 截图硬编码在 loop | 标准 ReAct，截图是普通 tool |
| **Tool 执行** | 只取第一个 tool call | 支持多 tool 串行执行 + 可配延时 |
| **LLM 输出** | 只能 tool call | 可自由对话 + tool call 混合 |
| **Prompt** | 硬编码规则 | 模版化 + Skill 注入 + Memory 注入 |
| **Skill** | 无 | 兼容 ClawHub 格式，扫描 skills/ 目录注入 |
| **MCP** | 无 | 支持接入 MCP 服务器，动态注册 tool |
| **Memory** | 无 | Mem0 集成，任务级 search + add |
| **Profile** | 单配置文件 | 多 profile 支持 |
| **Tool 返回** | 纯文本 `str` | 支持 multimodal（文本 + 图片） |
| **截图触发** | loop 每步自动 | 完全由 LLM 自主决定 |

---

## 3. v2 源码结构

```
see_agent/
├── __init__.py
├── __main__.py
├── config.py                        # 配置加载 + Profile 支持（见 §6）
│
├── agent/                           # 🧠 Agent 编排层
│   ├── loop.py                      # AgentLoop v2 — 自由 ReAct 循环（见 §4）
│   ├── context.py                   # ConversationContext — 消息管理 + 截图滑动窗口
│   └── environment.py               # 桌面环境感知
│
├── brain/                           # 🤖 LLM 调用层
│   ├── base.py                      # BaseBrain 抽象 + BrainResponse
│   ├── openai_client.py             # OpenAI 协议实现
│   └── prompts.py                   # System prompt 构建 v2（见 §5）
│
├── eye/                             # 👁️ 视觉感知层（不变）
│   ├── base.py                      # BaseEye + Screenshot
│   ├── mac.py                       # macOS 截屏
│   └── scaling.py                   # 坐标缩放
│
├── hand/                            # 🖐️ 工具执行层
│   ├── tool.py                      # Tool 基类 v2 + ToolRegistry v2 + ToolResult（见 §4.3）
│   ├── mcp.py                       # 【新增】MCP 客户端 — 连接外部 MCP 服务器（见 §4.5）
│   └── tools/
│       ├── __init__.py              # create_registry() — 注册内置 tool
│       ├── click.py
│       ├── type_text.py
│       ├── hotkey.py
│       ├── scroll.py
│       ├── drag.py
│       ├── shell.py
│       ├── screenshot.py            # 【重构】返回 ToolResult 含图片（见 §4.2）
│       ├── wait.py
│       ├── finished.py
│       └── call_user.py
│
├── memory/                          # 🧠【新增】记忆系统（见 §7）
│   ├── __init__.py
│   └── mem0_backend.py              # Mem0 OSS 封装
│
├── skill/                           # 📚【新增】Skill 加载器（见 §5.3）
│   ├── __init__.py
│   └── loader.py                    # 扫描 skills/ 目录，解析 SKILL.md
│
├── overlay/                         # 🎨 视觉反馈层（不变）
│   ├── base.py
│   └── mac_overlay.py
│
├── session/                         # 💾 会话持久化（不变）
│   └── store.py
│
├── server/                          # 🌐 HTTP/WebSocket 服务
│   ├── app.py
│   ├── models.py
│   └── routes/
│       ├── health.py
│       ├── chat.py
│       ├── task.py
│       ├── sessions.py
│       └── ws.py
│
└── cli/                             # ⌨️ CLI 命令
    └── main.py                      # 新增 --profile 参数
```

---

## 4. Agent Loop v2 设计

### 4.1 自由 ReAct 循环

v1 的 loop 是截图驱动的固定循环；v2 变成标准 ReAct：

```python
async def run(self, task: str, session_id: str | None = None) -> RunResult:
    # 1. 任务开始：检索相关记忆
    memories = self._memory.search(task) if self._memory else []
    
    # 2. 构建 context（注入环境 + 记忆 + 初始截图）
    ctx = self._build_initial_context(task, memories)
    
    # 3. ReAct 循环
    for step in range(1, self._max_steps + 1):
        response = await self._brain.chat(ctx.get_messages(), tools_schema)
        
        # 3a. 纯文字回复（LLM 不想调工具）
        if not response.tool_calls:
            ctx.add_assistant(response.raw)
            # 不 break —— 等下一轮用户输入（chat 模式）
            # 或者 break（run 模式，单任务）
            break
        
        ctx.add_assistant(response.raw)
        
        # 3b. 执行所有 tool calls（串行，可配延时）
        for tc in response.tool_calls:
            if tc.name == "finished":
                ...  # 结束逻辑
            
            result: ToolResult = await self._execute_tool(tc)
            ctx.add_tool_result(tc.id, result)
            
            # tool 间延时（固定配置，不让 LLM 控制）
            if self._tool_delay_ms > 0:
                await asyncio.sleep(self._tool_delay_ms / 1000.0)
    
    # 4. 任务结束：存储记忆
    if self._memory:
        self._memory.add(ctx.get_messages_for_memory(), session.id)
    
    return result
```

**关键变化**：
- LLM 可以一次返回多个 tool calls，全部串行执行
- LLM 可以纯文字回复，不再强制 tool call
- 截图不再每步硬编码注入，完全由 LLM 自主决定何时调用 `screenshot` tool
- 在 prompt `<RULES>` 中引导 LLM "操作后主动截图确认结果"，但不在代码层面强制

### 4.2 ToolResult — 多模态返回

v1 的 tool 返回纯文本 `str`。v2 引入 `ToolResult`，支持文本 + 图片：

```python
@dataclass
class ToolResultImage:
    """Tool 返回的图片。"""
    base64: str
    mime_type: str = "image/webp"
    detail: str = "high"

@dataclass
class ToolResult:
    """Tool 的执行结果，支持多模态。"""
    text: str
    images: list[ToolResultImage] = field(default_factory=list)
    
class Tool(ABC):
    @abstractmethod
    async def execute(self, **kwargs) -> ToolResult:   # 返回类型从 str 变为 ToolResult
        ...
```

**screenshot tool 的变化**：

v1：返回 `"截屏完成"` 文本，截图由 loop 硬编码注入。
v2：返回 `ToolResult(text="截屏完成", images=[ToolResultImage(base64=..., ...)])`，loop 只负责把 images 塞进 context。

```python
class ScreenshotTool(Tool):
    async def execute(self) -> ToolResult:
        screenshot = await self._eye.capture()
        scaled = self._maybe_scale(screenshot)
        return ToolResult(
            text=f"截屏完成 ({scaled.width}x{scaled.height})",
            images=[ToolResultImage(
                base64=scaled.base64,
                mime_type=scaled.mime_type,
                detail=scaled.detail,
            )],
        )
```

### 4.3 多 Tool 执行与延时

v1 只取第一个 tool call。v2 支持一次推理返回的所有 tool calls 串行执行：

```python
# 配置项
"tool_delay_ms": 200,    # tool 间延时（毫秒），默认 200
```

执行顺序严格按 LLM 返回的 tool_calls 数组顺序。例如 LLM 返回 `[click(x,y), screenshot()]`：
1. 执行 click(x,y)
2. 等待 200ms
3. 执行 screenshot()
4. 把两个 ToolResult 都加入 context

**关于延时设计**：tool 间延时是固定配置值 `tool_delay_ms`，不让 LLM 控制。LLM 不具备精确控制时间间隔的能力，这和操作不是一个维度的事。如果某些场景需要等更久（比如等页面加载），LLM 可以在**下一轮推理**中调用 `wait` tool。

分工：
- **多 tool 间的短延时**（UI 动画、渲染）→ 配置项 `tool_delay_ms`，代码自动加
- **长等待**（页面加载、网络请求）→ LLM 自己决定调 `wait` tool

### 4.4 MCP 支持

MCP（Model Context Protocol）允许 see-agent 连接外部工具服务器，动态扩展能力。

**配置格式**（参考 Claude Code 的 settings.json / claude.json）：

```json
// config.json
{
    "env": {
        "TAVILY_API_KEY": "tvly-dev-xxx",
        "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_xxx"
    },
    "mcp_servers": {
        "tavily": {
            "type": "stdio",
            "command": "npx",
            "args": ["tavily-mcp@latest"],
            "env": {}
        },
        "github": {
            "type": "http",
            "url": "https://api.githubcopilot.com/mcp",
            "headers": {
                "Authorization": "Bearer ${GITHUB_PERSONAL_ACCESS_TOKEN}"
            }
        },
        "playwright": {
            "type": "stdio",
            "command": "npx",
            "args": ["@playwright/mcp@latest"],
            "env": {}
        }
    }
}
```

**设计要点**：

1. **全局 `env`**：在 config.json 顶层，所有 MCP server 共享。启动 MCP 子进程时注入这些环境变量。
2. **每个 server 的 `env`**：可以覆盖全局 env，也可以添加 server 专属的环境变量。
3. **`mcp_servers` 是 object（按 name 做 key）**，不是 array。和 CC 保持一致，方便按名字引用和 merge。
4. **`type` 字段**：`"stdio"`（启动子进程）或 `"http"`（连接远程 URL）。
5. **`headers`**：http 模式下的自定义请求头，支持 `${ENV_VAR}` 变量替换。

**MCP CLI 管理命令**：

```bash
see-agent mcp add tavily --type stdio --command npx --args "tavily-mcp@latest"
see-agent mcp add github --type http --url "https://api.githubcopilot.com/mcp"
see-agent mcp list          # 遍历 mcp_servers，逐个尝试连接，报告状态
see-agent mcp remove tavily
```

`mcp add/remove` 本质上就是读写 config.json 里的 `mcp_servers` 对象。`mcp list` 遍历配置 -> 尝试连接 -> 打印健康状态（类似 `claude mcp list`）。

**代码实现**：

```python
# see_agent/hand/mcp.py

class MCPClient:
    """连接单个 MCP 服务器。"""
    
    def __init__(self, name: str, server_config: dict, global_env: dict):
        self._name = name
        self._type = server_config["type"]       # "stdio" | "http"
        self._command = server_config.get("command")
        self._args = server_config.get("args", [])
        self._url = server_config.get("url")
        self._headers = server_config.get("headers", {})
        # 合并环境变量：全局 env + server env
        self._env = {**global_env, **server_config.get("env", {})}
    
    async def connect(self) -> list[Tool]:
        """连接服务器并返回可用的 Tool 列表。"""
        ...
    
    async def execute(self, tool_name: str, args: dict) -> ToolResult:
        """调用远程 tool 并返回结果。"""
        ...
    
    async def health_check(self) -> bool:
        """检查连接健康状态。"""
        ...
```

Agent 启动时连接所有配置的 MCP 服务器，获取 tool 列表，注册到 ToolRegistry。

---

## 5. Prompt 模版设计 v2

### 5.1 整体结构

v1 的 prompt 是硬编码的规则列表。v2 改为模块化拼接，每个段落可以独立开关：

```
┌─────────────────────────────────┐
│ <IDENTITY>                      │  ← 身份定义（固定）
│ <PERSONALITY>                   │  ← SOUL.md 人格注入（可选）
│ <SKILLS>                        │  ← Skill 描述列表（动态）
│ <TOOLS>                         │  ← 可用工具说明（动态）
│ <RULES>                         │  ← 行为规则（v2 更新）
│ <CONSTRAINTS>                   │  ← 安全约束
│ <MEMORY>                        │  ← 相关记忆（Mem0 检索结果）
│ <ENVIRONMENT>                   │  ← 桌面环境信息（运行时注入）
└─────────────────────────────────┘
```

### 5.2 关键 Prompt 段落

**`<IDENTITY>`**（v2 更新）：

```
你是一个能看见屏幕、操作电脑的 AI 助手。你可以自由对话，也可以通过工具操作鼠标、键盘和终端。
当需要观察屏幕时，调用 screenshot 工具。当操作完成后，主动截图确认结果。
```

**`<RULES>`**（v2 更新 — 去掉"每次只调一个工具"的限制）：

```
1. 你可以在一次回复中调用多个工具，它们会按顺序串行执行。
2. 操作前先截图观察，确认要操作的位置。
3. 操作后主动截图确认结果。
4. 能用 shell 命令快速完成的事优先用 shell。
5. 连续 3 次操作没有进展时，重新分析当前状态，尝试不同策略。
6. 任务完成后必须调用 finished 工具。
7. 遇到无法解决的问题调用 call_user。
8. 在思考中维护累积状态摘要，早期截图会被裁剪，你的思考是唯一的历史记录。
```

**`<TOOLS>`**（v2 新增段落）：

动态生成，列出所有可用工具（内置 + MCP + Skill 提供的）的名称和简要说明。

**`<MEMORY>`**（v2 新增段落）：

```xml
<MEMORY>
以下是与当前任务可能相关的历史记忆：
- 用户偏好使用 Microsoft Edge 浏览器
- 钉钉搜索框在左侧边栏顶部，可用 Cmd+K 唤起
- 雪球 App 的登录状态正常
如果记忆与当前任务相关，可以直接利用。如果不相关，忽略即可。
</MEMORY>
```

### 5.3 Skill 系统

**Skill 格式兼容 ClawHub**：

```
~/.see-agent/skills/
├── web-search/
│   └── SKILL.md          # YAML frontmatter + Markdown 指令
├── dingtalk/
│   └── SKILL.md
└── ...
```

SKILL.md 格式：

```markdown
---
name: dingtalk
description: 操作钉钉桌面客户端
---

# 钉钉操作指南

## 搜索联系人
1. 使用 Cmd+K 打开搜索框
2. 输入联系人名称
3. 从搜索结果中点击目标

## 发送消息
1. 进入对话窗口后，点击输入框
2. 使用 type_text 输入消息
3. 按 Enter 发送
```

**加载逻辑**：

```python
# see_agent/skill/loader.py

class SkillLoader:
    def scan(self, skills_dirs: list[Path]) -> list[SkillInfo]:
        """扫描目录，解析所有 SKILL.md 的 frontmatter。"""
        ...
    
    def build_prompt_section(self, skills: list[SkillInfo], task: str) -> str:
        """根据任务描述，选择相关 Skill 的内容注入 prompt。
        
        不是把所有 Skill 的完整内容都塞进去（太长），而是：
        1. 所有 Skill 的 name + description 组成列表注入
        2. 当 LLM 判断需要某个 Skill 时，通过 tool 加载完整内容
        """
        ...
```

**Skill 目录搜索路径**：

```python
SKILL_DIRS = [
    Path.home() / ".see-agent" / "skills",       # see-agent 专属
    Path.home() / ".openclaw" / "skills",         # 共享 OpenClaw 的 skills
]
```

这样 `clawhub install some-skill --dir ~/.see-agent/skills/` 安装的 skill 可以直接被 see-agent 使用。同时如果 OpenClaw 已经安装了某些 skill，see-agent 也能读到。

---

## 6. 多配置 Profile

### 6.1 目录结构

```
~/.see-agent/
├── config.json              # 默认配置（base）
└── profiles/
    ├── opus.json            # Claude Opus 配置
    ├── gpt4o.json           # GPT-4o 配置
    ├── local.json           # 本地模型配置
    └── fast.json            # 快速模式（低 max_steps，小模型）
```

### 6.2 加载逻辑

Profile 文件只需写差异项，会 overlay 到 base config 之上：

```json
// profiles/opus.json
{
    "llm": {
        "base_url": "https://matrixllm.alipay.com/v1",
        "model": "claude-opus-4-6"
    },
    "max_steps": 70
}
```

加载优先级：`DEFAULT_CONFIG → config.json → profiles/{config.profile 或 --profile}.json → 环境变量`

### 6.3 CLI 使用

```bash
see-agent chat                       # 使用 config.json 中 "profile" 指定的默认 profile
see-agent chat --profile opus        # 覆盖默认，使用 opus profile
see-agent run "打开钉钉" --profile fast
see-agent config show --profile opus    # 查看合并后的配置
```

`--profile` CLI 参数优先级最高，config.json 里的 `"profile"` 是默认值。日常使用直接 `see-agent chat` 就能用默认 profile，想临时换时才加 `--profile`。

### 6.4 v2 config.json 完整配置项

```json
{
    "profile": null,
    "env": {},
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o"
    },
    "language": "zh",
    "max_steps": 50,
    "max_images": 5,
    "screenshot_interval_ms": 800,
    "tool_delay_ms": 200,
    "show_overlay": true,
    "scaling_enabled": true,
    "scaling_match": "aspect_ratio",
    "soul_path": "~/.see-agent/SOUL.md",
    "skills_dirs": [
        "~/.see-agent/skills",
        "~/.openclaw/skills"
    ],
    "mcp_servers": {},
    "memory": {
        "enabled": false,
        "provider": "mem0",
        "mem0": {
            "llm_base_url": "",
            "llm_api_key": "",
            "llm_model": "gpt-4.1-nano",
            "embedding_model": "text-embedding-3-small",
            "storage_path": "~/.see-agent/memory"
        }
    }
}
```

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `profile` | str\|null | null | 【新】默认 profile 名称，null 不使用 |
| `env` | object | {} | 【新】全局环境变量，注入所有 MCP 子进程 |
| `llm.base_url` | str | OpenAI | LLM API 地址 |
| `llm.api_key` | str | "" | API Key（优先用环境变量） |
| `llm.model` | str | "gpt-4o" | 模型 ID |
| `language` | str | "zh" | 提示词语言 |
| `max_steps` | int | 50 | 单任务最大步数 |
| `max_images` | int | 5 | 上下文保留截图数 |
| `screenshot_interval_ms` | int | 800 | 截图后等待 UI 稳定时间 |
| `tool_delay_ms` | int | 200 | 【新】多 tool 间延时 |
| `show_overlay` | bool | true | 是否显示可视化 overlay |
| `scaling_enabled` | bool | true | 是否缩放截图 |
| `scaling_match` | str | "aspect_ratio" | 缩放匹配模式 |
| `soul_path` | str | ~/.see-agent/SOUL.md | 人格文件路径 |
| `skills_dirs` | list | 见上 | 【新】Skill 搜索目录 |
| `mcp_servers` | object | {} | 【新】MCP 服务器配置（按 name 做 key） |
| `memory.enabled` | bool | false | 【新】是否启用记忆 |
| `memory.provider` | str | "mem0" | 【新】记忆后端 |
| `memory.mem0.*` | - | - | 【新】Mem0 配置 |

---

## 7. 记忆系统（Mem0）

### 7.1 设计决策

采用**方案 C：任务级 search + add**。

- **任务开始前**：用任务描述做语义搜索，检索相关记忆，注入 `<MEMORY>` prompt 段
- **Loop 中间**：不碰记忆，不增加额外 API 调用
- **任务结束后**：把对话 messages（去掉 base64 截图）存入 Mem0，由 Mem0 的 LLM 自动提取记忆

### 7.2 Mem0 封装

```python
# see_agent/memory/mem0_backend.py

class Mem0Memory:
    """Mem0 OSS 封装，提供 search / add 接口。"""
    
    def __init__(self, config: dict):
        from mem0 import Memory
        self._m = Memory.from_config({
            "llm": {
                "provider": "openai",
                "config": {
                    "model": config["llm_model"],
                    "api_key": config["llm_api_key"],
                    "openai_base_url": config.get("llm_base_url"),
                }
            },
            "embedder": {
                "provider": "openai",
                "config": {
                    "model": config["embedding_model"],
                    "api_key": config["llm_api_key"],
                    "openai_base_url": config.get("llm_base_url"),
                }
            },
            "vector_store": {
                "provider": "qdrant",
                "config": {
                    "collection_name": "see_agent",
                    "path": config.get("storage_path", "~/.see-agent/memory"),
                }
            }
        })
    
    def search(self, query: str, user_id: str = "default", limit: int = 5) -> list[str]:
        """语义搜索相关记忆，返回文本列表。"""
        results = self._m.search(query, user_id=user_id, limit=limit)
        return [r["memory"] for r in results.get("results", [])]
    
    def add(self, messages: list[dict], session_id: str, user_id: str = "default") -> None:
        """从对话 messages 中提取记忆并存储。
        
        传入的 messages 应已去掉 base64 截图数据（只保留文本内容）。
        Mem0 内部会用 LLM 自动提取值得记住的信息。
        """
        self._m.add(messages, user_id=user_id, metadata={"session_id": session_id})
```

### 7.3 依赖安装

```toml
# pyproject.toml
dependencies = [
    # ... existing ...
    "mem0ai>=1.0.0",
]
```

Mem0 OSS 自带 Qdrant（on-disk 模式），不需要额外部署服务。

### 7.4 记忆生命周期

```
用户输入任务 → mem0.search(task) → 注入 <MEMORY> 到 prompt
                                            │
                                    ┌───────▼───────┐
                                    │  ReAct Loop    │  ← 不碰记忆
                                    │  (多步执行)     │
                                    └───────┬───────┘
                                            │
任务结束（finished / max_steps）→ mem0.add(messages_without_base64)
```

---

## 8. v2 工作目录结构

```
~/.see-agent/
├── config.json                      # 主配置
├── SOUL.md                          # Agent 人格
├── profiles/                        # 【新增】多配置 Profile
│   ├── opus.json
│   ├── gpt4o.json
│   └── ...
├── skills/                          # 【新增】Skill 目录
│   ├── dingtalk/
│   │   └── SKILL.md
│   └── ...
├── memory/                          # 【新增】Mem0 向量库存储
│   └── qdrant/                      # Qdrant on-disk 数据
├── logs/
│   └── 2026-03-06.log
└── sessions/
    └── 20260306_HHMMSS_xxxxxx/
        ├── meta.json
        ├── messages.jsonl
        └── screenshots/
```

---

## 9. 实施路线

```
Phase 0: 多配置 Profile
  ├── profiles/ 目录 + overlay 加载逻辑
  ├── config.json 加 "profile" 默认值字段
  ├── CLI --profile 参数
  └── config show --profile

Phase 1: Loop 重构 → 自由 ReAct
  ├── ToolResult 多模态返回（替换 str）
  ├── screenshot tool 重构（返回 ToolResult 含图片）
  ├── Loop 支持多 tool 串行执行 + tool_delay_ms 延时
  ├── Loop 支持纯文字回复
  ├── Prompt v2（去掉"每次只调一个工具"限制，截图由 LLM 自主决定）
  └── 更新所有测试

Phase 2: Skill 生态
  ├── skill/loader.py — 扫描 + 解析 SKILL.md
  ├── Prompt 注入 <SKILLS> 段落
  ├── skills_dirs 配置项
  └── 验证 ClawHub skill 格式兼容

Phase 3: Mem0 记忆系统
  ├── memory/mem0_backend.py
  ├── Loop 前 search + 后 add（全部 messages，去掉 base64）
  ├── Prompt 注入 <MEMORY> 段落
  ├── memory 配置项
  └── 测试记忆检索 + 存储

Phase 4: MCP 支持
  ├── hand/mcp.py — MCP 客户端
  ├── config.json 加 env + mcp_servers（object 格式）
  ├── CLI: see-agent mcp add/list/remove
  ├── 启动时连接 + 注册 tool
  └── 测试 stdio + http 两种 transport
```

---

## 10. 关键设计约束

1. **向后兼容**：v1 的 config.json 在 v2 中仍然可用（新字段有默认值）
2. **渐进式**：每个 Phase 独立可交付，Phase 0 完成后 v1 功能不受影响
3. **Python 包依赖**：新增 `mem0ai`；MCP 可选（`mcp[cli]`）
4. **不改 session 格式**：messages.jsonl 的 schema 向后兼容，新增 type 不影响旧数据
5. **Prompt 的规则改变**：v2 去掉了"每次只调一个工具"的限制，这是**破坏性变更**——需要在 Phase 1 测试中重点验证 LLM 在新规则下的行为
