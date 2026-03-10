# see-agent v3 前端 PRD

> 版本：v3.0 | 作者：草莓🍓 + lanxuan | 日期：2026-03-10
> 目标：为 see-agent 构建 Web 管理界面（像素办公室 + 管理面板）

---

## 1. 项目概述

### 1.1 一句话

see-agent 的 Web 管理界面：通过像素办公室直观展示 agent 团队协作，同时提供完整的 team/agent/config 管理能力。

### 1.2 技术栈

| 类别 | 选型 | 说明 |
|------|------|------|
| 框架 | React 19 + TypeScript | SPA |
| 构建 | Vite | 快 |
| UI 组件 | shadcn/ui + Radix | 可定制、无侵入 |
| 样式 | Tailwind CSS 4 | 原子化 CSS |
| 像素引擎 | Phaser 3 | 2D 游戏引擎，渲染像素办公室 |
| 表单 | @rjsf/core + ajv8 | JSON Schema 驱动的配置表单 |
| 路由 | React Router v7 | |
| HTTP | axios | |
| 实时通信 | 原生 WebSocket | 已有后端 WS 支持 |
| 图标 | lucide-react | |

### 1.3 项目位置

Monorepo：`/web` 目录（在 see-agent 仓库内）

```
computer-use-and-memory-agent/
├── see_agent/          ← Python 后端
├── web/                ← 前端（本 PRD）
│   ├── src/
│   ├── public/
│   ├── package.json
│   ├── vite.config.ts
│   └── ...
├── docs/
└── ...
```

---

## 2. 全局布局

### 2.1 桌面端（≥1024px）

```
┌──────────────────────────────────────────────┐
│  Logo + 版本号                    [主题切换]  │  ← 顶栏 (48px)
├─────────┬────────────────────────────────────┤
│         │                                    │
│ 📮 聊天  │                                    │
│  Teams  │         主内容区                    │
│         │                                    │
│ 📊 控制  │                                    │
│  Dash-  │                                    │
│  board  │                                    │
│         │                                    │
│ 🤖 代理  │                                    │
│  Agents │                                    │
│  Skills │                                    │
│  MCP    │                                    │
│         │                                    │
│ ⚙️ 设置  │                                    │
│  Config │                                    │
│  Logs   │                                    │
│         │                                    │
├─────────┴────────────────────────────────────┤
```

- **左侧导航**：固定宽度 200px，分类标题为小字灰色，栏目项可点击高亮
- **顶栏**：左侧 see-agent logo + 版本号（如 `v2.5.0`），右侧主题切换按钮
- **主内容区**：填满剩余空间

### 2.2 移动端（<1024px）

- 左侧导航**默认收起**，顶栏左侧加汉堡菜单按钮 ☰
- 点击展开为覆盖层（overlay），选择栏目后自动收起
- 主内容区全宽
- 像素办公室在移动端**可横向滑动**查看

### 2.3 主题 & 配色

- 支持三种：**暗色 / 亮色 / 跟随系统**
- 默认：跟随系统
- 像素办公室场景：**暖色调**（日式像素游戏风格，现代办公物品）
- 切换按钮在顶栏右侧（太阳/月亮图标）

**配色方案**（参考 OpenClaw Control UI 风格）：

#### 暗色主题

| 变量 | 色值 | 用途 |
|------|------|------|
| `--bg` | `#12141a` | 页面背景 |
| `--bg-elevated` | `#1a1d25` | 提升层背景 |
| `--bg-hover` | `#262a35` | hover 状态 |
| `--card` | `#181b22` | 卡片背景 |
| `--text` | `#e4e4e7` | 正文文字 |
| `--text-strong` | `#fafafa` | 强调文字 |
| `--muted` | `#71717a` | 弱化文字/分类标题 |
| `--border` | `#27272a` | 边框 |
| `--border-strong` | `#3f3f46` | 强调边框 |
| `--accent` | `#ff5c5c` | 🔴 **主色（珊瑚红）**，高亮/选中/链接 |
| `--accent-hover` | `#ff7070` | 主色 hover |
| `--accent-subtle` | `rgba(255,92,92,0.15)` | 主色浅底（选中项背景） |
| `--accent-2` | `#14b8a6` | 第二强调色（青绿） |
| `--ok` | `#22c55e` | 成功/完成 |
| `--warn` | `#f59e0b` | 警告 |
| `--danger` | `#ef4444` | 错误/危险 |

#### 亮色主题

