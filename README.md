<div align="center">

<br/>

<img src="https://badgen.net/static/%F0%9F%A4%96%20see-agent-corp/AI%20Agent%20Teams%20for%20macOS/purple?scale=2" alt="see-agent-corp" />

<br/><br/>

**AI agents that see your screen and operate your Mac as a team.**

<br/>

<img src="https://badgen.net/static/%F0%9F%96%A5%EF%B8%8F%20Web%20UI/Single%20Binary/E91E63" alt="Web UI"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A4%9D%20Multi-Agent/Subprocess%20Isolation/F97316" alt="Multi-Agent"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%92%20Sandbox/macOS%20Kernel-Level/10B981" alt="Sandbox"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%A7%A0%20Memory/Markdown%20%C2%B7%20Agent-Managed/8B5CF6" alt="Memory"/>
&nbsp;&nbsp;
<img src="https://badgen.net/static/%F0%9F%94%8C%20Extensible/MCP%20%2B%20Skills/06B6D4" alt="MCP + Skills"/>

<br/><br/>

[![Rust](https://badgen.net/static/rust/2024/DEA584)](https://rust-lang.org)
[![License](https://badgen.net/github/license/Xiamu-ssr/see-agent)](LICENSE)
[![Stars](https://badgen.net/github/stars/Xiamu-ssr/see-agent)](https://github.com/Xiamu-ssr/see-agent)
[![Tests](https://badgen.net/static/tests/275%20passed/green)](https://github.com/Xiamu-ssr/see-agent)
[![CI](https://github.com/Xiamu-ssr/see-agent/actions/workflows/ci.yml/badge.svg)](https://github.com/Xiamu-ssr/see-agent/actions/workflows/ci.yml)

<br/>

[Quick Start](#-quick-start) · [How It Works](#-how-it-works) · [CLI](#-cli) · [Agent Teams](#-agent-teams) · [Memory](#-memory) · [Sandbox](#-sandbox) · [Architecture](#%EF%B8%8F-architecture) · [Development](#-development)

</div>

---

## What is see-agent-corp?

see-agent-corp is an open-source AI agent platform for macOS, written entirely in Rust. Each agent can **see** your screen (via screenshots), **think** (via LLM), and **act** (clicks, types, scrolls, shell commands). Multiple agents form teams to tackle complex tasks — each running in its own sandboxed subprocess, communicating through a file-based message bus.

Everything ships as a **single binary** with the web UI embedded. No Python, no Node.js, no runtime dependencies.

## Quick Start

**From release (recommended):**

```bash
curl -fsSL https://raw.githubusercontent.com/Xiamu-ssr/see-agent/main/scripts/install.sh | sh
see-agent-corp init
see-agent-corp start
# Open http://localhost:28789
```

**From source:**

```bash
git clone https://github.com/Xiamu-ssr/see-agent.git
cd see-agent
cargo build -p see-agent-corp-app --release
./target/release/see-agent-corp init
./target/release/see-agent-corp start
# Open http://localhost:28789
```

Configure your LLM API key in the Config page (or `~/.see-agent-corp/config.json`) and you're ready to go.

## How It Works

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

Each agent follows a **ReAct loop**: screenshot → think → act → screenshot → think → ... until the task is done or the step limit is reached. Safety detectors (repeat action, no progress, no screenshot, consecutive errors) automatically intervene when the agent gets stuck.

**Process lifecycle:** Sending a message to an agent automatically starts it — no manual start/stop needed. Idle agents sleep and wake on demand. The dashboard provides **freeze** (stop all) and **revive** (restart all) controls.

## CLI

Single binary, all commands:

| Command | Description |
|---------|-------------|
| `see-agent-corp init` | Initialize workspace (`~/.see-agent-corp/`) |
| `see-agent-corp start [--port N]` | Start as background daemon |
| `see-agent-corp stop` | Stop the daemon |
| `see-agent-corp restart [--port N]` | Restart the daemon |
| `see-agent-corp status` | Show workspace + server status |
| `see-agent-corp serve [--port N]` | Foreground mode (dev/debug) |
| `see-agent-corp agent create --id NAME` | Create an agent |
| `see-agent-corp agent list` | List all agents |
| `see-agent-corp agent delete NAME` | Delete an agent |
| `see-agent-corp team create --id NAME --members a,b` | Create a team |
| `see-agent-corp team list` | List all teams |
| `see-agent-corp send --agent NAME --message "..."` | Send a task (auto-starts agent) |
| `see-agent-corp config show` | Show merged config |
| `see-agent-corp config set llm.model gpt-4o` | Set a config value |

## Agent Teams

Define agents with different roles and form teams:

```json
{
  "name": "Alice",
  "role": "Frontend operation, browser interaction",
  "config_overrides": { "agent": { "max_steps": 30 } },
  "sandbox": { "extra_write": ["/tmp/workspace"] }
}
```

**How teams run:**

1. Leader receives the task, reads the screen, decomposes into subtasks
2. Leader creates and assigns tasks on the shared task board
3. Workers claim or receive tasks, execute them independently
4. Agents communicate via async message bus (file-based inbox)
5. Team members share a `shared/` workspace directory for file exchange
6. All messages are logged and visible in the web UI (single-page team view)

**Config hierarchy:** Settings cascade through three levels — `config.json` (global) → `team.json` (team-level) → `agent.json` (per-agent). Each level deep-merges on top of the previous. Environment variables (`SAC_*`) override everything.

## Memory

Agents manage their own memory as **plain Markdown files** — no vector database needed:

```
~/.see-agent-corp/agents/{id}/
├── AGENTS.md          ← Behavior guidelines (auto-generated)
├── SOUL.md            ← Personality description
└── memory/
    ├── MEMORY.md      ← Long-term memory (agent curates this)
    ├── 2026-03-18.md  ← Daily journal
    └── ...
```

Two built-in tools:
- **`memory_search`** — BM25 search across all memory files (CJK bigram + ASCII tokenizer)
- **`write_memory`** — Write to daily journal or update long-term memory

Before context compaction, a **memoryFlush** event prompts the agent to save important information from the current conversation.

## Sandbox

Each agent subprocess is wrapped in macOS `sandbox-exec` — kernel-level, not bypassable:

```
sandbox-exec -f agent-profile.sb see-agent-corp worker <agent_id> <workspace>
```

**Defaults:**
- Read: system runtime, own agent directory, team shared workspace
- Write: own agent directory only
- Network: allowed
- Everything else: deny by default

**Per-agent overrides:** `extra_read`, `extra_write` paths in agent config. Sandbox violations are logged — the web UI shows denials with actionable context.

## Architecture

```
┌───────────────────────────────────────────────────────────┐
│  🖥️  Web UI (Leptos 0.7 CSR + Thaw UI 0.4, WASM)         │
│  Dashboard / Agents / Teams / Config / Logs               │
├───────────────────────────────────────────────────────────┤
│  🌐 HTTP Server (Axum 0.8)                                │
│  30+ REST endpoints, static file serving, freeze/revive   │
├───────────────────────────────────────────────────────────┤
│  📡 Supervisor                                            │
│  Worker lifecycle · Auto-start · Heartbeat · Signal relay  │
├───────────────────────────────────────────────────────────┤
│  🤖 Agent worker subprocesses (one per agent)             │
│  AgentLoop · Brain (LLM) · Eye (screen) · Memory          │
│  MCP client · Skill loader · Sandbox wrapper               │
└───────────────────────────────────────────────────────────┘
```

**Key design choices:**

- **Single binary**: Rust + WASM frontend embedded via `rust-embed`. No runtime deps.
- **File-based IPC**: Agents communicate through JSONL inbox files. Simple, auditable, survives crashes.
- **3-level config**: `config.json` → `team.json` → `agent.json` deep merge, plus `SAC_*` env var overrides.
- **LLM agnostic**: Any OpenAI-compatible API (GPT-4o, Claude via proxy, local models).
- **Auto-start lifecycle**: Sending a message starts the agent; idle agents sleep; dashboard freeze/revive for batch control.
- **Lazy image loading**: Screenshots stored as path references, resolved to base64 only at LLM call time.
- **Zero warnings**: `clippy -D warnings` on both native and WASM targets. 275 tests across 3 crates.

## Configuration

```jsonc
// ~/.see-agent-corp/config.json
{
  "llm": {
    "base_url": "https://api.openai.com/v1",
    "api_key": "sk-...",
    "model": "gpt-4o"
  },
  "agent": {
    "max_steps": 50,
    "max_images": 5
  },
  "mcp": {
    "servers": {
      "tavily": {
        "command": "npx",
        "args": ["tavily-mcp@latest"],
        "env": { "TAVILY_API_KEY": "${TAVILY_API_KEY}" }
      }
    }
  }
}
```

Per-agent overrides in `~/.see-agent-corp/agents/{id}/agent.json` and team-level overrides in `team.json` are deep-merged on top of global config.

## Development

**Prerequisites:** Rust (stable), [Trunk](https://trunkrs.dev/) (for WASM frontend)

```bash
# Quality gate (runs all checks)
bash scripts/check.sh

# Pipeline: trunk build → clippy (native) → clippy (wasm) → cargo test → build → magic value scan
```

**Project structure:**

```
see-agent-corp/       ← Core library (types, agent loop, brain, eye, memory, mcp, sandbox, ...)
see-agent-corp-app/   ← Binary crate (CLI, HTTP server, worker, daemon)
see-agent-corp-web/   ← Frontend (Leptos 0.7 CSR + Thaw UI 0.4 → WASM)
scripts/              ← check.sh, install.sh
```

**Conventions:**
- All constants live in `see-agent-corp/src/consts.rs`. No magic values outside the isolation zone (`see-agent-corp/src/io/`).
- TDD: write test → red → implement → green → refactor.
- `cargo test` for inner loop, `check.sh` for final gate before commit.

## Roadmap

- [ ] WebSocket real-time push (agent status, messages, tasks)
- [ ] Agent event-driven loop (persistent agents, not one-shot)
- [ ] Config hot-reload without agent restart
- [ ] Multi-machine distributed teams (RemoteTransport)
- [ ] Linux sandbox support (seccomp/AppArmor)
- [ ] Linux screen capture (Wayland/X11)

---

<div align="center">

**Built with** 🦀 Rust · 🖥️ Leptos · 🎨 Thaw UI · 🧠 LLM Vision · ⚡ Axum · 🔒 macOS Sandbox

<sub>If you find this useful, a star on GitHub would be awesome!</sub>

</div>
