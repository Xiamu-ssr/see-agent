<div align="center">

<br/>

<img src="https://badgen.net/static/%F0%9F%91%81%EF%B8%8F%20see-agent/Your%20Mac%2C%20on%20Autopilot/purple?scale=2" alt="see-agent" />

<br/><br/>

**AI agents that see your screen, remember your workflow, and operate your Mac as a team.**

<br/>

<img src="https://badgen.net/static/%F0%9F%96%A5%EF%B8%8F%20Web%20UI/Pixel%20Office/E91E63" alt="Web UI"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A4%9D%20Multi-Agent/Subprocess%20Isolation/F97316" alt="Multi-Agent"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%92%20Sandbox/macOS%20Kernel-Level/10B981" alt="Sandbox"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A7%A0%20Memory/Persistent%20Cross-Session/8B5CF6" alt="Memory"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%8C%20Extensible/MCP%20%2B%20Skills/06B6D4" alt="MCP + Skills"/>

<br/><br/>

[![Python](https://badgen.net/static/python/3.11+/3776AB)](https://python.org)
[![License](https://badgen.net/github/license/Xiamu-ssr/see-agent)](LICENSE)
[![Stars](https://badgen.net/github/stars/Xiamu-ssr/see-agent)](https://github.com/Xiamu-ssr/see-agent)
[![Tests](https://badgen.net/static/tests/384%20passed/green)](https://github.com/Xiamu-ssr/see-agent)
[![Built with uv](https://badgen.net/static/built%20with/uv/7C4DFF)](https://docs.astral.sh/uv/)

<br/>

[Quick Start](#-quick-start) · [How It Works](#-how-it-works) · [Web UI](#-web-ui) · [Agent Teams](#-agent-teams) · [Sandbox](#-sandbox-isolation) · [Architecture](#%EF%B8%8F-architecture)

</div>

---

## What is see-agent?

see-agent is an open-source AI agent platform for macOS. Each agent can see your screen (via screenshots), think (via LLM), and act (clicks, types, scrolls, shell commands). Multiple agents form teams to tackle complex tasks — each running in its own sandboxed subprocess, communicating through a message bus, and taking turns using the screen through a lease system.

You manage everything from a web-based pixel office UI. No terminal needed after installation.

## ✨ Highlights

- **Multi-agent teams** — leader decomposes tasks, workers execute in parallel, async message bus for coordination
- **Process isolation** — each agent runs in an independent subprocess, one crash won't take down others
- **Kernel-level sandbox** — macOS `sandbox-exec` restricts each agent's file/network access. Dynamic permissions managed via UI
- **Screen lease system** — agents take turns using the screen with 10-minute leases, no more switching conflicts
- **Persistent memory** — file-based (zero deps) or Mem0 vector search, survives across sessions
- **MCP + Skills** — connect any MCP server, install skills from [ClawhHub](https://clawhub.com)
- **Web management UI** — pixel-art office, team management, config editor, log viewer, all in the browser
- **launchd service** — runs in background, auto-restarts on crash, doesn't occupy a terminal

## 🚀 Quick Start

```bash
# Clone and install
git clone https://github.com/Xiamu-ssr/see-agent.git
cd see-agent
uv sync                                        # Python deps
uv run see-agent install                       # Optional deps (memory, MCP, etc.)

# Build the web UI
cd web && npm install && npm run build && cd ..

# Launch (registers as launchd service, opens browser)
uv run see-agent start
```

That's it. The browser opens `http://localhost:8000`. Configure your LLM API key in the Config page and you're ready to go.

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

```
┌──────────────────────────────────────────────────┐
│                                                  │
│  🏢 Pixel Office                                  │
│                                                  │
│  🧑‍💻 leader [working]  👩‍💻 alice [idle]  👨‍💻 bob [idle] │
│                                                  │
│  ┌─ Task Board ──────┐  ┌─ Messages ──────────┐  │
│  │ ☑ Parse emails     │  │ leader: 任务分好了    │  │
│  │ ☐ Write report     │  │ alice: 收到，开始     │  │
│  │ ☐ Send summary     │  │ bob: 邮件解析完了     │  │
│  └───────────────────┘  └─────────────────────┘  │
│                                                  │
├────────────┬─────────────────────────────────────┤
│ 📮 Teams    │  Create / run / monitor teams       │
│ 📊 Dashboard│  Global stats at a glance           │
│ 🤖 Agents   │  CRUD, SOUL, sandbox permissions    │
│ 🔧 Skills   │  Install from ClawhHub              │
│ 🔌 MCP      │  Add MCP servers (npm/pip/manual)   │
│ ⚙️ Config    │  Schema-driven form + JSON editor   │
│ 📋 Logs     │  Filterable by date and level        │
└─────────────┴─────────────────────────────────────┘
```

## 🤝 Agent Teams

Define agents with different roles, form teams, and let them collaborate:

```json
// ~/.see-agent/agents/alice/agent.json
{
  "name": "Alice",
  "role": "前端操作，浏览器交互",
  "config_overrides": { "max_steps": 30 },
  "sandbox": { "enabled": true, "screen_access": true }
}
```

```json
// ~/.see-agent/teams/{id}/team.json
{
  "name": "Weekly Report",
  "leader": "leader",
  "members": ["leader", "alice", "bob"],
  "owner": { "display": "You" }
}
```

**How teams run:**

1. Leader gets the task, reads the screen, decomposes into subtasks
2. Workers claim subtasks from the shared task board
3. Agents communicate via async message bus (not polling, not shared memory)
4. Screen access is managed by lease — one agent at a time, 10-minute windows
5. Owner (you) can message any agent or broadcast to the whole team
6. All messages logged to `messages.jsonl` for audit

## 🔒 Sandbox Isolation

Each agent subprocess is wrapped in macOS `sandbox-exec` — kernel-level, not bypassable:

```
sandbox-exec -f agent-profile.sb python -m see_agent.agent.worker ...
```

**Default permissions (no config needed):**
- ✅ Read system runtime (`/usr`, `/bin`, `/System`)
- ✅ Read/write own agent directory
- ✅ Read/write team shared workspace
- ✅ Network access (for LLM API calls)
- ✅ Python + Node.js toolchains
- ❌ Everything else (deny by default)

**What you can toggle per agent:**

| Permission | Default | Effect |
|-----------|---------|--------|
| `screen_access` | `true` | Can use screenshot/click/type tools |
| `network` | `true` | Can make HTTP requests |
| `extra_read` | `[]` | Additional read-only paths |
| `extra_write` | `[]` | Additional read-write paths |

**Permission denied?** The Web UI shows sandbox violations with one-click "Allow" — writes to `agent.json`, takes effect on next agent restart.

Sandbox profiles are layered: [Safehouse](https://github.com/eugene1g/agent-safehouse) base (54 profiles) + see-agent common layer + per-agent dynamic layer.

## 🏗️ Architecture

```
┌────────────────────────────────────────────────────────┐
│  🖥️ Web UI (React 19 + TypeScript + Tailwind)          │
│  Pixel Office / Management Panel                       │
├────────────────────────────────────────────────────────┤
│  🌐 FastAPI Server (main process)                      │
│  REST API + Pydantic response models + OpenAPI spec    │
├────────────────────────────────────────────────────────┤
│  📡 AgentRouter (UDS server in main process)           │
│  Bus relay · TaskBoard proxy · ScreenManager           │
├────────────────────────────────────────────────────────┤
│  🤖 Agent subprocesses (one per agent)                 │
│  AgentLoop · Brain (LLM) · Remote tools (via UDS)     │
│  Wrapped in sandbox-exec                               │
└────────────────────────────────────────────────────────┘
```

**Process model:**

```
Main process (launchd managed)
├── FastAPI server (port 8000)
├── AgentRouter (UDS: ~/.see-agent/run/{team}.sock)
│   ├── TeamBus (asyncio.Queue per agent)
│   ├── TaskBoard (tasks.json)
│   └── ScreenManager (lease system)
│
├── Agent "leader" subprocess
│   ├── sandbox-exec wrapper
│   ├── UDS client → AgentRouter
│   └── AgentLoop (independent event loop)
│
├── Agent "alice" subprocess
│   └── ...
│
└── Agent "bob" subprocess
    └── ...
```

**IPC**: JSON-RPC over Unix Domain Socket. ~0.1ms latency (vs 2-5s LLM calls). Standard library, zero external deps.

## 🔧 Configuration

All configuration is managed through the Web UI. The config page provides a JSON Schema-driven form with live JSON preview.

```jsonc
// ~/.see-agent/config.json
{
  "llm": { "base_url": "...", "api_key": "...", "model": "claude-opus-4-6" },
  "memory": { "enabled": true, "provider": "file" },
  "compact": { "enabled": true, "context_window": 128000 },
  "mcp_servers": {
    "tavily": { "type": "stdio", "command": "npx", "args": ["tavily-mcp@latest"] }
  }
}
```

Agents inherit global config and override per-agent in `agent.json`.

## 📖 CLI

v3.1 CLI is minimal — everything else is in the Web UI:

| Command | Description |
|---------|-------------|
| `see-agent install` | Install all dependencies |
| `see-agent start` | Start as launchd service, open browser |
| `see-agent start -f` | Start in foreground (dev mode) |
| `see-agent stop` | Stop service + kill all agent subprocesses |
| `see-agent restart` | Restart the service |
| `see-agent status` | Show service status |
| `see-agent uninstall` | Remove service (optionally delete data) |
| `see-agent version` | Show version |

## 🛡️ Quality Gates

Every code change runs through `scripts/check.sh` (9 steps):

```
 1. pyright        → Backend type checking
 2. ruff           → Backend linting
 3. tsc            → Frontend type checking
 4. pytest         → 384 backend tests (isolated to tmp dirs)
 5. vite build     → Frontend build
 6. API contract   → Pydantic schemas ↔ TypeScript types sync
 7. API smoke      → Server starts + core endpoints respond
 8. CLI version    → Basic CLI works
 9. CLI status     → Service management works
```

**Frontend-backend type safety**: All API response types defined in `schemas.py` (Pydantic) → auto-generated as TypeScript types via OpenAPI spec → `tsc` catches field mismatches at compile time.

## 🔮 Roadmap

- [ ] Phaser 3 pixel office with sprite animations
- [ ] Per-agent screen recording and replay
- [ ] Multi-machine distributed agent teams
- [ ] Linux sandbox support (seccomp/AppArmor)

---

<div align="center">

**Built with** 🐍 Python · ⚛️ React · 🧠 Claude · 🔍 Anthropic Vision · ⚡ uv

<sub>If you find this useful, a ⭐ on GitHub would be awesome!</sub>

</div>