| 变量 | 色值 | 用途 |
|------|------|------|
| `--bg` | `#fafafa` | 页面背景 |
| `--bg-elevated` | `#ffffff` | 提升层 |
| `--card` | `#ffffff` | 卡片 |
| `--text` | `#3f3f46` | 正文 |
| `--text-strong` | `#18181b` | 强调文字 |
| `--muted` | `#71717a` | 弱化文字 |
| `--border` | `#e4e4e7` | 边框 |
| `--accent` | `#dc2626` | 🔴 **主色（深红）** |
| `--accent-2` | `#0d9488` | 第二强调色（深青绿） |

#### 字体

```css
--font-body: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
--mono: "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
```

#### 圆角 & 阴影

```css
--radius: 8px;
--radius-sm: 6px;
--radius-lg: 12px;
--shadow-sm: 0 1px 2px rgba(0,0,0,0.2);
--shadow-md: 0 4px 12px rgba(0,0,0,0.25);
```

---

## 3. 页面详情

### 3.1 Teams 列表页

**路由**：`/teams`

**布局**：卡片网格（2~3 列），每张卡片展示：
- Team 名称
- 成员数量 + 头像/emoji 缩略
- 状态标签：`created` | `running` | `completed` | `failed` | `stopped`
- 创建时间
- 未读消息红点（如果 owner 有未读）

**操作**：
- 点击卡片 → 进入 Team 详情页
- 右上角 「+ 新建 Team」按钮 → 弹出创建表单

---

### 3.2 Team 详情页 ⭐（核心页面）

**路由**：`/teams/:id`

**布局**：上下两部分

```
┌──────────────────────────────────────────────┐
│ ← 返回   Team 名称            [⚙️设置] [▶运行] │  ← 子顶栏
├──────────────────────────────────────────────┤
│                                              │
│            🏢 像素办公室（Phaser 3）            │  ← 上半部分 60~70%
│                                              │
│   🧑‍💻 leader    👩‍💻 alice     👨‍💻 bob            │
│   [敲键盘]      [坐着]       [敲键盘]          │
│                                              │
├──────────────────────────────────────────────┤
│ [任务看板]                    [💬 消息]        │  ← 下半部分 30~40%
│                                              │
│ ┌─pending──┐ ┌─claimed──┐ ┌──done───┐       │
│ │ 设计首页  │ │ 写API    │ │ 定需求 ✓│       │
│ │          │ │ → alice  │ │         │       │
│ └──────────┘ └──────────┘ └─────────┘       │
│                                              │
│ 💬 消息框                                     │
│ [选择对象 ▼ leader]  输入框...        [发送]  │
└──────────────────────────────────────────────┘
```

#### 3.2.1 像素办公室（上半部分）

**引擎**：Phaser 3，嵌入 React 组件

**办公室规格**：
- **3 人办公室**：members ≤ 3 时使用
- **6 人办公室**：4~6 人时使用
- **12 人办公室**：7~12 人时使用

每种规格是一张**预制像素背景图**，包含：
- 办公桌、椅子、电脑、植物、窗户等现代办公物品
- 固定的座位点（坐标预设）

**Agent Sprite**：
- 每个 agent 一个像素角色 sprite，坐在其分配的座位上
- 座位信息从 `team.json` 的 `seating` 字段读取
- 首次加入 team 时自动分配空座位

**状态表现**：

> ⚠️ v3.0 暂不实现 agent 状态区分（后端尚未支持）。所有 agent sprite 统一显示 idle 状态（坐着不动）。后续后端支持 status 后再加动画。

| 条件 | 状态 | 视觉表现 | 版本 |
|------|------|---------|------|
| 在 team 内 | 🔵 idle | 坐着不动 | v3.0 |
| team 在 running，agent 有任务 | 🟢 working | 敲键盘动画 + 绿色圆点 | v3.1+ |
| team 在 running，agent 无任务 | 🟡 waiting | 坐着 + 黄色圆点 | v3.1+ |

**交互**：
- **点击 agent sprite** → 弹出**个人信息卡片**（浮层）：
  - 头像/emoji + 名称 + 角色
  - 当前状态
  - 当前任务（如果有）
  - 简易消息框（可以快速给这个 agent 发消息）
  - 「查看详情」链接 → 跳转到 Agents 页面该 agent 详情
- 办公室**纯展示**，不支持拖拽换座位（换座位在设置里改）

**像素风格参考**：
- 颜色：日式像素游戏风（温暖的木质色调 + 柔和光线）
- 物品：现代——MacBook、升降桌、咖啡杯、绿植、白板
- 风格参考：类似《脑叶公司》/ Kairosoft 开罗游戏的办公场景

