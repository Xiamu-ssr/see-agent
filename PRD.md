# PRD — see-agent v0.1

> 给 Claude Code 看的完整项目文档。按此文档实现 v0.1 MVP。

---

## 一、产品概述

### 一句话
一个能看懂屏幕、操作电脑的 Mac AI 助手。用户用自然语言描述任务，Agent 通过截图理解界面、调用工具操作鼠标键盘，自动完成桌面操作。

### 项目名
see-agent

### MVP 验收标准
用户输入"帮我在钉钉给张三发一条消息说明天开会"，Agent 自动完成：打开钉钉 → 搜索联系人 → 输入消息 → 发送。

### 技术栈
- 语言：Python 3.11+
- 包管理：uv
- LLM 协议：OpenAI Chat Completions API（兼容所有供应商）
- LLM SDK：openai (Python)
- Web 框架：FastAPI（异步，WebSocket）
- 截图：PyAutoGUI + Pillow（跨平台）
- 鼠标键盘操作：PyAutoGUI（跨平台，Mac/Windows/Linux）
- CLI 框架：typer

### 核心设计原则
1. **单 Agent 多 Tool**：一个 LLM 大脑，多个工具。不用父子 Agent、不用 Agent 框架。
2. **OpenAI 协议 + 原生 Tool Calling**：tools 通过 API 参数传入，模型自动感知。不在提示词里写工具列表，不用正则解析输出。
3. **纯视觉路线**：截图发给 LLM 分析，不依赖 Accessibility API。天然跨平台。
4. **前后端分离**：FastAPI 后端 + CLI 客户端。未来可接 Web/桌面前端。
5. **函数拼接提示词**：用 Python 函数拼接 + XML 标签，不用占位符模板（避免自举陷阱）。

---

## 二、核心循环

```
用户: "帮我在钉钉给张三发消息"
        │
        ▼
  ┌──────────┐
  │ 初始截图  │  任务开始前先截一张当前桌面
  └────┬─────┘
       │
       ▼
  ┌──────────┐
  │  发给 LLM │  system prompt + messages(含历史截图) + tools
  └────┬─────┘
       │
       ▼
  ┌──────────────┐
  │  解析 LLM 返回 │  tool_calls → json.loads 解析参数
  └────┬─────────┘
       │
       ├── finished? → 结束循环，输出 summary（loop 层处理，不走 registry）
       ├── call_user? → 暂停等用户输入后继续（loop 层处理，不走 registry）
       │
       ▼
  ┌──────────┐
  │  执行操作 │  ToolRegistry.execute(name, args) → PyAutoGUI
  └────┬─────┘
       │
       ▼
  ┌──────────┐
  │  等待+截图│  等 500ms → 新截图 → 追加到 messages → 保存到磁盘
  └────┬─────┘
       │
       ▼
  (回到 "发给 LLM"，直到 finished / call_user / 达到 max_steps)
```

### 上下文管理（messages 数组）

messages 是一个 Python list，存在内存中，生命周期 = 一次任务。

- 每轮追加：assistant response + tool result + 新截图
- **滑动窗口**：保留最新 N 张截图（默认 5），老截图替换为 `[Screenshot omitted]`
- 所有文本历史（LLM 的思考和操作描述）始终保留
- 任务结束后 messages 释放，不做持久化（截图 base64 太大）
- v1 不使用 streaming（`stream=False`），简化 tool_calls 解析。LLM 完整响应后再处理。

### 截图处理

- macOS Retina 物理分辨率是逻辑分辨率的 2 倍
- PyAutoGUI 截图返回的已经是逻辑分辨率（PyAutoGUI 在 Mac 上自动处理 Retina）
- 但如果不是（需要运行时检查），则用 Pillow resize 到逻辑分辨率
- 格式：PNG（无损）
- 发给 LLM 时根据尺寸设 detail：<= 1024x1024 用 low，否则 high

### 坐标映射

- LLM 看到的截图是逻辑分辨率
- LLM 输出的坐标是逻辑像素
- PyAutoGUI 操作也使用逻辑像素（Mac 上自动处理 Retina）
- 所以：LLM 输出坐标 → 直接传给 PyAutoGUI，不需要额外转换

---

## 三、Tool 定义（OpenAI 格式）

