# Round 11: Chat 交互优化 + send_message 支持优先级 + 前端重构

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 57 (P1): Chat 自动滚动优化

### 现象
Chat 界面定时自动滚到底部，无法查看历史消息。

### 修复
1. **删除定时自动滚底**
2. **新增"滚到底部"悬浮按钮**（居中，底部偏上），只在不在底部时显示
3. **粘性滚动**：如果用户当前滚动位置在底部附近（距底部 < 100px），新消息到达时自动滚到底部。如果用户在查看历史（距底部 > 100px），不强制滚动。

```javascript
// 伪代码
onNewMessage() {
    if (scrollContainer.scrollHeight - scrollContainer.scrollTop - scrollContainer.clientHeight < 100) {
        scrollToBottom();  // 粘性：接近底部就跟随
    }
    // 否则不动，让用户看历史
}
```

---

## Bug 58 (P2): send_message tool 支持 collect/steer

### 现象
Agent 间用 send_message 发消息只能发 collect。有时 leader 需要发加急消息。

### 修复
`send_message` tool 的参数新增可选的 `priority` 字段：

```json
{
    "to": "designer",
    "content": "紧急停止当前任务",
    "priority": "steer"   // 可选，默认 "collect"
}
```

在 `team.rs` 的 `SendMessageTool` 中读取 priority 参数，传给 `send_to_inbox_with_id`。

---

## Bug 59 (P1): Agent 页面标题栏简化 + 布局调整

### 现象
右侧标题栏 "🤖 developer" 重复显示（左侧列表已有），Chat/Details 按钮在右边。

### 修复
1. 删掉标题栏的 emoji + agent name
2. Chat/Details 切换按钮移到左边
3. 标题栏只保留 Chat/Details 按钮，紧凑排列

---

## Bug 60 (P1): Team 详情页重构

### 现象
`/teams/{id}` 页面功能简陋。

### 修复

**布局：** 和 /agents 页面类似的主从结构

**左侧：成员列表面板**
- 每个成员显示：emoji + name + 状态（sleeping/active）+ team 角色（leader/developer/tester 等）
- leader 有特殊标识（👑 或 star icon）
- 点击成员跳转到该 agent 的 chat 页面

**右侧：主内容区**

Tab 切换：Overview / Task Board / Messages / Shared

**Task Board tab：**
- 任务卡片展示，每个卡片显示：title、status badge、assigned_to、depends_on
- **依赖关系树形展示**：
  - 无依赖的任务在根节点
  - 有依赖的任务显示为子节点，用连线连接
  - 用 DaisyUI 的嵌套列表 + 缩进，或简单的树形 ASCII 风格
  - 如果太复杂可以先做平铺列表 + depends_on 标签

**Overview tab：**
- Team 基本信息（name、id、status、created_at）
- 成员数量、任务统计（pending/claimed/done）

**Messages tab：**
- Team 公共消息（`teams/{id}/messages.jsonl`）

**Shared tab：**
- 共享文件列表（`teams/{id}/shared/`）

---

## 实施约束

1. 顺序：Bug 57 → Bug 58 → Bug 59 → Bug 60
2. Bug 60 改动最大，如果时间不够先做左侧成员列表 + Task Board，Messages 和 Shared 可以简单展示
3. 每步 cargo test
4. 最后 trunk build --release + git commit + git push