#### 3.2.2 任务看板 + 消息（下半部分）

**左侧（约 60%）**：任务看板
- 三列看板：`Pending` → `Claimed/In Progress` → `Done`
- 每张任务卡片：标题、负责人、状态标签
- Owner 可以点击任务卡片查看详情（描述、结果）
- 不需要拖拽（任务状态由 agent 通过 tool 改变）

**右侧（约 40%）**：Boss 消息框
- 消息框左侧/顶部有**对象选择器**（下拉）：
  - 默认选中 `Team Leader`
  - 可切换为任意 agent
  - 选 "全体" 可以广播
  - 每个 agent 名称旁显示**未读红点**（该 agent 的未读消息数）
- 消息列表：显示 owner 和所选 agent 之间的对话（`GET /api/teams/{id}/messages?agent_id=xxx`）
- 底部输入框 + 发送按钮
- **切换对象时**：拉取该 agent 的历史消息 + 自动调 `mark_read(agent_id)` 标记已读
- **实时推送**：新消息通过 WebSocket 即时显示

#### 3.2.3 Team 设置

点击子顶栏 ⚙️ 按钮 → 弹出设置面板（右侧抽屉 Drawer）：
- Team 基本信息（名称编辑）
- 成员管理（添加/移除 agent）
- 座位安排（下拉选座位号）
- 配置覆盖（team.json 的 overrides，用 rjsf 渲染）
- 危险区域：删除 Team

---

### 3.3 Dashboard 页

**路由**：`/dashboard`

**布局**：数字卡片（v1 简洁版）

```
┌────────────┐  ┌────────────┐  ┌────────────┐
│  Teams     │  │  Agents    │  │  Tasks     │
│    3       │  │    5       │  │   12       │
│ 1 running  │  │ 4 in team  │  │ 2 pending  │
│ 2 done     │  │ 1 idle     │  │ 10 done    │
└────────────┘  └────────────┘  └────────────┘
```

- 3~4 张统计卡片，简洁数字 + 小字副信息
- 未来可扩展：token 用量图表、任务趋势图（本次不做）

---

### 3.4 Agents 页

**路由**：`/agents`

**布局**：表格/列表视图

| 名称 | 角色 | 所属 Team | 状态 | 操作 |
|------|------|----------|------|------|
| Alice | 前端工程师 | 周报任务 | 🟢 working | [编辑] [详情] |
| Bob | 后端工程师 | 周报任务 | 🔵 idle | [编辑] [详情] |
| Charlie | 测试 | — (待岗) | ⚪ unassigned | [编辑] [分配] |

**操作**：
- 右上角 「+ 新建 Agent」
- [编辑] → 弹出编辑面板（抽屉）：
  - 基本信息（name, role）
  - SOUL.md 编辑器（Markdown textarea，后续可换 Monaco）
  - 配置覆盖（agent.json 的 config_overrides，rjsf 渲染）
  - Tools 配置（denied 列表勾选）
  - MCP 配置（enabled/disabled 勾选）
  - Skills 配置
- [详情] → Agent 详情页
- [分配] → 下拉选 team，执行 assign API

**Agent 详情页**（`/agents/:id`）：
- 顶部：基本信息卡
- Tab 切换：
  - SOUL：SOUL.md 内容展示/编辑
  - 配置：rjsf 表单
  - Session 历史：该 agent 的历史 session 列表（点击可展开查看步骤和截图）
  - Memory：记忆内容查看（P2）

---

### 3.5 Skills 页

**路由**：`/skills`

**布局**：卡片网格

每张卡片：
- Skill 名称 + 描述
- 状态：✅ available / ❌ blocked (原因)
- 被哪些 agent 使用（小头像/数字）
- 操作：[查看详情] [删除]

**查看详情**：弹出全屏 Modal，展示 SKILL.md 全文（Markdown 渲染）

---

### 3.6 MCP 页

**路由**：`/mcp`

**布局**：列表

| 名称 | 类型 | Command | 使用者 | 操作 |
|------|------|---------|--------|------|
| filesystem | stdio | npx -y @mcp/fs | Alice, Bob | [测试] [删除] |

**操作**：
- 右上角「+ 添加 MCP Server」→ 表单
- [测试] → 调 POST /api/mcp/{name}/test，显示结果（成功/失败+错误信息）
- [删除] → 校验引用后删除

---

### 3.7 Config 页

**路由**：`/config`

**布局**：左侧表单 + 右侧 JSON 预览

