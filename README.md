<div align="center">

<br/>

<img src="https://badgen.net/static/%F0%9F%91%81%EF%B8%8F%20see-agent/Your%20Mac%2C%20on%20Autopilot/purple?scale=2" alt="see-agent" />

<br/><br/>

**AI agents that see your screen and operate your Mac as a team.**

<br/>

<img src="https://badgen.net/static/%F0%9F%96%A5%EF%B8%8F%20Web%20UI/Pixel%20Office/E91E63" alt="Web UI"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A4%9D%20Multi-Agent/Subprocess%20Isolation/F97316" alt="Multi-Agent"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%92%20Sandbox/macOS%20Kernel-Level/10B981" alt="Sandbox"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A7%A0%20Memory/Markdown%20%C2%B7%20Agent-Managed/8B5CF6" alt="Memory"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%8C%20Extensible/MCP%20%2B%20Skills/06B6D4" alt="MCP + Skills"/>

<br/><br/>

[![Python](https://badgen.net/static/python/3.11+/3776AB)](https://python.org)
[![License](https://badgen.net/github/license/Xiamu-ssr/see-agent)](LICENSE)
[![Stars](https://badgen.net/github/stars/Xiamu-ssr/see-agent)](https://github.com/Xiamu-ssr/see-agent)
[![Tests](https://badgen.net/static/tests/368%20passed/green)](https://github.com/Xiamu-ssr/see-agent)
[![Built with uv](https://badgen.net/static/built%20with/uv/7C4DFF)](https://docs.astral.sh/uv/)

<br/>

[Quick Start](#-quick-start) · [How It Works](#-how-it-works) · [Web UI](#-web-ui) · [Agent Teams](#-agent-teams) · [Memory](#-memory) · [Sandbox](#-sandbox-isolation) · [Architecture](#%EF%B8%8F-architecture)

</div>

---

## What is see-agent?

see-agent is an open-source AI agent platform for macOS. Each agent can see your screen (via screenshots), think (via LLM), and act (clicks, types, scrolls, shell commands). Multiple agents form teams to tackle complex tasks — each running in its own sandboxed subprocess, communicating through a message bus, and taking turns using the screen through a lease system.

You manage everything from a web-based pixel office. No terminal needed after installation.

## 🚀 Quick Start

```bash
git clone https://github.com/Xiamu-ssr/see-agent.git
cd see-agent
uv sync              # Install Python deps
see-agent install    # Install optional deps + build frontend
see-agent start      # Start service + open browser
```

The browser opens `http://localhost:8000`. Configure your LLM API key in the Config page and you're ready to go.

## 🤔 How It Works

```
You type a task in the Web UI
         │
         ▼
   ┌── Leader agent ──┐
   │  Sees the screen  │
   │  Decomposes task   │
   │  Assigns to team   │
   └────────┬──────────┘
            │ message bus
     ┌──────┴──────┐
     ▼             ▼
  Worker A      Worker B
  (browser)     (terminal)
     │             │
     ▼             ▼
  clicks, types  runs commands
  scrolls, drags  reads output
     │             │
     └──────┬──────┘
            ▼
   Leader collects results
   Reports back to you
```

Each agent follows a **ReAct loop**: screenshot → think → act → screenshot → think → ... until the task is done. All agents run in separate processes, communicate through Unix Domain Sockets, and share the screen through a lease system.

## 🏢 Web UI

The management interface features a **Phaser 3 pixel-art office** where you can see your agents working at their desks, plus a full management panel:

- **Pixel Office** — Interactive game view with agent sprites, desks, and status indicators
- **Teams** — Create and run teams, task board, owner messaging
- **Agents** — CRUD, SOUL personality, sandbox permissions, memory viewer
- **Skills & MCP** — Install from [ClawhHub](https://clawhub.com) or add MCP servers
- **Config** — JSON Schema-driven form with live editor
- **Logs** — Filterable by date and level

## 🤝 Agent Teams

Define agents with different roles, form teams, and let them collaborate:

```json
// Agent definition
{
  "name": "Alice",
  "role": "前端操作，浏览器交互",
  "config_overrides": { "max_steps": 30 },
  "sandbox": { "enabled": true, "screen_access": true }
}
```

**How teams run:**

1. Leader gets the task, reads the screen, decomposes into subtasks
2. Workers claim subtasks from the shared task board
3. Agents communicate via async message bus
4. Screen access is managed by lease — one agent at a time, 10-minute windows
5. Owner (you) can message any agent or broadcast to the whole team
6. All messages logged for audit

## 🧠 Memory

Agents manage their own memory as **plain Markdown files** — no vector database needed:

```
agents/{id}/
├── AGENTS.md              ← Behavior guidelines (auto-generated template)
├── SOUL.md                ← Personality description
└── memory/
    ├── MEMORY.md          ← Long-term memory (agent curates this)
    ├── 2026-03-10.md      ← Daily journal
    └── 2026-03-09.md
```

Agents get two tools to manage their memory:
- **`memory_search`** — Search across all memory files for relevant context
- **`write_memory`** — Write to daily journal or update long-term memory

The agent decides what's worth remembering. Before context compaction, a **memoryFlush** event prompts the agent to save important information from the current conversation.

## 🔒 Sandbox Isolation

Each agent subprocess is wrapped in macOS `sandbox-exec` — kernel-level, not bypassable:

```
sandbox-exec -f agent-profile.sb python -m see_agent.agent.worker ...
```

**Defaults (no config needed):**
- ✅ Read system runtime, own agent directory, team shared workspace
- ✅ Network access, Python + Node.js toolchains
- ❌ Everything else (deny by default)

**Per-agent toggles:** `screen_access`, `network`, `extra_read`, `extra_write`

**Permission denied?** The Web UI shows violations with one-click "Allow" — takes effect on next agent restart.

Sandbox profiles are layered: [Safehouse](https://github.com/eugene1g/agent-safehouse) base (54 profiles) + see-agent common layer + per-agent dynamic layer.

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────┐
│  🖥️ Web UI (React 19 + Phaser 3 + Tailwind)          │
│  Pixel Office / Management Panel                     │
├──────────────────────────────────────────────────────┤
│  🌐 FastAPI Server (main process, launchd managed)   │
│  REST API + WebSocket + Pydantic response models     │
├──────────────────────────────────────────────────────┤
│  📡 AgentRouter (UDS server in main process)         │
│  Bus relay · TaskBoard proxy · ScreenManager         │
├──────────────────────────────────────────────────────┤
│  🤖 Agent subprocesses (one per agent)               │
│  AgentLoop · Brain (LLM) · Memory tools · Remote IPC │
│  Wrapped in sandbox-exec                             │
└──────────────────────────────────────────────────────┘
```

**Process model:**

```
Main process (launchd)
├── FastAPI server (port 8000)
├── AgentRouter (UDS: ~/.see-agent/run/{team}.sock)
│   ├── TeamBus (message relay)
│   ├── TaskBoard (shared tasks)
│   └── ScreenManager (lease system)
│
├── Agent "leader" ── sandbox-exec ── UDS client ── AgentLoop
├── Agent "alice"  ── sandbox-exec ── UDS client ── AgentLoop
└── Agent "bob"    ── sandbox-exec ── UDS client ── AgentLoop
```

**IPC**: JSON-RPC over Unix Domain Socket. ~0.1ms latency vs 2-5s LLM calls. Zero external deps.

## 🔧 Configuration

All configuration is managed through the Web UI. Config page provides a JSON Schema-driven form with live JSON preview.

```jsonc
// ~/.see-agent/config.json
{
  "llm": { "base_url": "...", "api_key": "...", "model": "claude-opus-4-6" },
  "compact": { "enabled": true, "context_window": 128000 },
  "mcp_servers": {
    "tavily": { "type": "stdio", "command": "npx", "args": ["tavily-mcp@latest"] }
  }
}
```

Agents inherit global config and override per-agent in `agent.json`.

## 📖 CLI

Minimal CLI — everything else is in the Web UI:

| Command | Description |
|---------|-------------|
| `see-agent install` | Install all dependencies (Python + frontend) |
| `see-agent start` | Start as launchd service, open browser |
| `see-agent start -f` | Start in foreground (dev mode) |
| `see-agent stop` | Stop service + kill all agent subprocesses |
| `see-agent restart` | Restart the service |
| `see-agent status` | Show service status |
| `see-agent uninstall` | Remove service (optionally delete data) |
| `see-agent version` | Show version |

## 🛡️ Quality Gates

Every code change runs through `scripts/check.sh`:

```
 1. pyright        → Backend type checking
 2. ruff           → Backend linting
 3. tsc            → Frontend type checking
 4. pytest         → 368 backend tests
 5. vite build     → Frontend build
 6. API contract   → Pydantic ↔ TypeScript type sync
 7. API smoke      → Server starts + core endpoints respond
 8-9. CLI smoke    → version + status work
```

**Frontend-backend type safety**: API response types defined in `schemas.py` → auto-generated as TypeScript via OpenAPI → `tsc` catches field mismatches at compile time.

## 🔮 Roadmap

- [ ] WebSocket real-time push (agent status, messages, tasks)
- [ ] Agent event-driven loop (persistent agents, not one-shot)
- [ ] Config hot-reload without agent restart
- [ ] Multi-machine distributed teams
- [ ] Linux sandbox support (seccomp/AppArmor)

---

<div align="center">

**Built with** 🐍 Python · ⚛️ React · 🎮 Phaser 3 · 🧠 Claude · 🔍 Anthropic Vision · ⚡ uv

<sub>If you find this useful, a ⭐ on GitHub would be awesome!</sub>

</div>