通过 API 的 `tools` 参数传入，模型自动感知可用工具。不需要在 system prompt 里写工具列表。

### Tool 架构

每个 Tool 是一个 Python 类（继承 `Tool` 基类），自包含名字、描述、参数定义和执行逻辑。
ToolRegistry 管理所有 Tool，调 API 时用 `registry.get_openai_schemas()` 自动生成下面的列表。

**扩展新 tool**：新建一个类文件 → 实现 Tool 基类 → 在 registry 注册 → 自动生效。

### 当前 Tool 列表（v0.1 共 10 个）

```python
TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "screenshot",
            "description": "截取当前屏幕截图，用于观察当前界面状态。在不确定当前状态时使用。",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "click",
            "description": "点击屏幕上的指定坐标。坐标是逻辑像素，左上角为 (0,0)。",
            "parameters": {
                "type": "object",
                "properties": {
                    "x": {"type": "integer", "description": "横坐标（逻辑像素）"},
                    "y": {"type": "integer", "description": "纵坐标（逻辑像素）"},
                    "button": {"type": "string", "enum": ["left","right","middle"], "default": "left"},
                    "double": {"type": "boolean", "default": false}
                },
                "required": ["x", "y"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "type_text",
            "description": "在当前焦点位置输入文字。中文通过剪贴板粘贴实现。如需按回车提交，在 text 末尾加 \\n。",
            "parameters": {
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "要输入的文字"}
                },
                "required": ["text"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "hotkey",
            "description": "按下快捷键组合。例如 ['command','c'] 表示 Cmd+C。",
            "parameters": {
                "type": "object",
                "properties": {
                    "keys": {"type": "array", "items": {"type": "string"}, "description": "按键列表"}
                },
                "required": ["keys"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "scroll",
            "description": "在指定位置滚动。",
            "parameters": {
                "type": "object",
                "properties": {
                    "x": {"type": "integer", "description": "滚动位置横坐标"},
                    "y": {"type": "integer", "description": "纵坐标"},
                    "direction": {"type": "string", "enum": ["up","down","left","right"]},
                    "amount": {"type": "integer", "default": 3, "description": "滚动格数"}
                },
                "required": ["x", "y", "direction"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "drag",
            "description": "从一个坐标拖拽到另一个坐标。",
            "parameters": {
                "type": "object",
                "properties": {
                    "start_x": {"type": "integer"},
                    "start_y": {"type": "integer"},
                    "end_x": {"type": "integer"},
                    "end_y": {"type": "integer"}
                },
                "required": ["start_x", "start_y", "end_x", "end_y"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "wait",
            "description": "等待指定秒数，用于等待页面加载或动画完成。",
            "parameters": {
                "type": "object",
                "properties": {
                    "seconds": {"type": "number", "default": 2}
                }
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "shell",
            "description": "执行终端命令。打开应用优先用 shell('open -a AppName')，比视觉找图标更快更准。",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "要执行的 shell 命令"}
                },
                "required": ["command"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "finished",
            "description": "任务完成。必须调用此工具表示任务结束。",
            "parameters": {
                "type": "object",
                "properties": {
                    "summary": {"type": "string", "description": "任务完成的总结"}
                },
                "required": ["summary"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "call_user",
            "description": "遇到无法解决的问题（需要密码、验证码等），请求用户帮助。",
            "parameters": {
                "type": "object",
                "properties": {
                    "question": {"type": "string", "description": "需要用户回答的问题"}
                },
                "required": ["question"]
            }
        }
    }
]
```

---

## 四、提示词设计

### 渲染方式
Python 函数拼接 + XML 标签。不用占位符模板。

### System Prompt 构建