```
┌─────────────────────┬──────────────────────┐
│                     │                      │
│   rjsf 渲染的表单    │   JSON 实时预览       │
│                     │    (只读/可编辑切换)   │
│   [LLM 配置]        │                      │
│   Model: [gpt-4o ▼] │   {                  │
│   API URL: [____]   │     "llm": {         │
│   API Key: [****]   │       "model": ...   │
│                     │     }                │
│   [通用配置]         │   }                  │
│   Max Steps: [50]   │                      │
│   Language: [zh ▼]  │                      │
│                     │                      │
│        [保存]       │                      │
└─────────────────────┴──────────────────────┘
```

- 左侧：rjsf 根据 `GET /api/schemas/config` 自动渲染
- **多语言**：Schema 只含数据约束（type/default/enum/min/max），不含中文 title。前端通过 i18n 映射文件翻译字段名（详见下方 9.1 节）
- 右侧：实时显示 JSON（可切换为可编辑模式，给高级用户直接改 JSON）
- 保存按钮 → `PUT /api/config`

---

### 3.8 Logs 页

**路由**：`/logs`

**布局**：日志查看器

```
┌──────────────────────────────────────────────┐
│ 日期: [2026-03-10 ▼]  级别: [ALL ▼]  [🔍搜索] │
├──────────────────────────────────────────────┤
│ 10:30:01  INFO   see_agent.server  Started   │
│ 10:30:05  WARN   see_agent.agent   timeout   │
│ 10:31:12  ERROR  see_agent.brain   API err   │
│ ...                                          │
│                                              │
│                              [加载更多 ↓]     │
└──────────────────────────────────────────────┘
```

- 日期选择器（默认今天）
- 级别过滤（ALL / DEBUG / INFO / WARNING / ERROR）
- 全文搜索
- 滚动加载（分页）
- 日志行按级别着色（ERROR 红色，WARNING 黄色）

---

## 4. 实时性设计

### 4.1 WebSocket（即时推送）

| 数据 | WS 端点 | 触发时机 |
|------|---------|---------|
| Agent 运行步骤 | `WS /api/ws/{task_id}` | 每个 step 完成 |
| Owner 消息 | `WS /api/ws/team/{team_id}/messages`（新增） | agent 发消息给 owner |
| 任务状态变更 | `WS /api/ws/team/{team_id}/tasks`（新增） | TaskBoard 变化 |

### 4.2 轮询（按需，5~10 秒）

**只在当前页面活跃时轮询**，切走即停止（组件 unmount 时清除 interval）。

| 数据 | 端点 | 触发条件 |
|------|------|---------|
| Dashboard 数字 | `GET /api/dashboard` | 用户在 Dashboard 页面时 |
| Agent 列表 | `GET /api/agents` | 用户在 Agents 页面时 |
| Team 列表 | `GET /api/teams` | 用户在 Teams 页面时 |

实现：`usePolling(url, intervalMs)` hook，mount 开始轮询，unmount 停止。

### 4.3 HTTP 拉取（一次性）

打开页面或刷新时，通过 HTTP 拉取完整数据（历史消息、日志等）。WebSocket 只推增量，不替代初始加载。

- 打开 Team 详情 → `GET /api/teams/{id}/messages` 拉历史消息 + 建立 WS 连接推增量
- 刷新页面 → 重新 HTTP 拉历史 + 重建 WS

---

## 5. 像素资源需求

### 5.1 办公室背景

需要 3 张像素背景图（PNG/WebP）：

| 规格 | 尺寸（参考） | 座位数 | 场景描述 |
|------|------------|--------|---------|
| 小型 | 480×320 | 3 | 一张长桌，3 个工位，窗户+绿植 |
| 中型 | 640×400 | 6 | 两排桌子，6 个工位，白板+咖啡机 |
| 大型 | 800×480 | 12 | 开放式办公室，3~4 排桌子，会议角 |

**风格**：俯视 45° 角（等轴视角），日式像素风暖色调，现代办公物品。

### 5.2 Agent Sprite

- 基础像素人物（16×24 或 32×48）
- **帧动画**：
  - idle：坐着不动（2 帧循环）
  - working：敲键盘（4 帧循环）
- **颜色变体**：至少 6 种发色/衣服配色，用于区分不同 agent
- 头顶状态小圆点：绿/黄（4×4 px）

### 5.3 UI 元素

- 消息气泡像素框（点击 agent 时弹出）
- 像素风信息卡片边框

> **资源获取**：可用 AI 生成（Midjourney/DALL-E 像素风格），或从 itch.io 购买现成素材包，或请像素画师定制。

---

## 6. 前端目录结构

