# Round 15: 初始化修复 + 二进制路径 + Markdown + Claw Race 主题

## 报告人
草莓🍓 | 2026-03-19

---

## Bug 74 (P0): cursor 初始化格式

`create_agent` 写的是旧格式 `{"line":0}`，应该是 `{"collect":0,"steer":0}`。一行改动。

---

## Bug 75 (P1): System Agent 找不到 CLI 二进制

### 现象
System agent 调 `see-agent-corp agent create` 报 command not found。

### 修复
1. Worker 启动时把自身二进制的完整路径注入环境变量 `SAC_BIN`
2. System agent 的 SKILL.md 和 templates 中，CLI 命令全部改为 `$SAC_BIN agent create ...` 格式
3. Worker spawn 时 `Command::new(...).env("SAC_BIN", &self.binary_path)`
4. 或者更好：把二进制路径写到 `~/.see-agent-corp/bin/` 下（symlink），Worker 启动时把这个目录加到 PATH

**同时：** 准备一个 `scripts/install.sh` 安装脚本（下载 release 二进制 + 放到 `~/.see-agent-corp/bin/` + 加到 PATH），供 GitHub README 使用。

---

## Bug 76 (P1): Markdown 渲染样式缺失

### 现象
Chat 中 agent 回复使用了 `render_markdown()` + `inner_html`，HTML 标签被渲染了但没有样式（代码块无高亮、标题无大小、列表无缩进）。

### 修复
需要为 `.markdown-body` class 添加 CSS 样式。两种方案：

**方案 A（推荐）：** 在 `index.html` 或 `styles.css` 中添加 markdown-body 样式：
```css
.markdown-body h1 { font-size: 1.5em; font-weight: bold; margin: 0.5em 0; }
.markdown-body h2 { font-size: 1.3em; font-weight: bold; margin: 0.4em 0; }
.markdown-body h3 { font-size: 1.1em; font-weight: bold; margin: 0.3em 0; }
.markdown-body p { margin: 0.3em 0; }
.markdown-body ul, .markdown-body ol { padding-left: 1.5em; margin: 0.3em 0; }
.markdown-body li { margin: 0.1em 0; }
.markdown-body code { background: rgba(0,0,0,0.2); padding: 0.1em 0.3em; border-radius: 3px; font-family: monospace; font-size: 0.9em; }
.markdown-body pre { background: rgba(0,0,0,0.3); padding: 0.8em; border-radius: 6px; overflow-x: auto; margin: 0.5em 0; }
.markdown-body pre code { background: none; padding: 0; }
.markdown-body blockquote { border-left: 3px solid rgba(255,255,255,0.3); padding-left: 0.8em; margin: 0.5em 0; opacity: 0.8; }
.markdown-body table { border-collapse: collapse; margin: 0.5em 0; }
.markdown-body th, .markdown-body td { border: 1px solid rgba(255,255,255,0.2); padding: 0.3em 0.6em; }
.markdown-body a { color: #58a6ff; text-decoration: underline; }
.markdown-body strong { font-weight: bold; }
.markdown-body em { font-style: italic; }
```

**方案 B：** 引入 github-markdown-css 包。但 WASM 环境下可能不方便，方案 A 更轻量。

---

## Bug 77 (P1): Claw Race 主题改造

### 前端品牌名
左上角 "see-agent-corp" → **"Claw Race"**

### IDENTITY.md 新增 Race 属性
```markdown
# Identity

**Name:** Steward
**Emoji:** 🦞
**Race:** 🦞

一个 AI 系统管家。
```

Race 属性可选值（有螯的生物 emoji）：🦞🦀🦐🦑🦂🦈

### Agent Details - Info tab 展示 Race
在 Info 区域显示 Race 属性（如果有），用大号 emoji 展示。

### templates/IDENTITY.md 更新
```markdown
# Identity

**Name:** Agent
**Emoji:** 🤖
**Race:** 🦀

一个 AI 助手。
```

### System Agent 改名
- 前端 agent 列表中 `⚙️ System` → **`🦞 Steward`**
- 后端 `is_system` 标记不变，只是前端显示改
- system agent 的默认 emoji 改为 🦞
- templates/system-soul.md 中的自我介绍更新

### IDENTITY.md 解析
`parse_identity_field` 函数新增解析 `Race` 字段，返回到 API 的 `AgentSummary` 中。

---

## 实施约束

1. 顺序：Bug 74 → Bug 75 → Bug 76 → Bug 77
2. Bug 76 的 CSS 要和 DaisyUI 暗色主题兼容
3. Bug 77 只改前端显示和模板，不改后端 `is_system` 逻辑
4. 每步 cargo test
5. 最后 trunk build --release + git commit + git push