```python
def build_system_prompt(config: dict) -> str:
    lang = config.get("language", "zh")
    max_steps = config.get("max_steps", 50)
    
    parts = []
    
    # 身份
    parts.append(
        "你是一个能操作 Mac 电脑的 AI 助手。"
        "你可以看到屏幕截图，并通过工具操作鼠标、键盘和终端。\n"
        f"使用{'中文' if lang == 'zh' else 'English'}思考和回复。"
    )
    
    # 操作规则
    parts.append(
        "<RULES>\n"
        "1. 每次只调用一个工具。调用后会收到新的截图，根据截图决定下一步。\n"
        "2. 操作前先仔细观察截图，确认要点击的位置。描述你看到了什么、打算做什么。\n"
        "3. 每次操作后，仔细对比前后截图，确认操作是否生效。没有生效则分析原因重试或换方式。\n"
        "4. 如果操作后界面没有变化，可能是点错位置、需要等待加载、或需要滚动。\n"
        "5. 能用 shell 命令快速完成的事优先用 shell，如打开应用用 shell('open -a AppName')。\n"
        "6. 输入中文前确认输入法状态，不确定则先用 hotkey 切换。\n"
        "7. 连续 3 次操作没有进展时，停下来重新分析当前状态，尝试完全不同的策略。\n"
        "8. 任务完成后必须调用 finished 工具。\n"
        "9. 遇到无法解决的问题（密码、验证码）调用 call_user，等用户处理后会通知你继续。\n"
        "</RULES>"
    )
    
    parts.append(
        "<CONSTRAINTS>\n"
        f"- 最多执行 {max_steps} 步\n"
        "- 不要执行危险的 shell 命令（rm -rf 等）\n"
        "- 不要访问或泄露密码、密钥等敏感信息\n"
        "</CONSTRAINTS>"
    )
    
    # SOUL.md（Agent 人格，可选）
    soul_path = config.get("soul_path")
    if soul_path:
        from pathlib import Path
        p = Path(soul_path).expanduser()
        if p.exists():
            soul = p.read_text().strip()
            parts.append(f"<PERSONALITY>\n{soul}\n</PERSONALITY>")
    
    return "\n\n".join(parts)
```

### 渲染后示例

```
你是一个能操作 Mac 电脑的 AI 助手。你可以看到屏幕截图，并通过工具操作鼠标、键盘和终端。
使用中文思考和回复。

<RULES>
1. 每次只调用一个工具。调用后会收到新的截图，根据截图决定下一步。
2. 操作前先仔细观察截图，确认要点击的位置。描述你看到了什么、打算做什么。
3. 每次操作后，仔细对比前后截图，确认操作是否生效。没有生效则分析原因重试或换方式。
4. 如果操作后界面没有变化，可能是点错位置、需要等待加载、或需要滚动。
5. 能用 shell 命令快速完成的事优先用 shell，如打开应用用 shell('open -a AppName')。
6. 输入中文前确认输入法状态，不确定则先用 hotkey 切换。
7. 连续 3 次操作没有进展时，停下来重新分析当前状态，尝试完全不同的策略。
8. 任务完成后必须调用 finished 工具。
9. 遇到无法解决的问题（密码、验证码）调用 call_user，等用户处理后会通知你继续。
</RULES>

<CONSTRAINTS>
- 最多执行 50 步
- 不要执行危险的 shell 命令（rm -rf 等）
- 不要访问或泄露密码、密钥等敏感信息
</CONSTRAINTS>
```

注意：tool 列表不在提示词里。通过 API 的 tools 参数传入，所有 OpenAI 兼容供应商都支持。

---

## 五、一次完整交互的 messages 示例

### 第 1 轮

```python
from openai import AsyncOpenAI
import json

client = AsyncOpenAI(base_url=config["llm"]["base_url"], api_key=config["llm"]["api_key"])

messages = [
    {"role": "system", "content": system_prompt},
    {"role": "user", "content": [
        {"type": "text", "text": "帮我在钉钉给张三发一条消息说'明天下午3点开会'"},
        {"type": "image_url", "image_url": {
            "url": f"data:image/png;base64,{screenshot_b64}",
            "detail": "high"
        }}
    ]}
]

response = await client.chat.completions.create(
    model=config["llm"]["model"],
    messages=messages,
    tools=TOOLS,
    max_tokens=4096,
)

msg = response.choices[0].message
# msg.content = "当前是桌面，钉钉未打开。先用命令打开钉钉。"
# msg.tool_calls = [ToolCall(id="call_01", function=Function(name="shell", arguments='{"command":"open -a DingTalk"}'))]
```

### 第 2 轮（执行后追加结果和新截图）

