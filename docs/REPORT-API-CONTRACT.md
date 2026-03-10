# Report: 前后端 API 契约强制绑定 + 测试防护体系

> 日期：2026-03-10 | 作者：蓝莓🫐
> 目标：改后端字段 → 前端编译报错，强制同步；测试不污染真实数据；check.sh 一键全检

本 report 包含两部分：
- **Part A**：前后端 API 契约强制绑定（schemas.py → OpenAPI → TS 类型）
- **Part B**：测试防护体系补强（环境隔离 + check.sh + trailing slash 修复）

---

# Part A：前后端 API 契约强制绑定

## 问题

当前所有 API 路由返回 `dict[str, Any]`，导致：
- FastAPI 生成的 OpenAPI spec 里没有字段信息
- 前端 TS 类型是手写的（`web/src/types/agent.ts`），和后端没有绑定
- CC 改了后端返回字段名 → 前端不报错 → 运行时才炸

## 方案

```
schemas.py（唯一事实源）
    │
    ├─→ pyright 检查后端代码（字段拼错立刻报）
    │
    ├─→ FastAPI 自动导出 OpenAPI spec
    │
    ├─→ openapi-typescript 生成 api.d.ts（全量覆盖，不会残留）
    │
    └─→ tsc 检查前端代码（字段名不对立刻报）
```

全程静态检查，不需要启动服务，不需要运行时 API 调用。

## Step 1：后端定义 Response Model

新建 `see_agent/server/schemas.py`（API 响应模型，和 `see_agent/schemas/` 配置表单 schema 是两回事）：

```python
"""API response models — 前后端契约的唯一事实源。

所有 API 路由的返回类型必须使用这里定义的 model。
前端的 TypeScript 类型从这些 model 自动生成。
不要在路由里返回 dict[str, Any]。
"""
from __future__ import annotations
from pydantic import BaseModel


# ── Agents ──

class AgentSummary(BaseModel):
    id: str
    name: str
    role: str
    team_id: str | None = None
    team_name: str | None = None
    status: str = "idle"

class AgentDetail(AgentSummary):
    config_overrides: dict = {}
    tools_config: dict = {}
    skills_config: dict = {}
    mcp_config: dict = {}
    has_soul: bool = False
    location: str = ""

class AgentCreateResponse(BaseModel):
    id: str
    name: str
    role: str


# ── Teams ──

class TeamSummary(BaseModel):
    id: str
    name: str
    members: list[str]
    status: str

class TeamDetail(TeamSummary):
    leader: str | None = None
    seating: dict[str, int] = {}
    created_at: str = ""


# ── Dashboard ──

class DashboardResponse(BaseModel):
    teams_count: int
    teams_by_status: dict[str, int]
    agents_in_team: int
    agents_idle: int
    total_tasks: int
    tasks_by_status: dict[str, int]


# ── Skills ──

class SkillInfo(BaseModel):
    name: str
    description: str
    available: bool


# ── Logs ──

class LogEntry(BaseModel):
    time: str
    level: str
    logger: str
    message: str


# ── Health ──

class HealthResponse(BaseModel):
    status: str
    version: str


# ── MCP ──

class InstallResponse(BaseModel):
    status: str
    name: str


# ── Config ──

class UpdateStatusResponse(BaseModel):
    status: str
```

## Step 2：路由使用 Response Model

所有返回 `dict[str, Any]` 的路由改成对应的 Response Model：

```python
# 改前
@router.get("/")
async def list_agents(request: Request) -> list[dict[str, Any]]:
    return [{"id": defn.id, "name": defn.name, ...}]

# 改后
from see_agent.server.schemas import AgentSummary

@router.get("", response_model=list[AgentSummary])
async def list_agents(request: Request) -> list[AgentSummary]:
    return [AgentSummary(id=defn.id, name=defn.name, ...)]
```

注意：`@router.get("/")` 同时改成 `@router.get("")` 修复 trailing slash 问题（见 Part B）。

