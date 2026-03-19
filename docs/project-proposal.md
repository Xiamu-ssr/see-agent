# Claw Race — 多智能体桌面协作系统

## 项目说明书

---

### 一、项目概述

**Claw Race** 是一个基于 Rust 构建的多智能体桌面 AI 系统。多个自主 AI Agent 以团队形式协作，通过视觉感知操作 Mac 电脑，完成用户分配的复杂任务。

**核心理念：** 让 AI Agent 不再是单兵作战，而是像人类团队一样分工协作——有领导、有成员、有任务看板、有沟通频道。每个 Agent 拥有独立的人格、记忆和技能，运行在安全沙箱中。

**一句话描述：** 一个有螯生物🦞🦀🦐组成的 AI 团队，住在你的 Mac 上，帮你干活。

---

### 二、核心能力

#### 2.1 多 Agent 管理

每个 Agent 是一个独立进程，拥有：
- **身份**（IDENTITY.md）：名称、Emoji、Race（有螯生物种族 🦞🦀🦐🦑🦂）
- **人格**（SOUL.md）：行为风格、语言偏好
- **记忆**（memory/）：长期记忆 + 日记，跨会话持久化
- **技能**（skills/）：可插拔的能力模块，按需加载
- **会话**（session/）：对话历史、截图、LLM 调用记录

系统内置管家 Agent **🦞 Steward**，负责帮助用户通过自然语言管理整个系统（创建 Agent、组建团队、分配任务）。

[插图 1：Agent 详情页截图 — 展示 Chat 视图和 Details 视图（Info tab 显示 Race、Sandbox 权限卡片）]

#### 2.2 团队协作

Agent 可以被组织成 Team：
- **Leader** 负责创建任务、分配工作、协调沟通
- **Member** 领取任务、执行、汇报进度
- **Task Board** 支持任务依赖关系（A 完成后 B 才能领取）
- **Team 消息频道** 记录团队内所有通信
- **共享工作空间** 团队成员可共享文件

#### 2.3 视觉感知与桌面操作

Agent 具备完整的 Mac 桌面操作能力：
- **截屏**：获取当前屏幕画面，支持 Retina 缩放
- **鼠标操作**：点击、双击、拖拽、滚动
- **键盘输入**：打字、快捷键
- **Shell 命令**：执行终端命令
- **文件操作**：读写文件

截图在上下文中有四级消退策略：最新 3 张高清 → 次新 3 张低清 → 更早的用文字占位 → 压缩时丢弃。保证 Agent 始终能看到近期屏幕，同时不浪费 Token。

#### 2.4 安全沙箱

每个 Agent 进程运行在 macOS 安全沙箱（Agent Safehouse）中：
- **deny-first 模型**：默认禁止一切，只开放必要权限
- **文件隔离**：Agent 只能读写自己的目录和团队共享目录
- **System Agent** 拥有整个工作目录的权限
- **权限拦截实时告警**：被沙箱拦截的操作记录到日志并在前端标红

---

### 三、系统架构

#### 3.1 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust + Axum + Tokio（异步运行时） |
| 前端 | Leptos（Rust → WASM）+ DaisyUI + Tailwind |
| LLM | OpenAI 兼容协议（支持任意 provider） |
| 沙箱 | Agent Safehouse（macOS sandbox-exec） |
| 构建 | Cargo + Trunk（WASM）+ GitHub Actions |

#### 3.2 进程模型

```
see-agent-corp serve (主进程)
├── HTTP Server (Axum)              ← 前后端一体，serve API + WASM 前端
├── Supervisor                      ← 管理所有 Worker 子进程
│   ├── Worker: system (🦞 Steward) ← 每个 Agent 一个独立进程
│   ├── Worker: developer (🦀)
│   ├── Worker: designer (🦐)
│   └── Worker: tester (🦑)
└── Config Watcher                  ← 监听 config.json 变化，热加载
```

每个 Worker 进程：
1. 被 Safehouse 沙箱包裹（进程级隔离）
2. 独立的 LLM 推理循环（ReAct 模式）
3. 通过 inbox 文件通信（SIGUSR1 信号唤醒）
4. Session restore 支持进程崩溃恢复

#### 3.3 消息流转

```
用户/Agent 发消息
    ↓
写入目标 Agent 的 inbox.jsonl
    ↓
SIGUSR1 唤醒 Worker 进程
    ↓
Worker drain inbox（双游标机制）
  ├── collect 消息 → 外层循环批量处理
  └── steer 消息 → 推理循环内即时注入（下一次 LLM 调用前）
    ↓
LLM 推理 → Tool 执行 → LLM 推理 → ...
    ↓
回复写入 session/messages.jsonl
    ↓
前端轮询展示
```

[插图 2：架构图 — 展示主进程、Supervisor、Worker 进程关系，inbox 消息流转，Safehouse 沙箱包裹]

---

### 四、上下文管理

#### 4.1 四层压缩机制

| 层级 | 触发条件 | 操作 | 影响 |
|------|---------|------|------|
| Layer 1 | Tool 返回时 | 截断过长输出（Shell 30K，Read 50K 字符） | 减少单条消息体积 |
| Layer 2 | Token 达 30% | Microcompact：旧 tool_result 替换为占位文字 | 只改内存，可恢复 |
| Layer 3 | Token 达 95% | Full Compact：LLM 生成摘要，保留最近 8 条 | 不可逆，写入持久化 |
| Layer 4 | 图片消退 | 最新 3 张高清 → 3 张低清 → 占位 → 丢弃 | 只影响发给 LLM 的副本 |

Compact 前系统会提醒 Agent 先将重要信息写入记忆文件。