```python
# 追加 assistant 的响应（原样）
messages.append(msg.model_dump())

# 追加 tool 执行结果
messages.append({
    "role": "tool",
    "tool_call_id": "call_01",
    "content": "命令已执行"
})

# 追加新截图
messages.append({
    "role": "user",
    "content": [{"type": "image_url", "image_url": {
        "url": f"data:image/png;base64,{new_screenshot_b64}",
        "detail": "high"
    }}]
})

# 继续调 LLM...
response = await client.chat.completions.create(
    model=config["llm"]["model"],
    messages=messages,
    tools=TOOLS,
    max_tokens=4096,
)
# msg.content = "钉钉已打开。点击顶部搜索框。"
# msg.tool_calls = [ToolCall(name="click", arguments='{"x":200,"y":55}')]
```

### 最后一轮（任务完成）

```python
# LLM 返回 finished tool call
# msg.tool_calls = [ToolCall(name="finished", arguments='{"summary":"已在钉钉给张三发送消息"}')]
# → 循环结束
```

### 滑动窗口（第 8 轮，max_images=5）

```python
# 第 1-3 轮的截图 → 替换为文字
{"role": "user", "content": [{"type": "text", "text": "[Screenshot omitted]"}]}

# 第 4-8 轮的截图 → 保留原图
{"role": "user", "content": [{"type": "image_url", "image_url": {"url": "data:image/png;base64,..."}}]}

# 所有轮次的文字（assistant content + tool result）→ 始终保留
```

---

## 六、源码目录结构

