# Bug Report: v2.5 前端适配补丁

> 日期：2026-03-10 | 作者：蓝莓🫐 | 基于 commit 6f9c135 的 Review

---

## 已修复（蓝莓直接改了）

| # | 问题 | 修复 |
|---|------|------|
| ✅ | `PUT /api/agents/{id}` 只能更新全局 agent，team 内 agent 404 | 改用 `find()` + `save_to(agent_dir.parent)` |
| ✅ | config.schema.json `scaling_match` enum 缺 `pixel_count` | 补全为 `["aspect_ratio", "pixel_count", "exact"]` |
| ✅ | config.schema.json 所有字段缺 `default` 值 | 已补全 |
| ✅ | dashboard report 要求 `recent_activity` 但不需要 | 已从 report 中删除 |

---

## 待 CC 处理

### Bug 1（P1）：`POST /api/agents` 缺少配置字段

**现状**：`CreateAgentRequest` 只有 `id` / `name` / `role` / `soul`，创建时不能带 `config_overrides` / `tools_config` / `skills_config` / `mcp_config`。

**影响**：前端创建 agent 后必须再发一次 PUT 才能配置，多一次请求。

**修复**：`CreateAgentRequest` 加可选字段：

```python
class CreateAgentRequest(BaseModel):
    id: str
    name: str
    role: str = "general assistant"
    soul: str | None = None
    config_overrides: dict[str, Any] | None = None
    tools_config: dict[str, Any] | None = None
    skills_config: dict[str, Any] | None = None
    mcp_config: dict[str, Any] | None = None
```

`create_agent` 里把非 None 字段传给 `AgentDefinition.create()`。

---

### 设计决策（P0）：JSON Schema 多语言策略

**背景**：前端用 rjsf 渲染 config / agent / team 的表单。目前 schema 里没有中文 `title` / `description`，rjsf 渲染出来只显示原始 key name（如 `max_steps`、`scaling_match`）。

**问题**：要不要在 schema 里写中文？如果写了怎么多语言？

**结论：schema 只做数据约束，不做 UI 文案。多语言由前端处理。**

#### 理由

1. JSON Schema 规范的 `title` / `description` 是给人看的，但没有多语言机制
2. see-agent 已有 `"language": "zh" | "en"` 配置，前端天然需要 i18n
3. 如果 schema 写死中文，切英文时 title 还是中文，不一致
4. rjsf 官方推荐方案就是 `uiSchema` + 自定义组件处理多语言

#### Schema 职责（后端）

只保证以下约束准确即可：

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

**不写**：`title`、`description`、`placeholder`

#### 前端职责

前端维护一个翻译映射文件，按字段路径查找：

```typescript
// locales/zh.json
{
  "config.llm.base_url": "API 地址",
  "config.llm.api_key": "API 密钥",
  "config.llm.model": "模型",
  "config.max_steps": "最大步数",
  "config.scaling_match": "缩放匹配模式",
  "config.memory.enabled": "启用记忆",
  "config.memory.provider": "记忆后端",
  // ...
}

// locales/en.json
{
  "config.llm.base_url": "API Endpoint",
  "config.llm.api_key": "API Key",
  "config.llm.model": "Model",
  "config.max_steps": "Max Steps",
  // ...
}
```

rjsf 用自定义 `TitleField` 或 `uiSchema` 注入翻译：

```typescript
// 方案 A: uiSchema 动态生成
function buildUiSchema(schema, locale) {
  const ui = {};
  for (const [key, prop] of Object.entries(schema.properties)) {
    ui[key] = { "ui:title": t(`config.${key}`) };
  }
  return ui;
}

// 方案 B: 自定义 TitleField（更干净）
const CustomTitleField = ({ title, id }) => {
  const path = id.replace("root_", "config.").replaceAll("_", ".");
  return <label>{t(path) || title}</label>;
};
```

#### CC 需要做的

1. **不改 schema** — 当前 schema（英文 key + default + enum + min/max）就是最终版
2. **新增 `GET /api/config/language`**（可选） — 返回当前 `language` 设置，前端据此选 locale
3. **确保所有 schema 字段和 DEFAULT_CONFIG 一一对应** — 目前已经对齐

---

### Bug 2（P2）：`_NullEye` hack

**文件**：`server/routes/tools.py`

**现状**：为了列 tools 不需要真实截屏，创建了一个 `_NullEye` 假对象传给 `create_registry()`。

**问题**：不优雅，如果 `create_registry` 签名变了就崩。

**修复建议**：`create_registry()` 的 `eye` 参数改为可选（默认 None），内部判断：

```python
def create_registry(eye: MacEye | None = None) -> ToolRegistry:
    registry = ToolRegistry()
    # 屏幕操作类 tool 只在有 eye 时注册
    if eye is not None:
        registry.register(ScreenshotTool(eye))
        registry.register(ClickTool(eye))
        # ...
    # 非屏幕 tool 始终注册
    registry.register(FinishedTool())
    registry.register(CallUserTool())
    # ...
```

这样 `tools.py` 路由直接 `create_registry()` 无参调用即可。

---

## 执行优先级

| # | 任务 | 优先级 |
|---|------|--------|
| 1 | POST agents 加配置字段 | P1 |
| 2 | Schema 多语言：不改后端，前端 i18n 处理（见上） | P0（设计决策，CC 知晓即可） |
| 3 | `_NullEye` 重构 | P2 |

做完跑 `scripts/check.sh` 确保全过。