需要改的路由文件：`agents.py`、`team.py`、`dashboard.py`、`skills.py`、`logs.py`、`config_routes.py`、`health.py`、`mcp.py`、`tools.py`。

有几个路由返回动态结构（如 `GET /api/config` 返回整个 config dict），这些可以保持 `dict[str, Any]`，不强求。

## Step 3：自动生成 TypeScript 类型

安装依赖：
```bash
cd web && npm install -D openapi-typescript
```

新建 `scripts/generate-api-types.sh`：
```bash
#!/bin/bash
set -e
cd "$(dirname "$0")/.."

# 1. 从 FastAPI app 导出 OpenAPI spec（不启动服务）
.venv/bin/python -c "
from see_agent.server.app import app
import json, pathlib
spec = app.openapi()
pathlib.Path('web/openapi.json').write_text(json.dumps(spec, indent=2))
"

# 2. 生成 TypeScript 类型（全量覆盖）
cd web
mkdir -p src/types/generated
npx openapi-typescript openapi.json -o src/types/generated/api.d.ts

# 3. 清理
rm -f openapi.json

echo "✅ API types generated: web/src/types/generated/api.d.ts"
```

生成的 `api.d.ts` **每次全量覆盖**，删了字段就没了，不会残留。这个文件要 commit 到仓库。

## Step 4：前端使用生成的类型

**删除手写类型文件**：
- 删除 `web/src/types/agent.ts`
- 删除 `web/src/types/team.ts`

**前端代码改为引用 generated 类型**：
```typescript
// 推荐在 web/src/types/index.ts 做 re-export 方便使用
import type { components } from './generated/api'

export type AgentSummary = components['schemas']['AgentSummary']
export type AgentDetail = components['schemas']['AgentDetail']
export type TeamSummary = components['schemas']['TeamSummary']
export type DashboardResponse = components['schemas']['DashboardResponse']
export type SkillInfo = components['schemas']['SkillInfo']
export type LogEntry = components['schemas']['LogEntry']
// ...

// 页面和 API 层这样用：
import type { AgentSummary } from '@/types'
```

## Step 5：防绕过检查

**目标**：确保没有人在 `schemas.py` → `api.d.ts` 链条之外手写 API 交互类型。

**类型文件目录规范**：
```
web/src/types/
├── generated/
│   └── api.d.ts         ← 自动生成，前后端交互类型，不要手改
├── index.ts             ← re-export generated 类型，方便 import
└── (不允许其他 .ts 文件)
```

纯前端的 UI 类型（组件 props 等）直接写在组件文件里或组件目录下，不放 `types/`。

**check.sh 中的防绕过检查**（见 Step 6 完整 check.sh）：

1. **`web/src/types/` 下除了 `generated/` 和 `index.ts` 不允许有 `.ts` 文件** — 防止有人又手写类型文件
2. **`web/src/api/*.ts` 里不允许定义 `export interface` 或 `export type X = {`** — 防止在 API 调用层内联定义类型
3. **重新生成 api.d.ts 后和仓库对比** — 防止 CC 改了 schemas.py 但忘了重新生成

---

# Part B：测试防护体系补强

## Bug 修复：Trailing Slash（P1）

`GET /api/agents` 返回 404，`GET /api/agents/` 才正常。

**修复**：所有 `@router.get("/")` 改成 `@router.get("")`。Step 2 改路由时顺手做。

需要检查的文件：`routes/agents.py`、`routes/team.py`，以及所有用 `prefix=` 的 router 的列表路由。

## 测试环境隔离

### conftest.py 全局守卫

新建 `tests/conftest.py`：

