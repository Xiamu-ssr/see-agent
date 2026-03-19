# Bugfix Round 6: 清理 + Memory 工具 + 前端收尾

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 30: 删除 Mode A 死代码

### 现状
`AgentLoop::run()` 和 `run_loop()` 方法在 `loop_core.rs` 中，但 CLI 里没有任何入口调用它们。完全是死代码。

### 修复
删除 `loop_core.rs` 中的 `run()` 和 `run_loop()` 方法，以及相关的 `initial_screenshot` 参数和处理逻辑。只保留 `run_one_turn()` 和 `maybe_compact()`。

---

## Bug 31: Memory 工具调整

### 现状
- 有 `memory_write` 工具（直接写 memory/MEMORY.md），但不需要——通用 `write` tool 就能做
- 没有 `memory_get` 工具（按路径+行号精确读取）

### 修复

1. **删除 `memory_write`** — 从 `builtin/memory.rs` 和注册逻辑中移除
2. **新增 `memory_get`** — 参考 OpenClaw 设计：
   - name: `memory_get`
   - description: "按路径和行号读取 memory 文件片段。配合 memory_search 使用：先搜索找到相关文件和行号，再用此工具精确读取需要的片段，节省上下文。"
   - parameters: `{ path: string (required), from: number (optional, 起始行号), lines: number (optional, 读取行数，默认20) }`
   - 实现：读取 `memory/` 目录下指定文件的指定行范围
3. **保留 `memory_search`** — 不变

---

## Bug 26: 前端 Agent 列表按 Team 分组（Round 5 未完成）

前端 Agent 列表按以下顺序分组显示：

```
⚙️ System
─────────────
📋 产品团队
  🔬 小明 (leader)
  🎨 小红 (designer)
  💻 小李 (developer)
─────────────
📦 无 Team
  🤖 其他agent...
```

需要后端 `GET /api/agents` 的返回值中每个 agent 包含 `team_id` 和 `team_name` 字段。如果目前没有，需要在 `list_agents()` 中查找填充。前端根据这些字段分组渲染。

---

## Bug 27: 前端 Details-Files tab 复制路径按钮（Round 5 未完成）

Files tab 中每个文件名旁边加一个 📋 图标按钮，点击复制该文件的完整路径（`~/.see-agent-corp/agents/{id}/{filename}`）到剪贴板。用 `navigator.clipboard.writeText()`。

---

## 实施约束

1. **顺序：** Bug 30 → Bug 31 → Bug 26 → Bug 27
2. 每步 cargo test
3. 不做兼容，不保留旧代码
4. Bug 30 删完后确保没有编译错误
5. Bug 31 新增 memory_get 要写测试