```
see-agent/
├── README.md
├── pyproject.toml                  # uv 依赖管理
├── .env.example                    # 环境变量模板（可选覆盖 config）
├── Makefile                        # 常用命令（make dev, make test）
│
├── src/
│   ├── __init__.py
│   ├── main.py                     # 程序入口
│   ├── config.py                   # 配置加载（~/.see-agent/config.json + 环境变量）
│   │
│   ├── agent/                      # === 核心 Agent 循环 ===
│   │   ├── __init__.py
│   │   ├── loop.py                 # 主循环
│   │   │   # class AgentLoop:
│   │   │   #   async def run(task: str) -> str
│   │   │   #   - 初始截图
│   │   │   #   - while not finished and step < max_steps:
│   │   │   #       response = brain.chat(messages, tools)
│   │   │   #       result = hand.execute(tool_call)
│   │   │   #       screenshot = eye.capture()
│   │   │   #       context.append(response, result, screenshot)
│   │   │   #   - return summary
│   │   │   #   - 通过 on_step 回调通知外部每步进展
│   │   │   #
│   │   │   # on_step 回调签名:
│   │   │   #   async def on_step(event: StepEvent) -> None
│   │   │   #   StepEvent = {
│   │   │   #     step: int,           # 当前步数
│   │   │   #     max_steps: int,      # 最大步数
│   │   │   #     thought: str,        # LLM 的思考文字
│   │   │   #     tool_name: str,      # 调用的工具名
│   │   │   #     tool_args: dict,     # 工具参数
│   │   │   #     tool_result: str,    # 执行结果
│   │   │   #     screenshot_path: str # 截图保存路径
│   │   │   #   }
│   │   │   #   CLI 用它打印实时输出，WebSocket 用它推送给前端
│   │   │
│   │   └── context.py              # 上下文管理（messages 数组 + 滑动窗口）
│   │       # class ConversationContext:
│   │       #   add_user_task(text, screenshot_b64)
│   │       #   add_assistant(message)
│   │       #   add_tool_result(tool_call_id, result, screenshot_b64)
│   │       #   add_screenshot(screenshot_b64) — 单独追加截图
│   │       #   add_user_reply(text) — call_user 后用户的回复
│   │       #   add_system_hint(text) — 插入系统提示（如无进展警告）
│   │       #   get_messages() -> list[dict]  (应用滑动窗口)
│   │
│   ├── brain/                      # === LLM 对接 ===
│   │   ├── __init__.py
│   │   ├── base.py                 # 抽象接口
│   │   │   # class BaseBrain(ABC):
│   │   │   #   async def chat(messages, tools) -> BrainResponse
│   │   │
│   │   ├── openai_client.py        # OpenAI 协议实现（兼容所有供应商）
│   │   │   # class OpenAIBrain(BaseBrain):
│   │   │   #   AsyncOpenAI(base_url=..., api_key=...)
│   │   │   #   原生 tool calling
│   │   │
│   │   └── prompts.py              # 提示词构建（函数拼接 + XML 标签）
│   │       # def build_system_prompt(config) -> str
│   │
│   ├── eye/                        # === 视觉感知 ===
│   │   ├── __init__.py
│   │   ├── base.py                 # 抽象接口
│   │   │   # class BaseEye(ABC):
│   │   │   #   async def capture() -> Screenshot
│   │   │
│   │   └── mac.py                  # macOS 实现
│   │       # class MacEye(BaseEye):
│   │       #   PyAutoGUI 截屏 → Pillow resize(如需) → PNG base64
│   │       #   返回 Screenshot(base64, width, height, scale_factor)
│   │
│   ├── hand/                       # === 操作执行 ===
│   │   ├── __init__.py
│   │   ├── tool.py                 # Tool 基类 + ToolRegistry
│   │   │   # class Tool(ABC):
│   │   │   #   name, description, parameters (抽象属性)
│   │   │   #   async execute(**kwargs) -> str (抽象方法)
│   │   │   #   to_openai_schema() -> dict (自动生成 OpenAI 格式)
│   │   │   #
│   │   │   # class ToolRegistry:
│   │   │   #   register(tool) — 注册
│   │   │   #   get(name) -> Tool — 获取
│   │   │   #   get_openai_schemas() -> list[dict] — 生成 API tools 参数
│   │   │   #   async execute(name, args) -> str — 路由执行
│   │   │   #
│   │   │   # Tool 可通过构造函数接收依赖（如 eye 实例）
│   │   │   # 例: ScreenshotTool(eye=mac_eye)
│   │   │
│   │   └── tools/                  # 具体 Tool 实现（每个文件一个 Tool）
│   │       ├── __init__.py         # 导出 create_registry(eye) -> ToolRegistry
│   │       │   # 创建 registry 并注册所有 tool
│   │       │   # 需要依赖注入的 tool 在这里接收参数
│   │       ├── click.py            # ClickTool: pyautogui.click
│   │       ├── type_text.py        # TypeTextTool: pbcopy+Cmd+V(Mac) / pyperclip(跨平台)
│   │       ├── hotkey.py           # HotkeyTool: pyautogui.hotkey
│   │       ├── scroll.py           # ScrollTool: pyautogui.scroll
│   │       ├── drag.py             # DragTool: pyautogui.moveTo + drag
│   │       ├── shell.py            # ShellTool: asyncio.create_subprocess_shell
│   │       ├── wait.py             # WaitTool: asyncio.sleep
│   │       ├── screenshot.py       # ScreenshotTool: 构造时接收 eye 实例
│   │       ├── finished.py         # FinishedTool: 不实际执行，loop 层检查 name 直接结束
│   │       └── call_user.py        # CallUserTool: 不实际执行，loop 层检查 name 暂停等用户
│   │
│   │       # 扩展新 tool：新建 xxx.py → 实现 Tool 基类 → 在 __init__.py 注册
│   │       # 平台差异在各 tool 内部处理（if platform == "darwin": ...）
│   │
│   ├── server/                     # === API 服务 ===
│   │   ├── __init__.py
│   │   ├── app.py                  # FastAPI 应用
│   │   ├── routes/
│   │   │   ├── __init__.py
│   │   │   ├── chat.py             # POST /api/chat → 提交任务
│   │   │   ├── task.py             # GET /api/task/{id} → 查询状态
│   │   │   ├── ws.py               # WS /api/ws/{task_id} → 实时进度
│   │   │   └── health.py           # GET /api/health
│   │   └── models.py               # Pydantic 数据模型
│   │
│   └── cli/                        # === 命令行界面 ===
│       ├── __init__.py
│       └── main.py
│           # see-agent serve        → 启动 API 服务
│           # see-agent chat         → 交互式对话（CLI 实时输出）
│           # see-agent run "任务"    → 单次执行
│           # see-agent config show  → 查看配置
│           # see-agent config init  → 初始化配置
│
├── workspace/                      # 默认工作空间模板（安装时复制到 ~/.see-agent/）
│   ├── config.json
│   └── SOUL.md
│
└── tests/
    ├── __init__.py
    ├── test_loop.py
    ├── test_brain.py
    ├── test_eye.py
    ├── test_hand.py
    └── test_context.py
```

---

## 七、运行时工作空间

