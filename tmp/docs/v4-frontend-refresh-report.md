# v4 前端翻新：shadcn/ui + Monaco Editor

## 目标
将 see-agent 前端从手写 div+tailwind 升级为 shadcn/ui 组件库 + Monaco 代码编辑器。

## 工作目录
`/Users/lanxuan/Code/computer-use-and-memory-agent/web`

## 当前状态
- React 19 + Vite + TailwindCSS + React Router 7
- 已有 shadcn 基础依赖：`class-variance-authority`, `clsx`, `tailwind-merge`
- 已有 `cn()` 工具函数在 `src/components/ui/cn.ts`
- 暗色主题，主色 `#ff5c5c`（珊瑚红）
- 42 个 .tsx/.ts 源文件

## Phase 1: 初始化 shadcn/ui

1. 运行 `npx shadcn@latest init` —— 选 New York style, Zinc base, CSS variables
2. 配置 `components.json`，确保 aliases 匹配现有 `@/` 路径
3. 安装需要的 shadcn 组件：
   ```bash
   npx shadcn@latest add button card switch tabs badge input textarea dialog tooltip scroll-area separator
   ```
4. 保持现有暗色主题变量，在 shadcn 的 CSS variables 里映射

## Phase 2: 安装 Monaco Editor

1. `npm install @monaco-editor/react`
2. 创建 `src/components/ui/CodeEditor.tsx` 封装 Monaco：
   - props: `value`, `onChange`, `language`, `readOnly`
   - 暗色主题（vs-dark）
   - 根据文件后缀自动检测语言：`.py`→python, `.json`→json, `.md`→markdown, `.yaml`→yaml, `.toml`→toml, `.ts`→typescript, `.tsx`→typescriptreact, `.js`→javascript, `.sh`→shell
   - 高度 100% 撑满父容器

## Phase 3: 替换手写组件

### 3.1 Toggle.tsx → shadcn Switch
- 删除 `src/components/ui/Toggle.tsx`
- `AgentTools.tsx` 和 `AgentSkills.tsx` 用 shadcn `Switch` 替换

### 3.2 AgentFiles.tsx — Monaco 编辑器
- 右侧编辑区从 `<textarea>` 改为 `<CodeEditor>`
- 保留左侧文件树（可以用 shadcn ScrollArea 包裹）
- Cmd+S 保存快捷键保留

### 3.3 AgentChat.tsx
- 输入框用 shadcn `Input` 或 `Textarea`
- 发送按钮用 shadcn `Button`
- Steer checkbox 可以用 shadcn `Switch` + `Label`

### 3.4 AgentsPage.tsx
- Details/Chat 切换用 shadcn `Tabs`
- Details 子 tab（Overview/Files/Tools/Skills/Safehouse）也用 shadcn `Tabs`
- 卡片用 shadcn `Card`
- Create Agent modal 用 shadcn `Dialog`

### 3.5 Layout / Sidebar
- Sidebar 按钮用 shadcn `Button` variant="ghost"
- Tooltip 用 shadcn `Tooltip`

### 3.6 其他页面
- **DashboardPage**: 卡片用 `Card`
- **ConfigPage**: 表单输入用 shadcn `Input`、`Button`
- **LogsPage**: 用 `ScrollArea`、`Badge` 标记级别
- **TeamsPage / TeamDetailPage**: `Card`、`Button`、`Dialog`、`Badge`
- **McpPage / SkillsPage**: `Card`、`Button`、`Badge`

## Phase 4: 样式统一

1. 所有硬编码颜色（`#0d1117`, `#21262d`, `#30363d`, `#7d8590`, `#e6edf3`）改为 shadcn CSS variables
2. 去掉 inline `style={{}}` 尽量用 tailwind class + shadcn 变量
3. 主色 `#ff5c5c` 映射到 shadcn 的 `--primary`
4. 响应式布局检查——所有页面确保 flex 撑满，无底部空白

## 实施约束

1. **不要新建文件夹结构**——shadcn 组件放 `src/components/ui/`（已有 cn.ts）
2. **保持现有 API 层不变**——`src/api/` 下的文件不改
3. **保持现有路由不变**——`router.tsx` 不改
4. **保持现有类型不变**——`src/types/` 不改
5. **每改完一个组件就跑 `npx tsc --noEmit`**，确保无类型错误
6. **最后跑 `npx vite build` 确保构建通过**
7. **一次性提交**，commit message: `v4: shadcn/ui + Monaco editor — full frontend refresh`

## 验收标准

- [ ] `npx tsc --noEmit` 无错误
- [ ] `npx vite build` 成功
- [ ] 所有页面在浏览器中正常渲染（暗色主题）
- [ ] Files tab 用 Monaco 编辑器，有语法高亮
- [ ] 所有开关用 shadcn Switch
- [ ] 所有按钮用 shadcn Button
- [ ] 无底部空白，flex 撑满
- [ ] 无 inline style 硬编码颜色（除非 shadcn 变量不够用）
