# v4 重构 Report — 按 MentalModel.md 对齐源码

> **给 CC 的指令**：这是一次大刀阔斧的重构，不做任何向后兼容。旧代码旧结构直接删。每完成一项跑 `bash scripts/check.sh`。改之前先读 `MentalModel.md`。

---

## 一、删除 workspace/ 层级

**现状**：Agent 的 md 文件同时存在于 `agents/{id}/` 和 `agents/{id}/workspace/` 下，prompts.py 读的是 workspace/ 下的。

**目标**：md 文件只在 `agents/{id}/` 下，不再有 workspace/ 目录。

### 1.1 `see_agent/agent/definition.py` → `AgentDefinition.create()`

删掉以下逻辑：
- 创建 `ws_dir = agent_dir / "workspace"` 的代码
- 复制模板到 workspace/ 的代码
- "Backward compat" 注释块（复制 AGENTS.md/SOUL.md 到 agent root 的代码）

改为：直接把模板复制到 `agent_dir/` 下。模板列表改为 `AGENTS.md`, `SOUL.md`, `IDENTITY.md`（新增），删掉 `TOOLS.md`, `USER.md`, `IDENTITY.md`（只保留 IDENTITY.md）。

等等，重新明确模板列表：只保留 `IDENTITY.md`、`AGENTS.md`、`SOUL.md` 三个模板。删掉 `TOOLS.md`、`USER.md` 模板。`see_agent/templates/` 目录下也删掉对应文件。

### 1.2 `see_agent/brain/prompts.py` → `_inject_workspace()`

- 函数名改为 `_inject_agent_files()`
- 读取路径从 `agent_dir / "workspace"` 改为直接读 `agent_dir/`
- `_WORKSPACE_FILES` 列表改为：`["IDENTITY.md", "AGENTS.md", "SOUL.md"]`
- 额外读取 `agent_dir / "memory" / "MEMORY.md"`（第 4 个注入文件）

### 1.3 `see_agent/server/routes/agents.py`

- `list_workspace_files` / `get_workspace_file` / `update_workspace_file` 三个路由：改为读写 `agent_dir/` 下的 md 文件（不再是 `agent_dir / "workspace"`）
- `get_agent()` 里 `has_soul` 检测：去掉 `workspace/SOUL.md` 分支，只检测 `agent_dir / "SOUL.md"`

### 1.4 `see_agent/templates/`

删掉 `TOOLS.md` 和 `USER.md`。确保有 `IDENTITY.md`（如果没有就创建一个模板）。

---

## 二、配置字段重新分组

**现状**：config.json 字段散在顶层（max_steps、max_images、tool_delay_ms 等）。

**目标**：按 MentalModel.md 分组到 `llm`、`agent`、`screen`、`skills`、`mcp`、`tools`、`sandbox`、`plugins`、`web`、`env`。

### 2.1 `see_agent/config.py`

`DEFAULT_CONFIG` 重写为：

```python
DEFAULT_CONFIG: dict[str, Any] = {
    "llm": {
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o",
    },
    "agent": {
        "max_steps": 50,
        "context_engine": "legacy",
        "compact": {
            "context_window": 200000,
            "target_ratio": 0.75,
            "keep_recent": 8,
            "summary_model": "",
        },
    },
    "screen": {
        "max_images": 5,
        "screenshot_interval_ms": 800,
        "tool_delay_ms": 200,
        "scaling_enabled": True,
        "scaling_match": "aspect_ratio",
        "show_overlay": True,
    },
    "skills": {
        "dirs": ["~/.see-agent/skills"],
        "disabled": [],
    },
    "mcp": {
        "servers": {},
        "disabled": [],
    },
    "tools": {
        "disabled": [],
    },
    "sandbox": {
        "profile": "default",
        "extra_read": [],
        "extra_write": [],
    },
    "plugins": {
        "enabled": True,
        "dirs": ["~/.see-agent/plugins"],
    },
    "web": {
        "language": "zh",
    },
    "env": {},
}
```