```
web/
├── public/
│   ├── assets/
│   │   ├── offices/          ← 办公室背景图
│   │   │   ├── office-3.png
│   │   │   ├── office-6.png
│   │   │   └── office-12.png
│   │   └── sprites/          ← agent 像素角色
│   │       ├── agent-idle.png
│   │       ├── agent-working.png
│   │       └── ...
│   └── favicon.ico
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── router.tsx            ← 路由定义
│   ├── api/                  ← API 封装
│   │   ├── client.ts         ← axios 实例
│   │   ├── agents.ts
│   │   ├── teams.ts
│   │   ├── config.ts
│   │   ├── skills.ts
│   │   ├── mcp.ts
│   │   ├── logs.ts
│   │   └── ws.ts             ← WebSocket 封装
│   ├── components/
│   │   ├── layout/
│   │   │   ├── Sidebar.tsx
│   │   │   ├── Topbar.tsx
│   │   │   └── Layout.tsx
│   │   ├── ui/               ← shadcn/ui 组件
│   │   │   └── ...
│   │   ├── office/           ← 像素办公室
│   │   │   ├── PixelOffice.tsx      ← React 包装器
│   │   │   ├── OfficeScene.ts       ← Phaser Scene
│   │   │   ├── AgentSprite.ts       ← Phaser Sprite 类
│   │   │   └── AgentInfoCard.tsx    ← 弹出信息卡片
│   │   ├── team/
│   │   │   ├── TaskBoard.tsx
│   │   │   ├── MessageBox.tsx
│   │   │   └── TeamSettings.tsx
│   │   ├── agent/
│   │   │   ├── AgentTable.tsx
│   │   │   ├── AgentEditor.tsx
│   │   │   └── SoulEditor.tsx
│   │   └── config/
│   │       └── SchemaForm.tsx       ← rjsf 封装
│   ├── pages/
│   │   ├── TeamsPage.tsx
│   │   ├── TeamDetailPage.tsx
│   │   ├── DashboardPage.tsx
│   │   ├── AgentsPage.tsx
│   │   ├── AgentDetailPage.tsx
│   │   ├── SkillsPage.tsx
│   │   ├── McpPage.tsx
│   │   ├── ConfigPage.tsx
│   │   └── LogsPage.tsx
│   ├── hooks/
│   │   ├── useWebSocket.ts
│   │   ├── usePolling.ts
│   │   └── useTheme.ts
│   ├── stores/               ← 状态管理（zustand 或 context）
│   │   └── ...
│   ├── types/
│   │   ├── agent.ts
│   │   ├── team.ts
│   │   └── ...
│   └── styles/
│       └── globals.css
├── index.html
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
└── components.json           ← shadcn/ui 配置
```

---

## 7. CLI 精简

有了 Web UI 后，CLI 只保留安装配置类命令，交互/管理类命令全部由前端承担。

### 7.1 保留的命令

| 命令 | 说明 |
|------|------|
| `see-agent start` | **启动前后端 + 自动打开浏览器**（原 `serve` 改名） |
| `see-agent stop` | 停止服务（新增） |
| `see-agent config init` | 首次配置向导（交互式设置 API key 等） |
| `see-agent config show` | 查看当前配置（API key 脱敏） |
| `see-agent setup install` | 安装可选依赖（`--full` / `--memory` / `--mcp` / `--dev`） |
| `see-agent setup check` | 检查环境和依赖状态 |
| `see-agent version` | 显示版本号（新增） |

### 7.2 删除的命令

| 命令 | 原因 |
|------|------|
| `see-agent chat` / `quick chat` / `quick run` | 前端做 |
| `see-agent resume` | 前端做 |
| `see-agent agent create/list/show` | 前端做 |
| `see-agent team create/list/status/run/stop` | 前端做 |
| `see-agent sessions list/show/clean` | 前端做 |
| `see-agent mcp add/remove/list` | 前端做 |

### 7.3 `see-agent start` 实现

```python
@app.command()
def start(
    port: int = typer.Option(8000, "--port", "-p"),
    no_browser: bool = typer.Option(False, "--no-browser"),
):
    """启动 see-agent 服务并在浏览器中打开"""
    ensure_workspace()
    setup_logging()
    config = load_config()
    _validate_api_key(config)

    url = f"http://localhost:{port}"
    typer.echo(f"🚀 Starting see-agent on {url}")

    if not no_browser:
        # 延迟 1.5 秒后打开浏览器（等服务启动）
        import threading, webbrowser
        threading.Timer(1.5, lambda: webbrowser.open(url)).start()

    import uvicorn
    uvicorn.run("see_agent.server.app:app", host="0.0.0.0", port=port, reload=False)
```