```
~/.see-agent/
├── config.json                     # 用户配置
│   # {
│   #   "llm": {
│   #     "base_url": "https://matrixllm.alipay.com/v1",
│   #     "api_key": "sk-xxx",
│   #     "model": "claude-opus-4-6"
│   #   },
│   #   "language": "zh",
│   #   "max_steps": 50,
│   #   "max_images": 5,
│   #   "screenshot_interval_ms": 500,
│   #   "soul_path": "~/.see-agent/SOUL.md"
│   # }
│
├── SOUL.md                         # Agent 人格（用户可编辑，可选）
│   # 被 system prompt 的 <PERSONALITY> 读取
│   # 不编辑则使用默认人格
│
├── screenshots/                    # 每步截图自动保存（按任务组织）
│   └── task_20260304_200000/
│       ├── step_000.png
│       ├── step_001.png
│       └── ...
│
└── logs/
    └── 2026-03-04.log
```

### 首次运行初始化
1. 检查 `~/.see-agent/` 是否存在
2. 不存在 → 创建目录 + 从模板复制 config.json 和 SOUL.md
3. 检查 config.json 中 api_key
4. 未配置 → CLI 提示用户输入
5. 已配置 → 启动

---

## 八、CLI 交互设计

v0.1 的唯一用户界面。实时输出每步操作，截图自动保存。

```bash
# 启动 API 服务（独立运行，供外部客户端调用）
see-agent serve [--port 8000]

# 交互式对话（直接调用 AgentLoop，不走 HTTP，不启动服务）
see-agent chat

# 进入对话后：
$ see-agent chat
🤖 see-agent v0.1 已启动

> 帮我在钉钉给张三发消息说明天开会

📸 [Step 0] 截屏完成 → screenshots/task_001/step_000.png
💭 当前是桌面，钉钉未打开。先用命令打开钉钉。
🖐️ [Step 1/50] shell: open -a DingTalk
⏳ 等待 500ms...

📸 [Step 1] 截屏完成 → screenshots/task_001/step_001.png
💭 钉钉已打开，看到主界面。点击顶部搜索框。
🖐️ [Step 2/50] click: (200, 55)
⏳ 等待 500ms...

📸 [Step 2] 截屏完成 → screenshots/task_001/step_002.png
💭 搜索框已获得焦点，输入"张三"搜索联系人。
🖐️ [Step 3/50] type_text: "张三"
⏳ 等待 500ms...

...

✅ [Step 8/50] finished: 已在钉钉给张三发送"明天开会"
📁 截图已保存: screenshots/task_001/ (9 张)
⏱️ 总耗时: 23s

> （等待下一个任务，Ctrl+C 退出）

# 单次执行
see-agent run "打开 Safari 搜索今天天气"

# 配置
see-agent config show
see-agent config init
```

### 监督方式
1. **实时 CLI 输出**：每步打印思考过程和操作
2. **截图自动保存**：事后可用 Finder 翻看
3. **Ctrl+C 随时中断**

---

## 九、配置项

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| llm.base_url | string | - | LLM API 地址（OpenAI 兼容） |
| llm.api_key | string | - | API 密钥 |
| llm.model | string | - | 模型名 |
| language | string | "zh" | Agent 思考和回复语言 |
| max_steps | int | 50 | 单次任务最大步数 |
| max_images | int | 5 | 上下文保留截图数（滑动窗口） |
| screenshot_interval_ms | int | 500 | 操作后等待再截图（毫秒） |
| soul_path | string | "~/.see-agent/SOUL.md" | Agent 人格文件 |

---

## 十、依赖

```toml
[project]
name = "see-agent"
version = "0.1.0"
requires-python = ">=3.11"
dependencies = [
    "openai>=1.60.0",
    "fastapi>=0.115.0",
    "uvicorn>=0.34.0",
    "websockets>=14.0",
    "pyautogui>=0.9.54",
    "pillow>=11.0.0",
    "typer>=0.15.0",
    "pydantic>=2.10.0",
    "httpx>=0.28.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=8.0.0",
    "pytest-asyncio>=0.25.0",
    "ruff>=0.8.0",
]

[project.scripts]
see-agent = "src.cli.main:app"
```

---

## 十一、关键实现注意事项