删掉 `RUN_DIR` 常量。删掉 `ensure_workspace()` 里创建 `RUN_DIR` 的代码。

### 2.2 全项目配置读取适配

所有读配置的地方都要改路径。**用 grep 找到所有引用，逐个改**：

| 旧路径 | 新路径 |
|--------|--------|
| `config["max_steps"]` | `config["agent"]["max_steps"]` |
| `config["max_images"]` | `config["screen"]["max_images"]` |
| `config["screenshot_interval_ms"]` | `config["screen"]["screenshot_interval_ms"]` |
| `config["tool_delay_ms"]` | `config["screen"]["tool_delay_ms"]` |
| `config["scaling_enabled"]` | `config["screen"]["scaling_enabled"]` |
| `config["scaling_match"]` | `config["screen"]["scaling_match"]` |
| `config["show_overlay"]` | `config["screen"]["show_overlay"]` |
| `config["language"]` | `config["web"]["language"]` |
| `config["context_engine"]` | `config["agent"]["context_engine"]` |
| `config["skills_dirs"]` | `config["skills"]["dirs"]` |
| `config["compact"]` | `config["agent"]["compact"]` |
| `config["compact"]["enabled"]` | 删除，compact 始终开启 |
| `config["mcp_servers"]` | `config["mcp"]["servers"]` |
| `config["memory"]` | 删除，记忆不可配置 |
| `config["env"]` | 不变 |

关键文件（至少这些要改）：
- `see_agent/agent/loop.py`（max_steps、max_images、screenshot_interval_ms、tool_delay_ms、scaling_enabled、scaling_match、compact）
- `see_agent/brain/prompts.py`（language、max_steps）
- `see_agent/server/supervisor.py`（读配置写运行时）
- `see_agent/agent/worker.py`（读配置）
- `see_agent/server/app.py`（启动日志）
- `see_agent/session/store.py`（config_snapshot）

---

## 三、删除 config_overrides + agent.json 瘦身

**现状**：agent.json 有 `config_overrides` 包装层，有 `name`、`role` 字段。

**目标**：agent.json 与 config.json 同构，直接 deep merge。只有 `id` 是独有字段。

### 3.1 `see_agent/agent/definition.py`

`AgentDefinition` dataclass 改为：
- 删掉 `name` 字段
- 删掉 `role` 字段
- 删掉 `config_overrides` 字段
- 保留 `id`
- 保留 `tools_config` → 但改名为 `tools`，结构变为 `{"disabled": [...]}`
- 保留 `skills_config` → 改名为 `skills`
- 保留 `mcp_config` → 改名为 `mcp`
- 保留 `sandbox`

`save_to()`：序列化时只写 `id` + 有值的配置分组字段（tools、skills、mcp、sandbox 等），不再输出 name/role/config_overrides。

`load_from()`：适配新结构。

`create()`：不再接收 `name`、`role` 参数。

### 3.2 `see_agent/config.py` → `load_agent_config()`

现在是从 `agent_data.get("config_overrides", {})` 取覆盖。改为：直接把 agent.json 整体（去掉 `id`）和全局配置 deep merge。

```python
def load_agent_config(agent_id: str) -> dict[str, Any]:
    global_config = load_config()
    agent_json = AGENTS_DIR / agent_id / "agent.json"
    with open(agent_json) as f:
        agent_data = json.load(f)
    # 去掉 id，剩下的直接 merge
    overrides = {k: v for k, v in agent_data.items() if k != "id"}
    return _deep_merge(global_config, overrides)
```

### 3.3 `see_agent/server/routes/agents.py`

- `CreateAgentRequest`：删掉 `name`、`role`、`config_overrides` 字段
- `UpdateAgentRequest`：同上
- `AgentSummary`：删掉 `name`、`role`，显示名从 IDENTITY.md 读取
- `AgentDetail`：同上
- `list_agents()`、`get_agent()`、`create_agent()`、`update_agent()`：适配