```python
"""Global test isolation — prevent any test from touching ~/.see-agent/."""
import json

import pytest
from unittest.mock import patch


@pytest.fixture(autouse=True)
def isolate_workspace(tmp_path):
    """All tests automatically isolated to a temp directory."""
    with (
        patch("see_agent.config.WORKSPACE_DIR", tmp_path),
        patch("see_agent.config.CONFIG_PATH", tmp_path / "config.json"),
        patch("see_agent.config.SESSIONS_DIR", tmp_path / "sessions"),
        patch("see_agent.config.LOGS_DIR", tmp_path / "logs"),
        patch("see_agent.config.SKILLS_DIR", tmp_path / "skills"),
        patch("see_agent.config.AGENTS_DIR", tmp_path / "agents"),
        patch("see_agent.config.TEAMS_DIR", tmp_path / "teams"),
    ):
        for d in ("sessions", "logs", "skills", "agents", "teams"):
            (tmp_path / d).mkdir(exist_ok=True)
        # Write a minimal config so tests that load config don't crash
        (tmp_path / "config.json").write_text(json.dumps({
            "llm": {"api_key": "test", "model": "test"}
        }))
        yield tmp_path
```

加了这个后，各测试文件里重复的 workspace fixture 不用立刻删（冗余但不冲突），可以逐步清理。

### SEE_AGENT_HOME 环境变量

`see_agent/config.py` 改一行：

```python
# 原来
WORKSPACE_DIR = Path.home() / ".see-agent"

# 改为
WORKSPACE_DIR = Path(os.environ.get("SEE_AGENT_HOME", "~/.see-agent")).expanduser()
```

---

# 完整 check.sh

```bash
#!/usr/bin/env bash
set +e

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'
PASS=0
FAIL=0

cd "$(dirname "$0")/.." || exit 1

run_step() {
    local name="$1"; shift
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "▶ $name"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    if "$@"; then
        echo -e "${GREEN}✅ $name passed${NC}"
        ((PASS++))
    else
        echo -e "${RED}❌ $name FAILED${NC}"
        ((FAIL++))
    fi
}

# ── 1. 后端类型检查 ──
run_step "pyright 类型检查" npx pyright@latest see_agent/ --pythonpath .venv/bin/python

# ── 2. 后端 lint ──
run_step "ruff lint" .venv/bin/ruff check see_agent/ tests/

# ── 3. 前端类型检查 ──
run_step "tsc 前端类型检查" bash -c 'cd web && npx tsc --noEmit'

# ── 4. 后端测试 ──
run_step "pytest" .venv/bin/pytest tests/ -v

# ── 5. 前端 build ──
run_step "前端 build" bash -c 'cd web && npm run build'

# ── 6. API 契约一致性 ──
run_step "API 契约检查" bash -c '
  bash scripts/generate-api-types.sh > /dev/null 2>&1

  # 6a. 生成的类型和仓库一致（CC 没忘记重新生成）
  if ! git diff --exit-code web/src/types/generated/api.d.ts > /dev/null 2>&1; then
    echo "  ❌ api.d.ts out of date — run: bash scripts/generate-api-types.sh"
    exit 1
  fi

  # 6b. types/ 下没有手写的类型文件
  STRAY=$(find web/src/types -name "*.ts" -not -path "*/generated/*" -not -name "index.ts" 2>/dev/null)
  if [ -n "$STRAY" ]; then
    echo "  ❌ 手写类型文件请删除，API 类型必须来自 generated/："
    echo "$STRAY"
    exit 1
  fi

  # 6c. api/ 层没有内联 interface/type 定义
  if grep -rn "^export interface \|^export type .* = {" web/src/api/*.ts 2>/dev/null; then
    echo "  ❌ web/src/api/ 中不允许手写类型定义"
    exit 1
  fi
'

# ── 7. API 冒烟 ──
run_step "API 冒烟测试" bash -c '
  TMPWS=$(mktemp -d)
  trap "rm -rf $TMPWS" EXIT
  mkdir -p $TMPWS/{logs,skills,agents,teams}
  echo "{\"llm\":{\"api_key\":\"smoke\",\"model\":\"x\"}}" > $TMPWS/config.json

  export SEE_AGENT_HOME="$TMPWS"
  PORT=18$(shuf -i 100-999 -n 1)
  .venv/bin/python -m uvicorn see_agent.server.app:app \
    --host 127.0.0.1 --port $PORT --log-level warning &
  PID=$!
  sleep 2

  FAIL=0
  for ep in /api/health /api/config /api/agents/ /api/teams/ /api/dashboard /api/schemas/config; do
    if ! curl -sf "http://127.0.0.1:$PORT$ep" > /dev/null 2>&1; then
      echo "  ❌ $ep failed"
      FAIL=1
    fi
  done

  kill $PID 2>/dev/null; wait $PID 2>/dev/null
  exit $FAIL
'

# ── 8. CLI 冒烟 ──
run_step "CLI 冒烟" bash -c '
  .venv/bin/see-agent version > /dev/null && .venv/bin/see-agent config show > /dev/null
'

# ── 汇总 ──
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "结果: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}有检查未通过，请修复后重试${NC}"
    exit 1
else
    echo -e "${GREEN}全部通过 🎉${NC}"
    exit 0
fi
```