---

## 8. 内置 Skill 与 ClawHub 生态

### 8.1 内置 Skill 机制

see-agent 包内自带一组内置 skill，首次启动（`ensure_workspace()`）时自动复制到用户目录：

```
see_agent/                      ← Python 包内（随 pip 安装）
├── builtin_skills/
│   └── clawhub/
│       └── SKILL.md

~/.see-agent/                   ← 用户目录
├── skills/                     ← 首次启动时从 builtin_skills 同步
│   ├── clawhub/                ← 内置，教 agent 使用 clawhub
│   │   └── SKILL.md
│   ├── some-community-skill/   ← 用户后续安装的
│   │   └── SKILL.md
│   └── ...
```

**同步逻辑**（在 `ensure_workspace()` 中）：

```python
def _sync_builtin_skills():
    """将包内 builtin_skills 同步到用户 skills 目录。
    只在目标不存在时复制（不覆盖用户修改）。
    """
    builtin_dir = Path(__file__).parent / "builtin_skills"
    if not builtin_dir.exists():
        return
    user_skills = SKILLS_DIR
    user_skills.mkdir(parents=True, exist_ok=True)
    for skill_dir in builtin_dir.iterdir():
        if skill_dir.is_dir():
            target = user_skills / skill_dir.name
            if not target.exists():
                shutil.copytree(skill_dir, target)
                logger.info("Installed builtin skill: %s", skill_dir.name)
```

### 8.2 ClawHub Skill 内容

内置的 `clawhub` skill 教 agent 如何从 ClawHub 搜索和安装 skill：

```markdown
---
name: clawhub
description: 从 ClawHub 搜索和安装 skill 到 see-agent。
---

## 搜索 Skill

在终端执行：
  clawhub search <keyword>

或浏览 https://clawhub.com 查找。

## 安装 Skill

  clawhub install <skill-name> --target ~/.see-agent/skills

安装完成后 skill 立即生效（下次 agent 对话时自动加载）。

## 查看已安装

  ls ~/.see-agent/skills/

每个子目录就是一个 skill，包含 SKILL.md。
```

### 8.3 前端 Skills 页面补充

Skills 页面除了展示已安装 skill，还提供安装入口：

```
┌──────────────────────────────────────────────┐
│ Skills                          [+ 安装 Skill] │
├──────────────────────────────────────────────┤
│ ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│ │ clawhub  │  │ browser  │  │ terminal │    │
│ │ 内置      │  │ ClawHub  │  │ ClawHub  │    │
│ │ ✅ active │  │ ✅ active │  │ ❌ blocked│    │
│ └──────────┘  └──────────┘  └──────────┘    │
└──────────────────────────────────────────────┘
```

点击 「+ 安装 Skill」弹出 Modal：

```
┌────────────────────────────────────────┐
│ 安装 Skill                             │
│                                        │
│ 安装方式:                               │
│   ○ 从 ClawHub 安装（推荐）              │
│   ○ 手动添加                            │
│                                        │
│ ── ClawHub 安装 ──                      │
│ Skill 名称: [open-browser         ]    │
│                                        │
│              [安装]                     │
│                                        │
│ ── 手动添加 ──                          │
│ 将 SKILL.md 所在目录放到:               │
│ ~/.see-agent/skills/ 即可               │
└────────────────────────────────────────┘
```

**ClawHub 安装**后端实现：

```python
# 新增 API: POST /api/skills/install
@router.post("/api/skills/install")
async def install_skill(request: InstallSkillRequest):
    """从 ClawHub 安装 skill"""
    # 执行: clawhub install <name> --target ~/.see-agent/skills
    result = subprocess.run(
        ["clawhub", "install", request.name, "--target", str(SKILLS_DIR)],
        capture_output=True, text=True, timeout=60,
    )
    if result.returncode != 0:
        raise HTTPException(400, f"安装失败: {result.stderr}")
    return {"status": "ok", "name": request.name}
```

---

## 9. MCP 安装方案

### 9.1 前端 MCP 页面：添加 MCP Server

点击「+ 添加 MCP Server」弹出 Modal：

