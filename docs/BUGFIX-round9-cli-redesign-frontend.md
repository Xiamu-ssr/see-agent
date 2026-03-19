# Round 9: CLI 重设计 + 前端修复 + 联动契约

## 报告人
草莓🍓 | 2026-03-19

---

## 一、CLI 重设计

### 删除的命令
- `init`（合并到 start）
- `config show`、`config path`（不需要 CLI）
- `send`（agent 用 tool 做）
- `serve`（合并到 start）

### 新 CLI 结构

```
see-agent-corp
├── start [--port]                        # 启动（含初始化 + system agent）
├── stop                                  # 停止
├── restart [--port]                      # stop + start
├── status                                # 系统状态
├── agent
│   ├── create -i -n -e                   # 创建 agent
│   ├── list                              # 列出 agents
│   ├── show <id>                         # 显示详情
│   ├── delete <id>                       # 删除 agent（含联动）
│   └── team <id> <team_id|none>          # 调整 agent 所属 team（含联动）
└── team
    ├── create <name> -l -m               # 创建 team
    ├── list                              # 列出 teams
    ├── show <id>                         # 显示详情
    ├── delete <id>                       # 删除 team（含联动）
    └── leader <id> <agent_id>            # 更换 leader（含联动）
```

### start 命令改造
1. 首次运行自动 `ensure_workspace()`（含 system agent 初始化）
2. 检查 stale PID 文件 → 清理
3. 启动 HTTP server（前后端一体）
4. 日志输出：`🚀 Server running at http://localhost:{port}`

### stop 命令修复
当前 stop 后 start 会端口占用。修复：
1. SIGTERM → 等 5 秒 → SIGKILL
2. **必须删 PID 文件**
3. 等端口释放（sleep 500ms）

---

## 二、CLI 联动契约

### agent delete <id>
除了删目录，还需要：
1. 遍历所有 team.json，从 members 中移除该 agent
2. 如果该 agent 是 leader → 报错 "请先更换 leader" 
3. 如果 team 只剩它一人 → 删除整个 team
4. 读 `worker.pid` 发 SIGTERM 杀掉 Worker 进程
5. 清理 TaskBoard 中 assigned_to 为该 agent 的任务 → 改回 unassigned

### agent team <id> <team_id|none>
1. 如果 `none` → 离开当前 team
   - 从 team.json members 中移除
   - 如果是 leader → 报错 "请先更换 leader"
   - 如果 team 只剩它 → 删除整个 team
2. 如果 `team_id` → 加入新 team
   - 如果已在其他 team → 先离开旧 team
   - 在新 team.json members 中添加 `{id, role: "member"}`
3. 重启该 agent 的 Worker（team 变了 → system prompt + tools 变了）

### team delete <id>
1. 重启所有成员的 Worker（team 没了 → tools/prompt 变了）
2. TaskBoard 随目录一起删除（可接受）

### team leader <id> <agent_id>
1. 修改 team.json 的 leader 字段
2. 重启旧 leader 的 Worker（prompt 从 leader 变 member）
3. 重启新 leader 的 Worker（prompt 从 member 变 leader）

### Worker 重启机制
需要 supervisor 暴露 `restart_agent(id)` 方法：
1. 写 shutdown 到 agent 的 inbox
2. 发 SIGUSR1 唤醒（如果在 sleep）
3. 等 Worker 退出
4. 下次收到消息时 supervisor 自动重新 spawn

---

## 三、前端修复

### Bug 47 (P1): 聊天窗口整页滚动
反复提过多次了！关键：右侧 panel `overflow-hidden`（不是 `overflow-y-auto`），只有 messages 区域内部滚动。参考主流聊天软件的布局。

整个 AgentsPage 的高度应该是 `h-screen`（或 `h-full`），不允许页面级滚动。

### Bug 48 (P1): Tool 展开没有输入参数
前端折叠展开 tool 消息只显示了 `tool_result` 的 content。应该同时显示 tool 输入：
- 在 `assistant` 类型消息的 `tool_calls` 中找到 `function.arguments`
- 展开后显示：
  ```
  🔧 shell
  ┌ Input: {"command": "ls -la"}
  └ Result: exit code: 0\nstdout:\n...
  ```

实现方案：前端在渲染 tool_result 时，向上查找最近的 `assistant` 消息中匹配的 `tool_call_id`，提取 `function.arguments` 和 `function.name`。

### Bug 49 (P1): Skills tab 显示的是全局 skills 不是 agent 专属
当前前端调的是 `GET /api/skills`（全局），应该改为调 `GET /api/agents/{id}/skills`。

需要后端新增 `/api/agents/{id}/skills` 端点：
1. 读 agent.json 的 skills 配置
2. 合并全局 skills.dirs 和 agent 专属 dirs
3. 返回该 agent 实际能看到的 skills

同时注意：system agent 需要也加载全局 `~/.see-agent-corp/skills`，当前 agent.json 只配了 `agents/system/skills`。

### Bug 50 (P1): Collect/Steer 显示为"普通/加急"
当前显示 `C` / `S`，改为：
- `普通`（对应 collect）
- `加急`（对应 steer）

### Bug 51 (P2): Config 页面基于 Schema 智能渲染
后端已有 `GET /api/config/schema` 返回 JSON Schema（含类型、默认值）。前端应基于 schema 渲染智能表单：
- `type: "string"` → 文本输入框
- `type: "integer"` / `type: "number"` → 数字输入框
- `type: "array"` → 可增删的列表
- `type: "object"` → 嵌套折叠区域
- `type: "boolean"` → 开关

当前全是文本框，需要根据 schema 动态渲染。

---

## 实施约束

1. **CLI 重设计优先**（删命令 + 改 start/stop + 联动逻辑）
2. **然后前端修复**（47→48→49→50→51）
3. 每步 cargo test
4. 最后 trunk build --release + git commit + git push
5. CLI 联动逻辑要写测试（delete agent 从 team 移除、leader 变更等）