---

# 改完后的全景

```
CC 改代码后跑 check.sh（8 步，约 3 分钟）：

 1. pyright        → 后端类型/签名错误
 2. ruff           → 后端代码规范
 3. tsc            → 前端类型错误（包括 API 字段名不匹配）
 4. pytest         → 后端逻辑错误
 5. 前端 build      → 前端编译错误
 6. API 契约检查    → schemas.py 和 api.d.ts 一致性 + 无手写类型泄漏
 7. API 冒烟        → 服务能启动 + 核心端点 200
 8. CLI 冒烟        → 基本命令能跑

全过 = 代码可以合。
```

---

# 文件清单

| # | 文件 | 改动 |
|---|------|------|
| 1 | `see_agent/server/schemas.py` | **新建** — API Response Model（唯一事实源） |
| 2 | `see_agent/server/routes/agents.py` | `dict[str, Any]` → Response Model + `"/"` → `""` |
| 3 | `see_agent/server/routes/team.py` | 同上 |
| 4 | `see_agent/server/routes/dashboard.py` | 同上 |
| 5 | `see_agent/server/routes/skills.py` | 同上 |
| 6 | `see_agent/server/routes/logs.py` | 同上 |
| 7 | `see_agent/server/routes/config_routes.py` | 同上 |
| 8 | `see_agent/server/routes/health.py` | 同上 |
| 9 | `see_agent/server/routes/mcp.py` | 同上 |
| 10 | `see_agent/server/routes/tools.py` | 同上 |
| 11 | `see_agent/config.py` | 加 `SEE_AGENT_HOME` 环境变量（1 行） |
| 12 | `tests/conftest.py` | **新建** — 全局测试隔离 |
| 13 | `scripts/generate-api-types.sh` | **新建** — OpenAPI → TS 类型生成 |
| 14 | `scripts/check.sh` | **重写** — 8 步完整检查 |
| 15 | `web/package.json` | devDependencies 加 `openapi-typescript` |
| 16 | `web/src/types/generated/api.d.ts` | **自动生成** — 不要手改 |
| 17 | `web/src/types/index.ts` | re-export generated 类型 |
| 18 | `web/src/types/agent.ts` | **删除** |
| 19 | `web/src/types/team.ts` | **删除** |
| 20 | 前端 `api/*.ts` 和 `pages/*.tsx` | import 改为 `@/types`（generated） |

---

# 执行顺序

1. **Step 1-2**：后端 schemas.py + 路由改 Response Model + trailing slash → 跑 pyright + pytest
2. **Step 3**：generate-api-types.sh + 安装 openapi-typescript → 验证能生成
3. **Step 4**：前端删手写类型 + 改 import → 跑 tsc
4. **conftest.py + SEE_AGENT_HOME**：测试隔离
5. **check.sh 重写**：8 步全检
6. 最后跑一遍完整 `check.sh`，全过即可