```
┌────────────────────────────────────────┐
│ 添加 MCP Server                        │
│                                        │
│ 安装方式:                               │
│   ◉ npm 包      ○ pip 包     ○ 手动    │
│                                        │
│ ── npm 包 ──                            │
│ 包名: [@modelcontextprotocol/server-fs ]│
│ 参数: [/Users/lanxuan/Documents     ]  │
│ 名称: [filesystem                    ] │
│ (名称自动从包名推断，可修改)              │
│                                        │
│              [安装并添加]                │
│                                        │
│ ── pip 包 ──                            │
│ 包名: [mcp-server-sqlite             ] │
│ 参数: [--db /path/to/db.sqlite       ] │
│                                        │
│ ── 手动配置 ──                          │
│ 名称: [                              ] │
│ Command: [                            ] │
│ Args: [                               ] │
│ 环境变量:                               │
│   KEY: [          ]  VALUE: [        ] │
│   [+ 添加变量]                          │
│                                        │
│              [添加]                     │
└────────────────────────────────────────┘
```

### 9.2 三种安装方式的后端处理

**npm 包**：
- 不需要真正安装，npx 自动拉取
- 生成配置写入 config.json：

```json
{
  "mcp_servers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users/lanxuan/Documents"]
    }
  }
}
```

**pip 包**：
- 后端执行 `pip install <package>`（在 see-agent 的 venv 中）
- 生成配置：

```json
{
  "mcp_servers": {
    "sqlite": {
      "type": "stdio",
      "command": "python",
      "args": ["-m", "mcp_server_sqlite", "--db", "/path/to/db.sqlite"]
    }
  }
}
```

**手动**：
- 用户直接填 command / args / env
- 写入 config.json

### 9.3 新增 API

```python
# POST /api/mcp/install
class InstallMcpRequest(BaseModel):
    name: str                          # MCP server 名称
    install_type: str                  # "npm" | "pip" | "manual"
    package: str | None = None         # npm/pip 包名
    params: str | None = None          # 额外参数（如路径）
    command: str | None = None         # 手动模式: command
    args: list[str] | None = None      # 手动模式: args
    env: dict[str, str] | None = None  # 手动模式: env vars

@router.post("/api/mcp/install")
async def install_mcp(request: InstallMcpRequest):
    config = load_config()
    if "mcp_servers" not in config:
        config["mcp_servers"] = {}

    if request.install_type == "npm":
        # npm 包用 npx，不需要安装
        server_cfg = {
            "type": "stdio",
            "command": "npx",
            "args": ["-y", request.package] + (request.params.split() if request.params else []),
        }
    elif request.install_type == "pip":
        # pip 包需要先安装
        result = subprocess.run(
            [sys.executable, "-m", "pip", "install", request.package],
            capture_output=True, text=True, timeout=120,
        )
        if result.returncode != 0:
            raise HTTPException(400, f"pip install 失败: {result.stderr}")
        # 推断 module name（mcp-server-xxx → mcp_server_xxx）
        module_name = request.package.replace("-", "_")
        server_cfg = {
            "type": "stdio",
            "command": "python",
            "args": ["-m", module_name] + (request.params.split() if request.params else []),
        }
    elif request.install_type == "manual":
        server_cfg = {
            "type": "stdio",
            "command": request.command,
            "args": request.args or [],
        }
        if request.env:
            server_cfg["env"] = request.env
    else:
        raise HTTPException(400, f"未知安装类型: {request.install_type}")

    config["mcp_servers"][request.name] = server_cfg
    save_config(config)
    return {"status": "ok", "name": request.name, "config": server_cfg}
```

### 9.4 MCP 搜索（v3.1+ 路线图）

v3.0 不做搜索，用户需要知道包名。v3.1+ 可选方案：

- **方案 A**：内置 MCP 注册表（`see_agent/data/mcp_registry.json`），包含常用 MCP server 列表，前端本地搜索
- **方案 B**：对接 Smithery.ai API（MCP 的包注册中心，正在发展中）
- **方案 C**：直接搜 npmjs.com / pypi.org API（按前缀过滤）

---

## 10. 后端配合项

### 10.1 新增 WebSocket 端点

| 端点 | 说明 |
|------|------|
| `WS /api/ws/team/{team_id}/messages` | 实时推送 owner 相关消息 |
| `WS /api/ws/team/{team_id}/tasks` | 实时推送任务状态变更 |

### 10.2 CORS 配置

后端 FastAPI 需要配置 CORS，允许前端开发服务器访问：

```python
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:5173"],  # Vite dev server
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)
```

### 10.3 静态文件服务

生产环境下，后端 serve 前端 build 产物：

```python
# app.py
app.mount("/", StaticFiles(directory="web/dist", html=True), name="frontend")
```

### 10.4 API 依赖

本 PRD 依赖 `v2.5-frontend-backend-report.md` 中定义的所有 P0/P1 API。

此外还需新增：

