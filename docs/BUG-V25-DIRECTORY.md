# Bug Report: v2.5 工作目录结构 + 遗留清理

> 生成日期：2026-03-09
> 基于 commit 19459d8 的 Review
> 补充 BUG-V25-REVIEW.md（功能性问题），本文聚焦目录结构和遗留代码

---

## 一、应删未删（PRD 明确要求删除）

### 1. `~/.see-agent/profiles/` 目录 + profiles 代码

**PRD 原文**："删除的目录：`~/.see-agent/profiles/` — agent 定义天然替代了 profiles"

**实际**：
- `config.py` 仍有 `PROFILES_DIR = WORKSPACE_DIR / "profiles"`
- `ensure_workspace()` 仍创建 `PROFILES_DIR.mkdir(exist_ok=True)`
- `load_config(profile=...)` 完整的 profile overlay 逻辑保留
- CLI 多个命令仍有 `--profile` 参数（chat/run/serve/quick_run/quick_chat）
- 只加了一个 `DeprecationWarning`

**修复**：
1. `config.py`：删除 `PROFILES_DIR`，删除 `ensure_workspace()` 中的 `PROFILES_DIR.mkdir`
2. `config.py`：`load_config()` 删除 `profile` 参数和 profile overlay 逻辑
3. `cli/main.py`：所有命令删除 `--profile` 参数
4. `DEFAULT_CONFIG`：删除 `"profile": None`

### 2. `~/.see-agent/sessions/` 全局目录

**PRD 原文**："删除的目录：`~/.see-agent/sessions/` — session 下沉到 team/agent 级"

**实际**：`config.py` 仍有 `SESSIONS_DIR = WORKSPACE_DIR / "sessions"`，`ensure_workspace()` 仍创建它。`quick run/chat` 仍然把 session 写到全局 `sessions/` 下。

**修复**：
- `quick run/chat` 应该自动创建临时单人 team，session 写到 `teams/<tmp_id>/agents/default/sessions/` 下
- 或者保留全局 `sessions/` 作为 quick 模式的存储位置，但 PRD 说删

**建议**：保留全局 `sessions/` 给 quick 模式用（实用性 > PRD 严格性），但在注释中说明。

### 3. `~/.see-agent/memory/` 全局目录

**PRD 原文**："删除的目录：`~/.see-agent/memory/` — memory 下沉到 team/agent 级"

**实际**：`config.py` 仍有 `MEMORY_DIR = WORKSPACE_DIR / "memory"`，`ensure_workspace()` 仍创建它。旧的 qdrant 数据还在里面。

**修复**：
- 删除 `MEMORY_DIR` 常量和 `ensure_workspace()` 中的创建
- `FileMemory.__init__` 的默认路径改为从 agent/team 上下文传入，不用全局常量

### 4. `~/.see-agent/SOUL.md` 全局文件

**PRD 原文**："删除的目录：`~/.see-agent/SOUL.md` — 不再有全局 SOUL，每个 agent 有自己的"

**实际**：文件还在。`config.json` 里 `"soul_path": "~/.see-agent/SOUL.md"` 仍指向它。

**修复**：
- `DEFAULT_CONFIG` 中删除 `"soul_path"` 或改为 `null`
- 每个 agent 的 soul_path 由 `agents/<id>/SOUL.md` 自动解析，不需要全局配置

### 5. `mem0_backend.py` 未移出

**PRD 原文**："删除文件：`memory/mem0_backend.py` — 移出为可选插件"

**实际**：文件还在源码中。

**修复**：可以保留文件但确保它只在 `pip install see-agent[memory]` 时可用。当前已经做到了（import 失败 graceful），所以这个**不紧急**，保留也行。

---

## 二、应建未建（PRD 要求的 team 下目录结构）

### PRD 定义的完整 team 目录

```
teams/<team_id>/
├── team.json               ✅ 已有
├── tasks.json              ✅ 已有
├── messages.jsonl          ❌ 缺失
├── shared/                 ❌ 缺失
└── agents/
    └── <agent_id>/
        ├── workspace/      ❌ 缺失
        ├── sessions/       ✅ 已有
        ├── memory/         ❌ 缺失
        └── logs/           ❌ 缺失
```

### 6. `teams/<id>/messages.jsonl` 未创建

**原因**：TeamBus 的 `_log()` 方法会写这个文件，但由于 Bus→Agent 桥接断了（BUG-V25-REVIEW Bug 1），没有消息流过，文件没被创建。