### 3.4 `see_agent/server/schemas.py`

`AgentSummary`、`AgentDetail`、`AgentCreateResponse` 删掉 `name`、`role` 字段。

---

## 四、sessions/ → session/（单会话）

**现状**：`agents/{id}/sessions/{session_id}/` 多会话目录。

**目标**：`agents/{id}/session/` 单会话，不再有 session_id 子目录。

### 4.1 `see_agent/session/store.py`

`SessionStore.create()`：不再生成 session_id 子目录，直接在 `session/` 下创建文件。

`SessionStore.load()`：直接读 `session/meta.json`。

`SessionStore.list()`：删除或改为返回单个 session。

`SessionStore.delete()` / `SessionStore.clean()`：适配单目录。

`Session.__post_init__()`：路径固定为 `agent_dir / "session"`。

### 4.2 `see_agent/agent/loop.py`

`AgentLoop.__init__()` 的 `session_root` 参数改为 `session_dir`（直接指向 `agents/{id}/session/`）。

`AgentLoop.run()`：不再创建 session_id 子目录。resume 逻辑改为读取固定的 `session/` 目录。

### 4.3 `see_agent/server/supervisor.py`

`start_agent()`：`config["_session_root"]` 改为 `config["_session_dir"]`，值改为 `agents/{id}/session`。

### 4.4 `see_agent/agent/worker.py`

读取 `config["_session_dir"]` 替代 `config["_session_root"]`。

### 4.5 `see_agent/server/routes/agents.py` → `get_agent_chat()`

不再遍历 sessions 目录找最新，直接读 `agents/{id}/session/messages.jsonl`。

---

## 五、删除 run/ 目录

**现状**：`run/agents/{id}/` 存放 config.json、inbox.jsonl、agent.sock。

**目标**：run/ 整个删除。

### 5.1 `see_agent/config.py`

- 删除 `RUN_DIR` 常量
- `ensure_workspace()` 不再创建 `RUN_DIR`

### 5.2 `see_agent/server/supervisor.py`

- sock 路径改为 `/tmp/see-agent-{agent_id}.sock`
- 运行时 config 不再写文件，Worker 自己读 config.json + agent.json 做 merge（或通过 stdin 传 JSON）
- inbox.jsonl 移到 `agents/{id}/inbox.jsonl`

### 5.3 `see_agent/agent/worker.py`

- 改为自己读配置做 merge，不再从 run/ 读 config.json
- 或者改为从 stdin 读 JSON

### 5.4 `see_agent/server/app.py` → lifespan shutdown

清理 sock 文件改为清理 `/tmp/see-agent-*.sock`。

---

## 六、inbox 移到 agent 目录

**现状**：inbox.jsonl 在 `run/agents/{id}/inbox.jsonl`。

**目标**：`agents/{id}/inbox.jsonl` + `agents/{id}/inbox_cursor.json`。

### 6.1 `see_agent/server/supervisor.py` → `send_to()`

写入路径改为 `AGENTS_DIR / agent_id / "inbox.jsonl"`。

### 6.2 `see_agent/ipc/message.py` → `Message`

删掉 `source` 字段。只保留 `sender`、`content`、`priority`、`metadata`、`timestamp`。

`priority` 的值从 `"normal"` 改为 `"collect"`。

`format_prefix()` 改为 `[{sender}]`（不再包含 source）。

### 6.3 `see_agent/server/message_router.py`

`_classify_source()` 删除。不再区分 source。

`on_user_message()`、`on_agent_message()`：构造 Message 时不传 source。

### 6.4 `see_agent/agent/runtime.py` / `see_agent/agent/loop.py`

消费 inbox 后更新 `inbox_cursor.json`。进程恢复时读 cursor 继续。

---

## 七、compact 始终开启

**现状**：compact 有 `enabled` 开关，默认 false。

**目标**：compact 始终开启，删除 `enabled` 字段。

### 7.1 `see_agent/agent/loop.py` → `_maybe_compact()`