| API | 方法 | 说明 |
|-----|------|------|
| `POST /api/skills/install` | POST | 从 ClawHub 安装 skill |
| `POST /api/mcp/install` | POST | 安装 MCP server（npm/pip/手动） |

---

## 11. 实施计划

### Phase 1 — 骨架 + 核心页面（3~4 天）

| # | 任务 | 预估 |
|---|------|------|
| 1 | Vite + React + TypeScript + Tailwind + shadcn/ui 项目初始化 | 1h |
| 2 | 全局布局（Sidebar + Topbar + Layout + 路由） | 2h |
| 3 | 主题切换（暗/亮/系统）+ 完整配色变量 | 1h |
| 4 | 移动端响应式（Sidebar 折叠） | 1h |
| 5 | API 封装层 + WebSocket hook + usePolling hook | 2h |
| 6 | Teams 列表页 | 2h |
| 7 | Dashboard 页（数字卡片） | 1h |
| 8 | Agents 页（表格 + CRUD） | 3h |
| 9 | Config 页（rjsf 表单 + JSON 预览 + i18n） | 2h |
| 10 | Logs 页（日志查看器） | 2h |

### Phase 2 — 像素办公室 + Team 详情（3~4 天）

| # | 任务 | 预估 |
|---|------|------|
| 11 | 像素资源准备（办公室背景 + sprite） | 4h |
| 12 | Phaser 3 集成到 React | 2h |
| 13 | PixelOffice 组件（背景 + 座位点 + agent 加载） | 4h |
| 14 | Agent sprite idle 动画 | 1h |
| 15 | 点击 agent → 信息卡片 + 快速消息 | 2h |
| 16 | TaskBoard 看板组件 | 3h |
| 17 | MessageBox 消息框（对象选择 + 未读红点 + 实时消息） | 3h |
| 18 | Team 设置抽屉 | 2h |

### Phase 3 — 补全 + 生态（2~3 天）

| # | 任务 | 预估 |
|---|------|------|
| 19 | Skills 页 + ClawHub 安装 Modal | 3h |
| 20 | MCP 页 + 三种安装方式 Modal | 3h |
| 21 | Agent 详情页（Tab: SOUL + 配置 + Session 历史） | 3h |
| 22 | 生产构建 + 后端静态文件 serve | 1h |
| 23 | 移动端测试 + 微调 | 2h |
| 24 | 全局 loading/error/empty 状态处理 | 2h |

### Phase 4 — CLI 精简 + 内置 Skill（1 天）

| # | 任务 | 预估 |
|---|------|------|
| 25 | CLI 删除多余命令，`serve` → `start`，新增 `stop` + `version` | 2h |
| 26 | 创建 `builtin_skills/clawhub/SKILL.md` | 30min |
| 27 | `ensure_workspace()` 增加内置 skill 同步逻辑 | 30min |
| 28 | `POST /api/skills/install` + `POST /api/mcp/install` 后端实现 | 2h |

---

## 12. 设计约束

1. **team 是唯一运行单元**：所有操作（运行、消息、任务）都在 team 粒度
2. **Agent 状态不存储**：v3.0 暂不实现状态区分，后续后端支持后再加
3. **目录即数据库**：前端通过 API 操作，后端操作文件系统，无外部数据库
4. **配置表单统一**：config / agent / team 三种 JSON Schema + rjsf，同一套组件
5. **像素办公室是展示层**：不改变数据，只可视化状态 + 触发信息卡片
6. **前端不直接操作文件**：所有 CRUD 通过后端 API
7. **Schema 从后端获取**：`GET /api/schemas/{type}`，前端不硬编码字段
8. **未读状态按 team/agent 分列**：每个 team 的 `read_state.json` 按 agent 记录 last_read_ts

### 12.1 Schema 多语言策略

**原则：Schema 只做数据约束，不做 UI 文案。多语言由前端处理。**

Schema 示例（后端提供）：
```json
{
  "max_steps": {
    "type": "integer",
    "minimum": 1,
    "maximum": 200,
    "default": 50
  }
}
```

前端 i18n 映射（`locales/zh.json`）：
```json
{
  "config.llm.base_url": "API 地址",
  "config.llm.api_key": "API 密钥",
  "config.llm.model": "模型",
  "config.max_steps": "最大步数",
  "config.language": "语言"
}
```

rjsf 通过自定义 `TitleField` 组件注入翻译：
```typescript
const CustomTitleField = ({ title, id }) => {
  const path = id.replace("root_", "config.").replaceAll("_", ".");
  return <label>{t(path) || title}</label>;
};
```

初始版本只做中文，后续加英文。