**修复**：Bug 1 修复后自然会创建。也可以在 TeamManager 初始化时 `touch` 这个文件。

### 7. `teams/<id>/shared/` 未创建

**用途**：team 级共享产出物目录，agent 把重要文件写到这里供其他 agent 访问。

**修复**：`TeamManager.__init__` 或 `TeamDefinition.create` 时创建：
```python
(self._team_dir / "shared").mkdir(exist_ok=True)
```

### 8. `teams/<id>/agents/<aid>/workspace/` 未创建

**用途**：agent 的私有工作目录。

**修复**：`TeamManager._build_agent_loop` 里创建：
```python
agent_dir = self._team_dir / "agents" / agent_id
(agent_dir / "workspace").mkdir(parents=True, exist_ok=True)
```

### 9. `teams/<id>/agents/<aid>/memory/` 未创建

**用途**：agent 在本 team 内的记忆存储。FileMemory 应该指向这里。

**修复**：
1. 创建目录：`(agent_dir / "memory").mkdir(exist_ok=True)`
2. 构建 memory 时传入这个路径：
```python
memory = FileMemory(memory_dir=agent_dir / "memory")
```

### 10. `teams/<id>/agents/<aid>/logs/` 未创建

**用途**：agent 级运行日志（区别于全局日志）。

**修复**：创建目录，session.log 已经在 session 目录下了，这里可以放 agent 级聚合日志。

---

## 三、Agent 创建不完整

### 11. `see-agent agent create` 缺少配置参数

**现状**：只接受 `--name` 和 `--role`，生成的 `agent.json` 只有 3 个字段：

```json
{"id": "xxx", "name": "Dev Leader", "role": "Leader"}
```

**PRD 定义的完整 agent.json**：

```json
{
  "name": "Alice",
  "role": "前端操作员",
  "llm": { "model": "claude-sonnet-4-5" },
  "max_steps": 30,
  "tools": { "denied": ["shell"] },
  "skills": { "disabled": ["coding-agent"] },
  "mcp_servers": { "enabled": ["tavily"] },
  "memory": { "provider": "file" }
}
```

**修复**：CLI 不需要一次性支持所有字段（太多参数不友好），但至少支持：
- `--model` → 覆盖 LLM 模型
- `--deny-tools` → tools.denied 列表
- `--max-steps` → 覆盖 max_steps

其余可以通过 `see-agent agent edit <id>` 手动编辑 agent.json。

---

## 四、旧命令未清理

### 12. `see-agent chat` 和 `see-agent run` 与 `quick` 版本重复

**PRD 原文**：
- `see-agent chat` → 替代为 `see-agent quick chat`
- `see-agent run` → 替代为 `see-agent quick run`

**实际**：两套命令并存。旧的 `chat`/`run` 还在，且旧的 `chat` 有 stdin reader 而新的 `quick chat` 没有。

**修复**：删除旧的 `chat`/`run` 命令（或改为 alias 到 quick 版本）。把旧 `chat` 的 stdin reader 逻辑移到 `quick chat` 里。

---

## 执行优先级

| # | 问题 | 优先级 | 改动量 |
|---|------|--------|--------|
| 7 | shared/ 目录 | ⚠️ P1 | 1 行 |
| 8 | workspace/ 目录 | ⚠️ P1 | 1 行 |
| 9 | memory/ 目录 + FileMemory 路径 | ⚠️ P1 | ~10 行 |
| 10 | logs/ 目录 | 📝 P2 | 1 行 |
| 6 | messages.jsonl | 📝 P2 | Bug 1 修后自动解决 |
| 1 | 删 profiles | ⚠️ P1 | ~40 行删除 |
| 2 | 全局 sessions 处理 | 📝 P2 | 看决策 |
| 3 | 删全局 memory 常量 | 📝 P2 | ~5 行 |
| 4 | 删全局 SOUL.md 引用 | 📝 P2 | ~3 行 |
| 5 | mem0 保留 | 📝 P3 | 不改 |
| 11 | agent create 参数 | 📝 P2 | ~15 行 |
| 12 | 删旧命令 | ⚠️ P1 | ~50 行删除 |

**建议执行顺序**：先修 BUG-V25-REVIEW.md 的 3 个 P0 → 然后本文的 P1（目录创建 + 删 profiles + 删旧命令）→ 最后 P2/P3。

做完跑 `scripts/check.sh` 确保全过。