#### 4.2 System Prompt 组装

```
1. IDENTITY.md        — 身份信息（名称、种族）
2. AGENTS.md          — 操作规范
3. SOUL.md            — 人格设定
4. MEMORY.md          — 长期记忆
5. 安全约束            — max_steps、安全规则
6. Skills（按需）      — 只注入 name + description + location
7. Team Context（可选）— Leader/Member 角色模板 + 成员列表
```

Skills 采用懒加载：System Prompt 只包含 Skill 的名称和简介，Agent 需要时自己用 read 工具读取完整指南。

---

### 五、工具系统

#### 5.1 内置工具分组

| 分组 | 工具 | 说明 |
|------|------|------|
| **Core** | shell, read, write | 基础操作 |
| **Screen** | screenshot, mouse_click, keyboard_type... | 桌面操作 |
| **Memory** | memory_search, memory_get | 记忆检索 |
| **Team** | send_message, create_task, assign_task, claim_task, complete_task... | 团队协作 |

#### 5.2 扩展机制

- **Skills**：可插拔的能力模块，放到 `skills/` 目录即可被发现
- **MCP Servers**：通过 Model Context Protocol 接入外部工具
- **ClawHub**：内置的 Skill 市场，搜索和安装社区 Skills

---

### 六、Web UI

Claw Race 提供完整的 Web 管理界面，前端使用 Leptos（Rust → WASM）构建，与后端编译为单一二进制文件。

[插图 3：Dashboard 截图 — 展示系统概览、Agent 数量、Team 数量、沙箱状态]

**主要页面：**

| 页面 | 功能 |
|------|------|
| **Dashboard** | 系统概览、Agent/Team 统计、沙箱状态 |
| **Agents** | Agent 列表（按 Team 分组）、Chat 聊天、Details 详情 |
| **Teams** | 团队管理、成员列表、Task Board（依赖关系树）、共享文件 |
| **Config** | 基于 JSON Schema 的智能配置编辑器 |
| **Skills** | 全局 Skill 列表、启用/禁用 |
| **Tools** | 分组展示所有可用工具 |
| **MCP** | MCP Server 管理 |
| **Logs** | 系统日志（等级着色） |

**Chat 界面特色：**
- 粘性滚动（接近底部时跟随新消息，查看历史时不打断）
- Tool 调用默认折叠（点击展开输入参数和执行结果）
- Markdown 渲染（代码块、表格、列表）
- 消息来源标签（用户 / Agent 名称）
- 普通/加急 消息优先级

---

### 七、配置体系

三层配置，同构 deep merge：

```
config.json（全局默认）
    ↓ merge
  team.json（团队覆盖）
    ↓ merge
    agent.json（Agent 覆盖）
```

合并规则：dict 递归合并，数组直接覆盖。

核心配置项：

| 分类 | 字段 | 说明 |
|------|------|------|
| LLM | base_url, api_key, model | 支持任意 OpenAI 兼容 API |
| 压缩 | context_window, microcompact_ratio, full_compact_ratio | 四层压缩参数 |
| 沙箱 | sandbox.enabled | 启用/关闭进程隔离 |
| 工具 | tools.disabled | 按名称禁用工具 |
| 技能 | skills.dirs, skills.disabled | 额外搜索路径、按名称禁用 |

支持热加载——修改 config.json 后 Worker 自动检测并重建 LLM 连接和 Prompt。

---

### 八、安装与使用

#### 快速安装

```bash
curl -fsSL https://raw.githubusercontent.com/Xiamu-ssr/see-agent/main/scripts/install.sh | bash
```

自动检测平台（macOS arm64/x86_64, Linux），下载对应二进制，安装 Safehouse 沙箱依赖。

#### 启动

```bash
see-agent-corp start --port 28789
# 打开 http://localhost:28789
```

首次启动自动初始化工作目录 `~/.see-agent-corp/`，创建系统管家 🦞 Steward。

#### 典型工作流

1. 在 Web UI Config 页面配置 LLM（API Key + Model）
2. 与 🦞 Steward 对话："帮我创建一个开发团队，3 个成员"
3. Steward 通过 CLI 创建 Agent + Team
4. 在 Team 页面查看 Task Board，Leader 分配任务
5. 成员 Agent 自动领取并执行任务（截屏、写代码、跑命令）
6. 成员完成后汇报，Leader 汇总结果

---

### 九、项目状态与规划

#### 当前版本：v0.4.0

- ✅ 多 Agent 创建、管理、删除
- ✅ Team 协作（Leader/Member、Task Board、依赖关系）
- ✅ 桌面视觉操作（截屏 + 鼠标键盘）
- ✅ 四层上下文压缩
- ✅ 双游标 Steer 实时消息注入
- ✅ 进程级安全沙箱（macOS Safehouse）
- ✅ Web UI 全功能管理界面
- ✅ Config 热加载
- ✅ Skills 懒加载 + ClawHub 生态

#### 后续规划

| 阶段 | 内容 |
|------|------|
| v0.5 | MCP Server 完整接入、Memory Flush 自动触发 |
| v0.6 | Linux 沙箱（Landlock）、跨平台完善 |
| v1.0 | Tool 命名空间、远程 Agent 节点、生产级稳定性 |

---

### 十、团队与致谢

- **架构设计 & PM**：lanxuan
- **AI 协作**：🍓 草莓（OpenClaw Agent，设计报告 + 架构决策 + 代码审查）
- **核心编码**：Claude Code（15 轮迭代，从零到完整系统）
- **参考项目**：OpenClaw、Anthropic computer-use-demo、Agent Safehouse

---

*Claw Race — Where autonomous agents race to get things done. 🦞🦀🦐*
