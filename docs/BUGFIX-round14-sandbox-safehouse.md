# Round 14: macOS Safehouse 沙箱集成

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 72 (P1): Agent Worker 进程沙箱化

### 设计

**核心：** supervisor spawn worker 时用 Safehouse 包裹，deny-first，每个 agent 动态生成权限。

### 前置

1. Safehouse 是系统必要依赖
2. `see-agent-corp start` 启动时检测 `safehouse` 是否在 PATH 中，不在则**报错退出**并提示：`错误：缺少依赖 agent-safehouse。请运行：brew install eugene1g/safehouse/agent-safehouse`
3. config.json 新增 `sandbox.enabled: bool`（默认 true），设为 false 可关闭沙箱（开发调试用）
4. `sandbox.enabled: false` 时在 Dashboard 和 Agent 详情页显示 ⚠️ 警告

### 权限模型

**基础 profile（所有 agent 共享，复用 Safehouse 内置兼容性）：**
- 只读：`/usr/bin`, `/usr/local/bin`, `/bin`, `/sbin`, `/opt/homebrew`
- 只读：`/etc`, `/private/etc`（DNS 解析等）
- 只读：`/Library/Frameworks`, `/System`
- 读写：`/tmp`, `/var/tmp`
- 网络：允许出口（LLM API + shell 中的网络命令）

**System Agent 专属：**
- 读写：`~/.see-agent-corp/`（整个工作目录）

**普通 Agent 专属：**
- 读写：`~/.see-agent-corp/agents/{id}/`（自己的目录）
- 只读：`~/.see-agent-corp/config.json`
- 只读：`~/.see-agent-corp/skills/`

**有 Team 的 Agent 额外：**
- 读写：`~/.see-agent-corp/teams/{team_id}/shared/`
- 只读：`~/.see-agent-corp/teams/{team_id}/team.json`
- 只读：`~/.see-agent-corp/teams/{team_id}/tasklist.json`

**agent.json 可选扩展：**
```json
{
  "sandbox": {
    "extra_read": ["/Users/lanxuan/Documents/reports/"],
    "extra_write": ["/tmp/agent-workspace/"]
  }
}
```

### 实现

```rust
// supervisor/manager.rs
fn spawn_worker(&self, agent_id: &str) -> Result<Child> {
    let profile = build_sandbox_profile(agent_id, &self.workspace)?;
    
    if self.sandbox_enabled && self.safehouse_available {
        Command::new("safehouse")
            .args(profile.to_safehouse_args())
            .arg("--")
            .arg(&self.binary_path)
            .args(["worker", agent_id, &self.workspace_path])
            .stderr(log_file)
            .stdout(log_file)
            .spawn()
    } else {
        // 降级：无沙箱
        Command::new(&self.binary_path)
            .args(["worker", agent_id, &self.workspace_path])
            .stderr(log_file)
            .stdout(log_file)
            .spawn()
    }
}
```

### 权限被拒绝时的行为

当 agent 尝试访问沙箱外的文件时：
- shell tool：命令返回 `Operation not permitted` 错误
- read/write tool：系统调用返回 EPERM
- 这些错误会正常回到 LLM 的 tool_result 中，agent 会看到并理解
- **同时写一条 WARNING 到 worker.log**：`WARN sandbox: agent "developer" access denied: /etc/passwd (read)`

---

## Bug 73 (P2): 前端沙箱状态展示 + 日志等级着色

### Agent 详情页

**Details - Overview 区域新增"安全沙箱"卡片：**

```
🛡️ 安全沙箱
├── 状态：✅ 已激活 / ⚠️ 未激活（Safehouse 未安装）
├── 引擎：macOS Safehouse
├── 可读目录：
│   • ~/.see-agent-corp/agents/developer/ (读写)
│   • ~/.see-agent-corp/skills/ (只读)
│   • ~/.see-agent-corp/config.json (只读)
│   • ~/.see-agent-corp/teams/dev-team/shared/ (读写)
├── 网络：✅ 允许出口
└── 额外权限：无
```

**后端新增 API：**

`GET /api/agents/{id}/sandbox`
```json
{
  "enabled": true,
  "engine": "safehouse",
  "available": true,
  "profile": {
    "rw_dirs": ["~/.see-agent-corp/agents/developer/", "~/.see-agent-corp/teams/dev-team/shared/"],
    "ro_dirs": ["~/.see-agent-corp/config.json", "~/.see-agent-corp/skills/"],
    "network_outbound": true,
    "extra_read": [],
    "extra_write": []
  }
}
```

### Chat 中权限拒绝的展示

当 tool_result 包含 `Operation not permitted` 或 `Permission denied` 时，前端在该 tool 消息上额外显示一个红色小标签：

```
🔧 shell ❌ 🛡️ 沙箱拦截
┌ Input: {"command": "cat /etc/passwd"}
└ Result: cat: /etc/passwd: Operation not permitted
```

前端通过关键词匹配 tool_result 内容判断（`Operation not permitted` / `Permission denied` / `EPERM`）。

### 全局状态

`/status` 或 Dashboard 页面显示：
```
🛡️ 沙箱保护：✅ 已启用（Safehouse v1.x）
   活跃 Agent：3/3 已沙箱化
```

如果 Safehouse 未安装：
```
🛡️ 沙箱保护：⚠️ 未启用
   安装方法：brew install eugene1g/safehouse/agent-safehouse
```

### Log 界面等级着色

`/logs` 页面和 Agent Details - Logs tab，日志按等级着色：
- `ERROR` → 红色
- `WARN` → 橙色/黄色
- `INFO` → 默认色
- `DEBUG` → 灰色

沙箱拦截的 WARN 日志会特别醒目。

---

## 实施约束

1. 顺序：Bug 72 后端沙箱集成 → Bug 73 前端展示
2. Bug 72 需要先在本机 `brew install eugene1g/safehouse/agent-safehouse` 测试
3. 沙箱降级（未安装时）必须正常工作，不能阻塞启动
4. team 变更、agent 权限变更时，需要重启 worker（沙箱 profile 在 spawn 时确定）
5. 每步 cargo test
6. 最后 trunk build --release + git commit + git push
