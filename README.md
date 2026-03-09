<div align="center">

<!-- Replace with your own banner image -->
<!-- <img src="assets/banner.png" width="800" /> -->

<br/>

<img src="https://badgen.net/static/%F0%9F%91%81%EF%B8%8F%20see-agent/Your%20Mac%2C%20on%20Autopilot/purple?scale=2" alt="see-agent" />

<br/><br/>

**An AI agent that sees your screen, remembers your workflow, and operates your Mac.**

<br/>

<!-- Highlight badges -->
<img src="https://badgen.net/static/%F0%9F%8F%86%20Open%20Source/Mac%20AI%20Agent/FFB020" alt="Mac AI Agent"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A7%A0%20Memory/Persistent%20%C2%B7%20Cross-Session/8B5CF6" alt="Persistent Memory"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A4%9D%20Multi-Agent/Team%20Collaboration/F97316" alt="Multi-Agent Team"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%8C%20Extensible/MCP%20%2B%20Plugins/06B6D4" alt="MCP + Plugins"/>

<br/><br/>

<!-- Project stats -->
[![Python](https://badgen.net/static/python/3.11+/3776AB)](https://python.org)
[![License](https://badgen.net/github/license/Xiamu-ssr/see-agent)](LICENSE)
[![Stars](https://badgen.net/github/stars/Xiamu-ssr/see-agent)](https://github.com/Xiamu-ssr/see-agent)
[![Tests](https://badgen.net/static/tests/333%20passed/green)](https://github.com/Xiamu-ssr/see-agent)
[![Built with uv](https://badgen.net/static/built%20with/uv/7C4DFF)](https://docs.astral.sh/uv/)

<br/>

[Quick Start](#-quick-start) · [Features](#-features) · [Agent Teams](#-agent-teams) · [Architecture](#%EF%B8%8F-architecture) · [Docs](docs/)

</div>

---

## ✨ Features

<table>
<tr>
<td width="50%">

**🖥️ Screen-Aware**
> Captures your screen in real-time, understands UI elements, and takes precise actions — clicks, types, scrolls, drags.

**🧠 Persistent Memory**
> Remembers across sessions. File-based (zero deps) or Mem0 vector search. Context compaction keeps conversations infinite.

**🔌 Plugin System**
> Extensible memory backends, context engines, lifecycle hooks, and tool registration. Build your own or use built-in.

</td>
<td width="50%">

**🤝 Agent Teams**
> Multiple agents collaborate on complex tasks. Leader decomposes work, workers execute, everyone communicates via message bus.

**🛠️ MCP + Skills**
> Connect any MCP server (Tavily, GitHub, etc.) and load skills from [ClawhHub](https://clawhub.com). Plug-and-play.

**⚡ Smart & Efficient**
> Anthropic-style coordinate scaling, WebP screenshots, sliding image window, auto-compaction — optimized for long tasks.

</td>
</tr>
</table>

## 🚀 Quick Start

```bash
# Install
git clone https://github.com/Xiamu-ssr/see-agent.git
cd see-agent
uv sync

# Configure
uv run see-agent config init

# Run
uv run see-agent quick chat
```

```
🤖 see-agent v0.1 已启动
  Memory: active
  MCP: active (1 servers)
  Skills: 4 loaded
📋 Session: 20260309_143000_abc123

> 打开钉钉，给张三发消息说明天开会

⏳ [Step 1] screenshot → 分析桌面...
⏳ [Step 2] shell → open -a DingTalk
⏳ [Step 3] click → 搜索框 (234, 89)
⏳ [Step 4] type_text → 张三
...
✅ [Step 8] finished: 已在钉钉给张三发送消息"明天开会"
```

## 🤝 Agent Teams

Define agents with different roles, form teams, and let them collaborate:

```bash
# Create agents
see-agent agent create leader --name "Tech Lead" --role "分解任务、协调进度"
see-agent agent create alice  --name "Alice"     --role "前端操作、UI 交互"
see-agent agent create bob    --name "Bob"        --role "后端操作、数据处理"

# Create a team and run
see-agent team create --name "周报" --members leader,alice,bob --leader leader
see-agent team run <team_id> "帮我写本周周报，从 git log 和邮件中整理"
```

```
┌─────────────────────────────────────────────┐
│  Leader: 分解任务 → 分配给 Alice & Bob        │
│  Alice:  打开邮箱，整理本周邮件摘要             │
│  Bob:    执行 git log，提取提交记录             │
│  Leader: 汇总结果 → 生成周报 → 完成            │
└─────────────────────────────────────────────┘
```

## 🏗️ Architecture

```
┌─────────────────────────────────────┐
│  🔌 Plugin Layer                     │
│  Memory / ContextEngine / Hooks     │
├─────────────────────────────────────┤
│  🤝 Team Layer                       │
│  TeamManager / Bus / TaskBoard      │
├─────────────────────────────────────┤
│  🤖 Agent Layer                      │
│  AgentLoop / Brain / Eye / Hand     │
└─────────────────────────────────────┘
```

| Layer | Components |
|-------|-----------|
| **Agent** | `AgentLoop` (ReAct), `Brain` (LLM), `Eye` (screenshot), `Hand` (tools), `Session`, `Overlay` |
| **Team** | `TeamManager`, `TeamBus` (async messaging), `TaskBoard` (shared tasks), Screen Lock |
| **Plugin** | `BaseMemory` (File/Mem0), `BaseContextEngine`, `HookBus`, `ToolRegistry` |

## 🔧 Configuration

```jsonc
// ~/.see-agent/config.json
{
  "llm": { "base_url": "...", "api_key": "...", "model": "claude-opus-4-6" },
  "memory": { "enabled": true, "provider": "file" },
  "compact": { "enabled": true, "context_window": 128000 },
  "mcp_servers": { "tavily": { "type": "stdio", "command": "npx", "args": ["tavily-mcp@latest"] } }
}
```

Agents inherit global config and override per-agent:

```jsonc
// ~/.see-agent/agents/alice/agent.json
{
  "name": "Alice", "role": "前端操作员",
  "config_overrides": { "llm": { "model": "claude-sonnet-4-5" }, "max_steps": 30 },
  "tools": { "denied": ["shell"] }
}
```

## 📦 Optional Dependencies

```bash
see-agent setup install              # All optional deps
see-agent setup install --memory     # Mem0 vector memory
see-agent setup install --mcp        # MCP protocol support
```

## 📖 CLI Reference

| Command | Description |
|---------|-------------|
| `see-agent quick chat` | Interactive single-agent mode |
| `see-agent quick run "task"` | Single-agent one-shot task |
| `see-agent agent create <id>` | Define a new agent |
| `see-agent agent list` | List all agents |
| `see-agent team create` | Create an agent team |
| `see-agent team run <id> "task"` | Run a team task |
| `see-agent team status <id>` | Check team progress |
| `see-agent config show` | View configuration |
| `see-agent setup check` | Verify dependencies |

## 🛡️ Safety

- Screen-only operation — no system-level access by default
- Tool allowlist/denylist per agent
- All actions logged with screenshots for audit
- Shell commands configurable and sandboxable

---

<div align="center">

**Built with** 🐍 Python · 🧠 Claude · 🔍 Anthropic Vision · ⚡ uv

<sub>If you find this useful, a ⭐ on GitHub would be awesome!</sub>

</div>
