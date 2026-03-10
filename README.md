<div align="center">

<br/>

<img src="https://badgen.net/static/%F0%9F%91%81%EF%B8%8F%20see-agent/Your%20Mac%2C%20on%20Autopilot/purple?scale=2" alt="see-agent" />

<br/><br/>

**An AI agent that sees your screen, remembers your workflow, and operates your Mac — with a pixel-art office to manage your agent team.**

<br/>

<img src="https://badgen.net/static/%F0%9F%8F%86%20Open%20Source/Mac%20AI%20Agent/FFB020" alt="Mac AI Agent"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%96%A5%EF%B8%8F%20Web%20UI/Pixel%20Office/E91E63" alt="Web UI"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A4%9D%20Multi-Agent/Team%20Collaboration/F97316" alt="Multi-Agent Team"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A7%A0%20Memory/Persistent%20%C2%B7%20Cross-Session/8B5CF6" alt="Persistent Memory"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%8C%20Extensible/MCP%20%2B%20Skills/06B6D4" alt="MCP + Skills"/>

<br/><br/>

[![Python](https://badgen.net/static/python/3.11+/3776AB)](https://python.org)
[![License](https://badgen.net/github/license/Xiamu-ssr/see-agent)](LICENSE)
[![Stars](https://badgen.net/github/stars/Xiamu-ssr/see-agent)](https://github.com/Xiamu-ssr/see-agent)
[![Tests](https://badgen.net/static/tests/379%20passed/green)](https://github.com/Xiamu-ssr/see-agent)
[![Built with uv](https://badgen.net/static/built%20with/uv/7C4DFF)](https://docs.astral.sh/uv/)

<br/>

[Quick Start](#-quick-start) · [Features](#-features) · [Web UI](#-web-ui) · [Agent Teams](#-agent-teams) · [Architecture](#%EF%B8%8F-architecture) · [Docs](docs/)

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

**🏢 Pixel Office**
> Manage your agent team through an interactive pixel-art office. See who's working, send messages, track tasks — all in one place.

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

# Configure your LLM
uv run see-agent config init

# Build the web UI
cd web && npm install && npm run build && cd ..

# Launch — opens browser automatically
uv run see-agent start
```

This starts the server and opens `http://localhost:8000` with the full management UI.

## 🏢 Web UI

see-agent ships with a built-in web management interface:

```
┌──────────────────────────────────────────────┐
│ 📮 Teams      │                              │
│ 📊 Dashboard  │   🏢 Pixel Office            │
│ 🤖 Agents     │                              │
│ 🔧 Skills     │   🧑‍💻 leader  👩‍💻 alice  👨‍💻 bob │
│ 🔌 MCP        │                              │
│ ⚙️ Config     │   [Task Board] [💬 Messages] │
│ 📋 Logs       │                              │
└──────────────────────────────────────────────┘
```

- **Teams** — Create and manage agent teams, pixel office view, task board, owner messaging
- **Dashboard** — Global stats at a glance
- **Agents** — CRUD agents, edit SOUL personality, configure tools/skills/MCP per agent
- **Skills & MCP** — Install from ClawhHub or add MCP servers (npm/pip/manual)
- **Config** — JSON Schema-driven form + live JSON preview
- **Logs** — Filterable log viewer with date/level selection

## 🤝 Agent Teams

Define agents with different roles, form teams, and let them collaborate:

```
┌─────────────────────────────────────────────┐
│  Leader: Decomposes task → assigns to team   │
│  Alice:  Opens browser, gathers information  │
│  Bob:    Runs git log, extracts commit data  │
│  Leader: Aggregates results → done           │
└─────────────────────────────────────────────┘
```

Teams are created and managed through the Web UI:
- Drag agents into teams, assign seats in the pixel office
- Leader auto-decomposes tasks, workers claim from the shared task board
- Owner (you) can message any agent or broadcast to the team
- All communication flows through an async message bus with full audit log

## 🏗️ Architecture

```
┌─────────────────────────────────────┐
│  🖥️ Web UI (React + Phaser)         │
│  Pixel Office / Management Panel    │
├─────────────────────────────────────┤
│  🌐 API Layer (FastAPI)             │
│  REST + WebSocket + OpenAPI         │
├─────────────────────────────────────┤
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
| **Web UI** | React 19, TypeScript, Tailwind, shadcn/ui, Phaser 3 (pixel office) |
| **API** | FastAPI, Pydantic response models, OpenAPI spec, WebSocket |
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

Agents inherit global config and can override per-agent:

```jsonc
// ~/.see-agent/agents/alice/agent.json
{
  "name": "Alice", "role": "前端操作员",
  "config_overrides": { "llm": { "model": "claude-sonnet-4-5" }, "max_steps": 30 },
  "tools_config": { "denied": ["shell"] }
}
```

All configuration is editable through the Web UI (Config page → JSON Schema form).

## 📦 Optional Dependencies

```bash
see-agent setup install              # All optional deps
see-agent setup install --memory     # Mem0 vector memory
see-agent setup install --mcp        # MCP protocol support
see-agent setup check                # Verify environment
```

## 📖 CLI Reference

v3 ships with a minimal CLI — all management is done through the Web UI.

| Command | Description |
|---------|-------------|
| `see-agent start` | Start the server and open the browser |
| `see-agent stop` | Stop the running server |
| `see-agent version` | Show version |
| `see-agent config init` | Interactive first-time configuration |
| `see-agent config show` | View current config (API key masked) |
| `see-agent setup install` | Install optional dependencies |
| `see-agent setup check` | Verify environment and dependencies |

## 🛡️ Quality Gates

Every code change must pass `scripts/check.sh`:

```
1. pyright       → Backend type checking
2. ruff          → Backend linting
3. tsc           → Frontend type checking
4. pytest        → 379 backend tests
5. vite build    → Frontend build
6. API contract  → Pydantic ↔ TypeScript type sync
7. API smoke     → Server starts + core endpoints respond
8. CLI smoke     → Basic commands work
```

## 🛡️ Safety

- Screen-only operation — no system-level access by default
- Tool allowlist/denylist per agent
- All actions logged with screenshots for audit
- Shell commands configurable and sandboxable

---

<div align="center">

**Built with** 🐍 Python · ⚛️ React · 🧠 Claude · 🔍 Anthropic Vision · ⚡ uv

<sub>If you find this useful, a ⭐ on GitHub would be awesome!</sub>

</div>