### 1. 中文输入
PyAutoGUI 不支持直接输入中文，用剪贴板：
```python
import subprocess, pyautogui

def type_chinese(text: str):
    process = subprocess.Popen(['pbcopy'], stdin=subprocess.PIPE)
    process.communicate(text.encode('utf-8'))
    pyautogui.hotkey('command', 'v')
```

### 2. macOS 权限
首次运行需授予：
- **辅助功能**（Accessibility）：控制鼠标键盘
- **屏幕录制**（Screen Recording）：截图

### 3. 错误处理与防死循环
```python
MAX_CONSECUTIVE_ERRORS = 3
NO_PROGRESS_LIMIT = 3
error_count = 0
no_progress_count = 0
last_screenshot_hash = None

for step in range(max_steps):
    try:
        response = await brain.chat(context.get_messages(), TOOLS)
        tool_calls = response.choices[0].message.tool_calls

        if not tool_calls:
            break  # 纯文本回复，可能任务结束或需要人工介入

        # 只执行第一个 tool_call（GUI 操作必须串行）
        tc = tool_calls[0]
        name = tc.function.name
        args = json.loads(tc.function.arguments)

        # call_user: 暂停等用户处理，然后继续循环
        if name == "call_user":
            print(f"\U0001f91a Agent 需要帮助: {args['question']}")
            user_reply = input("处理完后输入回复（直接回车=继续）: ")
            context.add_user_reply(user_reply or "已处理，请继续")
            new_screenshot = await eye.capture()
            context.add_screenshot(new_screenshot)
            continue

        result = await registry.execute(name, args)
        error_count = 0

        # 截图并检测是否有进展（截图 hash 比对）
        new_screenshot = await eye.capture()
        current_hash = hash(new_screenshot.base64[:1000])
        if current_hash == last_screenshot_hash:
            no_progress_count += 1
            if no_progress_count >= NO_PROGRESS_LIMIT:
                context.add_system_hint(
                    f"警告：连续 {NO_PROGRESS_LIMIT} 次操作后界面没有变化。"
                    "请重新分析当前状态，尝试完全不同的策略。"
                )
                no_progress_count = 0  # 重置，给 LLM 机会换策略
        else:
            no_progress_count = 0
        last_screenshot_hash = current_hash

    except Exception as e:
        error_count += 1
        if error_count >= MAX_CONSECUTIVE_ERRORS:
            break
        # 错误信息追加到 messages，让 LLM 自行纠正
```

### 4. Tool Call 参数解析
OpenAI 的 tool_calls[].function.arguments 是 JSON 字符串，需要 json.loads：
```python
tc = message.tool_calls[0]
name = tc.function.name                     # "click"
args = json.loads(tc.function.arguments)    # {"x": 200, "y": 55}
```

### 5. PyAutoGUI 安全
```python
import pyautogui
pyautogui.FAILSAFE = True   # 鼠标移到左上角中断（安全机制）
pyautogui.PAUSE = 0.1       # 操作间隔
```

---

## 十二、实现优先级（按顺序）

每步可独立测试。

```
Day 1: 项目骨架
  - pyproject.toml + 目录结构 + config.py
  - ~/.see-agent/ 初始化逻辑

Day 2: eye/mac.py — 视觉感知
  - 截屏 → resize(如需) → PNG base64
  - 测试：截图保存为文件验证

Day 3: hand/tools/ — 操作执行
  - Tool 基类 + ToolRegistry + 全部 10 个 tool
  - 测试：click、type_text("你好")、shell("ls")

Day 4: brain/openai_client.py — LLM 对接
  - OpenAI SDK + 提示词构建
  - 测试：发截图+任务 → 验证返回 tool_calls

Day 5-6: agent/loop.py + context.py — 核心循环
  - 滑动窗口
  - 主循环串联 eye + brain + hand
  - on_step 回调
  - 测试：完成"打开 Finder"

Day 7: cli/main.py — CLI
  - chat 交互模式 + run 单次模式
  - 实时输出 + 截图保存

Day 8-9: server/ — API
  - FastAPI 路由 + WebSocket
  - 测试：curl 调 API

Day 10: 集成测试
  - 端到端：打开钉钉发消息
  - 调优提示词
```

---

## 十三、非功能性要求

- Python type hints 全覆盖
- ruff 格式化 + lint
- 异步优先（async/await）
- logging 分级输出，敏感信息不入日志
- 截图文件按任务 ID 组织，支持手动清理
