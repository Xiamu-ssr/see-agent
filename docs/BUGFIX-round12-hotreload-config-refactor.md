# Round 12: 热加载 + 配置重构 + 前端错误提示 + Skill 一致性

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 61 (P1): Config 热加载

### 现象
修改 config.json 后 Worker 不生效，必须重启。

### 修复
Worker 推理循环每次迭代前检查 config.json 的 mtime，变了就重新加载。延迟热加载：

```rust
// worker.rs 主循环中
let mut config_mtime = std::fs::metadata(&config_path)?.modified()?;

loop {
    // 检查 config 是否变化
    if let Ok(meta) = std::fs::metadata(&config_path) {
        if let Ok(new_mtime) = meta.modified() {
            if new_mtime > config_mtime {
                info!("config changed, reloading");
                config = load_config(&config_path)?;
                // 重建 brain（base_url/api_key/model 可能变了）
                brain = OpenAiBrain::new(&config.llm);
                // 重建 system prompt（skills 可能变了）
                // ...
                config_mtime = new_mtime;
            }
        }
    }
    // ... drain + run_one_turn
}
```

serve 进程也要 watch config.json，更新内存中的 `RwLock<Config>`。

---

## Bug 62 (P1): 前端错误提示 Toast

### 现象
LLM 返回 402 等错误，前端无任何提示。用户只看到 agent 不回复。

### 修复
1. 后端新增 `GET /api/agents/{id}/errors` 端点，返回最近的错误（从 worker.log 解析，或内存中维护一个 error ring buffer）
2. 前端 Chat 界面轮询 errors 端点，有新错误时在右下角显示 toast 通知（自动 3 秒消失）
3. 用 DaisyUI 的 `toast` + `alert` 组件

或者更简单：在 `messages.jsonl` 中新增一个 `error` 类型消息，LLM 调用失败时写入。前端展示为红色系统消息。

---

## Bug 63 (P1): Skill tab 展示 vs 运行时不一致

### 现象
API `/agents/{id}/skills` 用全局+专属合并逻辑，Worker 运行时用覆盖逻辑。

### 修复
**统一为覆盖逻辑**——和 config merge 机制一致（数组字段覆盖）：

`list_agent_skills_handler` 改为：
```rust
let dirs = if agent 有 skills.dirs 配置 {
    agent.skills.dirs  // 覆盖，不合并
} else {
    config.skills.dirs  // 用全局
};
```

如果 system agent 想要全局 skill + 专属 skill，在自己的 `agent.json` 里把两个目录都写上：
```json
{
  "skills": {
    "dirs": [
      "/Users/lanxuan/.see-agent-corp/agents/system/skills",
      "~/.see-agent-corp/skills"
    ]
  }
}
```

同时更新 system agent 的 `agent.json`（在 `ensure_workspace` 中）。

---

## Bug 64 (P2): Config 重构——compact + 图片消退

### 现象
config.json 的 compact 配置和实际压缩策略有 gap。`max_images` 语义不明。

### 修复

**删除：** `agent.max_images`

**新增/修改 `agent.compact`：**
```json
{
  "agent": {
    "max_steps": 50,
    "compact": {
      "context_window": 200000,
      "microcompact_ratio": 0.30,
      "full_compact_ratio": 0.95,
      "keep_recent": 8,
      "summary_model": "",
      "image_high_count": 3,
      "image_low_count": 3
    }
  }
}
```

- `microcompact_ratio`：Layer 2 触发阈值（清旧 tool_result）
- `full_compact_ratio`：Layer 3 触发阈值（LLM summarize）
- `image_high_count`：四级消退策略 Level 1 数量（detail: high）
- `image_low_count`：Level 2 数量（detail: low）
- 删除旧的 `target_ratio`（没被用）

代码中 `MICROCOMPACT_RATIO` 和 `FULL_COMPACT_RATIO` 常量改为从 config 读取。`IMAGE_LEVEL1_COUNT` 和 `IMAGE_LEVEL2_COUNT` 也从 config 读取。

Config 页面前端需要正确渲染嵌套对象（当前跳过了 compact 子对象）。

---

## Bug 65 (P2): Files tab 大文件处理

### 现象
`messages.jsonl` 等大文件点不开。

### 修复
后端读取文件内容的 API 加 `max_size` 限制（默认 100KB），超过时只返回最后 100KB + 提示 "文件过大，仅显示最后部分"。

前端用 `<pre>` + `overflow-auto` + `max-height` 展示文件内容，避免撑爆页面。

---

## Bug 66 (P3): send_message tool 支持 priority 参数

### 现象
Agent 间 send_message 只能发 collect。

### 修复
`SendMessageTool` 的参数新增可选 `priority` 字段（默认 "collect"）：
```json
{
    "to": "designer",
    "content": "紧急停止",
    "priority": "steer"
}
```

---

## 实施约束

1. 顺序：Bug 63 → Bug 64 → Bug 61 → Bug 62 → Bug 65 → Bug 66
2. Bug 64 改 config 结构后要兼容旧 config（旧字段 `max_images` / `target_ratio` 存在时自动迁移）
3. 每步 cargo test
4. 最后 trunk build --release + git commit + git push