删掉 `if not compact_cfg.get("enabled", False): return` 判断。compact 始终执行检查。

配置路径改为 `config["agent"]["compact"]`。

### 7.2 触发前静默提醒

在 `_maybe_compact()` 执行压缩前，先注入一条 system_hint：

```python
ctx.add_system_hint(
    "[系统提示] 上下文即将达到窗口上限，请立即用 write_memory 保存重要信息，下一轮将执行上下文压缩。"
)
```

然后 return，让 Agent 回复一轮。下一轮再次触发 `_maybe_compact()` 时真正执行压缩。用一个 flag（`_compact_warned`）避免重复提醒。

---

## 八、team.json 适配

**现状**：members 是 string[]，leader 是单独字段。

**目标**：members 改为 `[{id, role}]`，leader 保留。

### 8.1 `see_agent/team/definition.py`

`TeamDefinition` dataclass：
- `members: list[str]` → `members: list[dict[str, str]]`（每项 `{"id": "xxx", "role": "xxx"}`）
- `leader` 保留
- 删掉 `screen_mode`

`save()` / `load()` / `create()` / `list_all()` 适配。

### 8.2 `see_agent/server/routes/team.py`

适配新的 members 结构。

### 8.3 `see_agent/server/schemas.py`

`TeamSummary`、`TeamStatus` 的 `members` 类型改为 `list[dict[str, str]]`。

---

## 九、memory 配置删除

**现状**：config.json 有 `memory` 分组（enabled、search.mode、provider、mem0）。

**目标**：记忆不可配置，代码里硬编码 BM25，删掉所有 memory 配置读取。

### 9.1 `see_agent/config.py`

`DEFAULT_CONFIG` 删掉 `memory` 分组。

### 9.2 全项目

grep `config.*memory` / `config.*mem0`，删掉所有相关配置读取。记忆 backend 直接用 `MarkdownMemoryBackend`，不走配置。

### 9.3 `see_agent/memory/base.py`

如果有 provider 选择逻辑，删掉，只保留 MarkdownMemoryBackend。

---

## 十、清理

### 10.1 删除文件

- `see_agent/templates/TOOLS.md`
- `see_agent/templates/USER.md`

### 10.2 磁盘上的 workspace/ 残留

在 `ensure_workspace()` 或 agent 加载时，检测 `agents/{id}/workspace/` 是否存在：
- 如果有文件且 `agents/{id}/` 下没有同名文件 → 移动过去
- 删除空的 `workspace/` 目录

### 10.3 前端 tools tab 写回 agent.json

`see_agent/server/routes/tools.py`：
- 新增 `GET /api/agents/{agent_id}/tools`：返回工具列表 + 该 agent 的 disabled 状态
- 新增 `PUT /api/agents/{agent_id}/tools`：修改 agent.json 的 `tools.disabled`

### 10.4 system prompt 删除硬编码身份声明

`see_agent/brain/prompts.py` → `build_system_prompt()`：

删掉第一段硬编码身份声明（"你是一个能操作 Mac 电脑的 AI 助手..."）。身份完全由 IDENTITY.md 定义。

### 10.5 约束声明中的 language 判断

`build_system_prompt()` 里根据 `language` 选中英文的逻辑：改为读 `config["web"]["language"]`。或者更好的做法——约束声明不分语言了，统一用中文（基模自适应）。

---

## 执行顺序建议

1. **第二项**（配置重新分组）— 先改这个，因为几乎所有后续改动都依赖新的配置路径
2. **第三项**（删 config_overrides）— 紧跟配置重组
3. **第一项**（删 workspace/）
4. **第四项**（单会话）
5. **第五+六项**（删 run/ + inbox 移动）
6. **第七项**（compact 始终开启）
7. **第八项**（team.json 适配）
8. **第九项**（memory 配置删除）
9. **第十项**（清理）

每完成一项跑 `bash scripts/check.sh`，全过再继续下一项。
