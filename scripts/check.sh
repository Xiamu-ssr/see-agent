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

# ── 1. 静态类型检查（抓接口问题）──
run_step "pyright 类型检查" npx pyright@latest see_agent/ --pythonpath .venv/bin/python

# ── 2. Lint（抓代码规范）──
run_step "ruff lint" .venv/bin/ruff check see_agent/ tests/

# ── 3. 单元测试（抓逻辑问题）──
run_step "pytest 单元测试" .venv/bin/pytest tests/ -v

# ── 4. CLI 冒烟测试（抓组装问题）──
run_step "CLI version" .venv/bin/see-agent version
run_step "CLI config show" .venv/bin/see-agent config show

# ── 5. 前端类型检查 ──
run_step "tsc 前端类型检查" bash -c "cd web && npx tsc --noEmit"

# ── 6. 前端构建 ──
run_step "vite 前端构建" npm --prefix web run build

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
