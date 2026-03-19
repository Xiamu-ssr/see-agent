# Bugfix Round 8: 截图重复 + System Agent 升级 + 前端输入优化 + 消息来源

## 报告人
草莓🍓 | 2026-03-19

---

## 🔴 Bug 43 (P0): 截图 tool 产生两张重复图片

### 现象
Agent 调用 screenshot tool 后，基模收到两条连续的 role: "user" 图片消息。

### 根因
`loop_core.rs` 中 `run_one_turn` 的 screenshot 处理：

```rust
// 第一次：tool 返回的 images → add_tool_result → add_screenshot(base64)
ctx.add_tool_result(&tc.id, &result.text, &ctx_images);

// 第二次：又调了 self.eye.capture().await → save_screenshot_ref
if tc.name == "screenshot" {
    if let Ok(new_ss) = self.eye.capture().await {  // ← 又截了一次屏幕！
        self.save_screenshot_ref(ctx, &new_ss);     // ← 又加了一张图片！
    }
}
```

### 修复
不要重新截屏。用第一次 tool 返回的图片数据直接保存到磁盘：

```rust
if tc.name == "screenshot" && !result.images.is_empty() {
    let img = &result.images[0];
    // 保存到磁盘（path ref），但不要再 add_screenshot
    let path = self.save_image_to_disk(&img.base64, &img.mime_type)?;
    // 只更新 screen_dims，不加新消息
    // screen_dims 可以从 tool result 的 metadata 中获取
}
```

同时删除 `if tc.name == "screenshot"` 块中的 `self.eye.capture().await`。

---

## Bug 38 (P1): System Agent workspace 升级初始化不完整

### 现象
已存在的 workspace 的 system agent 缺少 skills 目录、SKILL.md、SOUL.md、AGENTS.md。因为 `ensure_workspace` 只在 `!agent_json.exists()` 时初始化。

### 修复
把 `if !system_dir.agent_json().exists()` 的检查改为分项检查：

```rust
// agent.json
if !system_dir.agent_json().exists() {
    // 创建 agent.json
}
// 确保 skills 目录和内容存在（即使 agent.json 已有）
let skills_dir = system_dir.path().join("skills").join("system-management");
if !skills_dir.exists() {
    std::fs::create_dir_all(&skills_dir)?;
    std::fs::write(skills_dir.join("SKILL.md"), SYSTEM_SKILL)?;
}
// 确保 agent.json 有 skills 配置
// 读取现有 agent.json，如果没有 skills 字段就补上
// 确保 SOUL.md 存在
if !system_dir.soul_md().exists() {
    write_text(&system_dir.soul_md(), SYSTEM_SOUL_TEMPLATE)?;
}
// 确保 AGENTS.md 存在
if !system_dir.agents_md().exists() {
    write_text(&system_dir.agents_md(), AGENTS_TEMPLATE)?;
}
```

在 templates/ 下新增 `system-soul.md`：
```markdown
# SOUL.md

你是 See-Agent-Corp 的系统管理员 Agent。

## 性格
- 专业、高效
- 乐于帮助用户管理系统
- 熟悉 workspace 的所有细节

## 原则
- 保护系统安全
- 谨慎执行危险操作
- 清晰解释每个操作的影响
```

---

## Bug 44 (P1): 前端消息来源显示优化

### 现象
用户消息在聊天框里显示为 `[用户] 你好`，`[用户]` 前缀是 content 的一部分。应该把来源显示为 chat header。

### 修复

**后端 messages.jsonl 改进：**
1. `user_reply` 类型增加 `priority` 字段（目前只有 sender）：
```json
{"type":"user_reply", "content":"你好", "sender":"user", "priority":"collect"}
```
2. 存入 messages.jsonl 的 content **不要带 `[用户]` 前缀**——前缀只在发给基模的运行时 context 中加
3. 发给基模的 context 中保持 `[用户]` 前缀（基模需要区分消息来源）

**前端改进：**
1. 解析 `sender` 字段，映射为显示名：`user` → "You"，`xiaoming` → "🔬 小明"
2. 显示在 chat-header 位置
3. content 中如果有 `[xxx]` 前缀，剥离后再显示
4. steer 消息可以加一个小标签 `⚡ steer`

---

## Bug 45 (P1): 前端输入区域优化

### 现象
Collect/Steer 下拉框太大，占据过多空间。

### 修复
1. 输入框改为单行高度（`rows="1"`），文字多了自动变高（max 5 行）
2. Collect/Steer 改为小按钮样式的下拉框（`select-xs`）
3. 输入框和 Send 按钮等高
4. 布局：`[输入框] [C▾] [Send]`，一行紧凑排列

```html
<div class="flex items-end gap-1">
    <textarea class="textarea textarea-bordered textarea-sm flex-1 resize-none min-h-[36px] max-h-[120px]"
        rows="1"
        placeholder="消息... (Ctrl+Enter 发送)"
    ></textarea>
    <select class="select select-bordered select-xs w-16">
        <option value="collect">C</option>
        <option value="steer">S</option>
    </select>
    <button class="btn btn-primary btn-sm">Send</button>
</div>
```

---

## Bug 46 (P2): Chat 消息支持 Markdown 渲染

### 现象
Agent 回复的 Markdown 内容（代码块、列表、粗体等）显示为纯文本。

### 修复
Agent 消息已经使用了 `render_markdown()` 函数（在 `assistant` 类型消息中）。检查是否正确渲染。如果样式不对，检查 CSS 是否包含 markdown-body 样式类。

用户消息不需要 Markdown 渲染。

---

## Bug 39 (P2): Tool 命名空间/分组机制

### 设计

**ToolRegistry 改造：**
```rust
struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    groups: HashMap<String, Vec<String>>,  // group_name → [tool_names]
}
```

注册时指定 group：
```rust
registry.register_group("core", vec![shell_tool, read_tool, screenshot_tool, finished_tool]);
registry.register_group("memory", vec![memory_search_tool, memory_get_tool]);
registry.register_group("team", vec![send_message_tool, ...]);
```

**OpenAI API 传出时**：tool name 保持原样（不加前缀），因为当前 tools 没有重名。等 skill/MCP tools 加入时，再用 `namespace__tool_name` 格式。

**前端 Tools tab：**
按 group 分组展示，每组一个折叠区域：
```
▾ Core (4)
  shell ✅  read ✅  screenshot ✅  finished ✅
▾ Memory (2)
  memory_search ✅  memory_get ✅
▾ Team (5)
  send_message ✅  create_task ✅  ...
```

---

## 实施约束

1. **Bug 43 最先修**——截图重复是 P0
2. **Bug 38 次之**——system agent 初始化不完整
3. 然后 44 → 45 → 46 → 39
4. 每步 cargo test
5. 最后 trunk build --release + git commit
6. **时间充裕，不急**
