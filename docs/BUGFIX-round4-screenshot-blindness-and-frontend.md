# Bugfix Round 4: 截图失明 + 前端重构 + System Agent

## 报告人
草莓🍓 | 2026-03-19

---

## 🔴 Bug 21 (Critical): Mode B (run_one_turn) 截图图片被丢弃——Agent 是瞎子

### 现象
Agent 调用 screenshot tool 后，收到文本返回 "screenshot captured (1728x1117)"，但实际看不到图片内容。Agent 自己承认在"幻觉"。

### 根因
`loop_core.rs` 第 504 行，`run_one_turn`（Mode B，Worker 使用）中：

```rust
ctx.add_tool_result(&tc.id, &result.text, &[]);  // ← 硬编码空数组！images 被丢弃！
```

对比 Mode A（`run_loop`）第 315-324 行正确处理了 images：

```rust
let ctx_images: Vec<super::context::ToolResultImage> = result
    .images
    .iter()
    .map(|img| super::context::ToolResultImage { ... })
    .collect();
ctx.add_tool_result(&tc.id, &result_text, &ctx_images);  // ✅ 传了 images
```

同时，`run_one_turn` 也缺少以下 Mode A 中有的逻辑：
- `save_screenshot_ref()` 保存图片到磁盘
- 更新 `screen_dims`（坐标缩放）
- `no_progress` 检测器
- `no_screenshot` 检测器

### 修复
把 Mode A 的 tool result 处理逻辑复制到 Mode B。具体是在 `run_one_turn` 的 tool 执行后：

```rust
let result = match self.registry.execute(&tc.name, tc.arguments.clone()).await {
    Ok(r) => r,
    Err(e) => { ... }
};

// 转换 images（和 Mode A 一样）
let ctx_images: Vec<super::context::ToolResultImage> = result
    .images
    .iter()
    .map(|img| super::context::ToolResultImage {
        base64: img.base64.clone(),
        mime_type: img.mime_type.clone(),
        detail: img.detail.clone(),
    })
    .collect();
ctx.add_tool_result(&tc.id, &result.text, &ctx_images);

// 如果是 screenshot tool，额外保存到磁盘 + 更新坐标
if tc.name == "screenshot" {
    if let Ok(new_ss) = self.eye.capture().await {
        self.screen_dims = (
            new_ss.width,
            new_ss.height,
            new_ss.screen_width.unwrap_or(new_ss.width),
            new_ss.screen_height.unwrap_or(new_ss.height),
        );
        self.save_screenshot_ref(ctx, &new_ss);
    }
}
```

---

## Bug 22: System Agent 初始化 + 前端隔离

### 现状
- `agents/system/` 目录在 `ensure_workspace()` 时创建，但缺少 `agent.json` 等完整文件
- `list_agents()` 把 system 和普通 agent 混在一起返回
- 前端没有 system agent 的专属栏目

### 修复

**后端：**

1. `AgentDefinition` 新增 `is_system: bool` 字段（默认 `false`）
2. `ensure_workspace()` 中用 `create_agent()` 完整初始化 system agent（如果不存在），设 `is_system: true`
3. `AgentSummary` 也新增 `is_system` 字段
4. system agent 的 skills.dirs 包含自己的目录路径（如 `~/.see-agent-corp/agents/system/skills/`），其他 agent 不包含

**前端：**

5. Agent 列表过滤掉 `is_system: true` 的 agent
6. 左侧导航栏底部或顶部增加 "⚙️ System" 入口，点击进入 system agent 的聊天界面

**System Agent 的 Skill：**

7. 在 `agents/system/skills/` 下放一个 SKILL.md，描述系统设计和 CLI 使用方法
8. 这个 skill 只有 system agent 能看到（因为在它自己的 skills 目录下）
9. 其他 agent 的 skills.dirs 不包含这个路径，自然就看不到

---

## Bug 14-20: 前端 Chat 界面重构

### 14. Chat 和 Details 分离

**现在：** Agent 页面直接显示 tabs（Chat/Details/Logs 等），Chat 是其中一个 tab。

**改为：** 
- 标题栏 "🤖 alice" 右侧有一个切换按钮：`Chat | Details`
- 默认进入 Chat 视图（全屏聊天框）
- 切换到 Details 视图时，聊天框消失，显示现有的 tab 分组（Info/Logs/Memory 等）

### 15. Chat 聊天框容器修复

**现在：** 消息多了整个页面出滚动条。

**改为：**
- 聊天框用固定高度容器（`h-full` 或 `calc(100vh - header)`）
- 内部消息区域 `overflow-y: auto` 自带滚动条
- 底部输入框固定在容器底部
- 新消息自动滚到底部

### 16. Tool 消息折叠

**现在：** Tool result 平铺展示，占大量空间。

**改为：**
- Tool 消息默认折叠，只显示一行：`🔧 screenshot` 或 `🔧 shell`
- 点击展开，显示：
  - Tool 输入参数（JSON 格式）
  - Tool 执行结果（文本/图片）
- 用 DaisyUI 的 `collapse` 组件

### 17. 发消息快捷键

**现在：** Enter 发送。

**改为：** Ctrl+Enter 发送，Enter 换行。避免误触。

### 19. 标题栏去掉 "active"

**现在：** 标题栏显示 "🤖 alice active"。

**改为：** 只显示 "🤖 alice"。状态已在左侧列表显示，不需要重复。

### 20. Log tab 放到 Details 的 tabs 里

CC 第三轮未完成的 Bug 13（Agent Log tab），现在放到 Details 视图的 tab 分组中。

---

## 实施约束

1. **Bug 21 最先修**——这是 critical，Agent 是瞎子
2. **Bug 22 次之**——需要后端 + 前端配合
3. **Bug 14-20 前端重构最后**——改动大但不影响核心功能
4. 每步 cargo test
5. 不做兼容，不保留旧代码
6. 前端用现有 DaisyUI 组件，保持一致风格
