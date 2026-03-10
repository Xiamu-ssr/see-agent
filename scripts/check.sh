#!/usr/bin/env bash
# scripts/check.sh — 质量门禁，改完代码必须全过
# 用法: bash scripts/check.sh

set +e  # 不要遇到失败就退出，跑完全部再汇总

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

PASS=0
FAIL=0

# 项目根目录
cd "$(dirname "$0")/.." || exit 1

run_step() {
    local name="$1"
    shift
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

# ── 1. 后端静态类型检查 ──
run_step "pyright 类型检查" npx pyright@latest see_agent/ --pythonpath .venv/bin/python

# ── 2. 后端 Lint ──
run_step "ruff lint" .venv/bin/ruff check see_agent/ tests/

# ── 3. 前端类型检查（抓 API 字段漂移）──
run_step "tsc 前端类型检查" bash -c "cd web && npx tsc --noEmit"

# ── 4. 后端单元测试 ──
run_step "pytest 单元测试" .venv/bin/pytest tests/ -v

# ── 5. 前端构建 ──
run_step "vite 前端构建" npm --prefix web run build

# ── 6. API 契约检查 ──
run_step "API contract check" bash -c '
    set -e
    # Regenerate and diff
    bash scripts/generate-api-types.sh > /dev/null 2>&1
    if ! git diff --quiet web/src/types/generated/api.d.ts; then
        echo "api.d.ts is out of date — run: bash scripts/generate-api-types.sh"
        git diff --stat web/src/types/generated/api.d.ts
        git checkout web/src/types/generated/api.d.ts 2>/dev/null
        exit 1
    fi
    # No stray hand-written type files
    for f in web/src/types/agent.ts web/src/types/team.ts; do
        if [ -f "$f" ]; then
            echo "Stray type file found: $f — delete it and use @/types instead"
            exit 1
        fi
    done
    echo "Contract OK"
'

# ── 7. API 冒烟测试 ──
run_step "API smoke" bash -c '
    set -e
    SMOKE_PORT=$((18900 + RANDOM % 100))
    TMPDIR=$(mktemp -d)
    trap "rm -rf $TMPDIR" EXIT
    export SEE_AGENT_HOME="$TMPDIR"
    mkdir -p "$TMPDIR"
    echo "{\"llm\":{\"api_key\":\"test\",\"model\":\"test\"}}" > "$TMPDIR/config.json"
    .venv/bin/uvicorn see_agent.server.app:app --host 127.0.0.1 --port $SMOKE_PORT &
    PID=$!
    trap "kill $PID 2>/dev/null; rm -rf $TMPDIR" EXIT
    for i in $(seq 1 20); do
        if curl -sf http://127.0.0.1:$SMOKE_PORT/api/health > /dev/null 2>&1; then break; fi
        sleep 0.3
    done
    curl -sf http://127.0.0.1:$SMOKE_PORT/api/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d[\"status\"]==\"ok\", d"
    curl -sf http://127.0.0.1:$SMOKE_PORT/api/agents | python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d, list), d"
    curl -sf http://127.0.0.1:$SMOKE_PORT/api/teams | python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d, list), d"
    echo "Smoke OK"
'

# ── 8. CLI 冒烟测试 ──
run_step "CLI version" .venv/bin/see-agent version
run_step "CLI config show" .venv/bin/see-agent config show

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
