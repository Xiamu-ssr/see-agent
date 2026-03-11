# Bug Report: v3.2 关键问题

> 日期：2026-03-10 | 作者：蓝莓🫐 | 基于 commit 6ca9332
> 严重程度：含 1 个致命 bug（所有 team run 崩溃）

---

## Bug 1（P0 致命）：Agent 子进程启动即崩溃

**现象**：任何 team run 都立即 failed，agent 没有执行任何动作。

**日志**（`~/.see-agent/teams/{id}/agents/{aid}/logs/worker.log`）：

```
RuntimeError: This event loop is already running
```

**根因**：`RemoteBus.drain()` 和 `RemoteBus.send()` 是同步方法，内部用 `loop.run_until_complete()` 包装 async 的 `UDSClient.call()`。但它们被 `AgentLoop._run_loop()`（async 上下文）调用时，event loop 已经在运行，不能嵌套。

```
AgentLoop._run_loop() [async]
  → _drain_team_bus() [sync]
    → RemoteBus.drain() [sync]
      → loop.run_until_complete(client.call(...)) [💥 event loop already running]
```

**调用链**：`loop.py:478 _drain_team_bus()` → `loop.py:201 self._team_bus.drain()` → `remote_tools.py:82 loop.run_until_complete()`

**修复**：

方案 A（推荐）：`_drain_team_bus` 和 `_inject_bus_messages` 改为 async，调用 `RemoteBus` 的 async 方法：

```python
# agent/loop.py
async def _drain_team_bus(self, ctx: ConversationContext) -> int:
    if self._team_bus is None or self._agent_id is None:
        return 0
    messages = await self._team_bus.async_drain(self._agent_id)  # ← await
    ...
```

```python
# ipc/remote_tools.py — RemoteBus 加 async_drain
async def async_drain(self, agent_id: str) -> list[Any]:
    result = await self._client.call(BUS_DRAIN, agent_id=agent_id)
    return result.get("messages", [])
```

同时检查 `RemoteBus.send()` 和 `RemoteBoard` 中所有用 `loop.run_until_complete()` 的地方，全部改为 async。在 async 上下文中**永远不要用 `run_until_complete()`**。

方案 B（兜底）：用 `nest_asyncio` 允许嵌套 event loop。但这是 hack，不推荐。

**影响范围**：`remote_tools.py` 中所有同步方法（`send`、`drain`、`claim`、`complete` 等）如果从 async 上下文调用都会崩。需要全部检查。

---

## Bug 2（P1）：Agent 的 Team 列显示为 "-"

**现象**：Agents 页面中所有 agent 的 Team 列显示 "-"，即使 agent 已被 team.json 的 members 引用。

**根因**：`AgentDefinition.list_all_global()` 遍历目录结构来判断 agent 是否属于 team：
- 全局 agent：`~/.see-agent/agents/{id}/agent.json` → `team_id = None`
- Team agent：`~/.see-agent/teams/{tid}/agents/{id}/agent.json` → `team_id = tid`

但用户通过前端创建 agent 后，agent 文件在 `~/.see-agent/agents/` 下。把 agent 加入 team 时，只修改了 `team.json` 的 `members` 列表，**没有把 agent 文件搬到 team 目录**。所以 `list_all_global()` 找到的全是全局 agent，team_id 全是 None。

**修复**（二选一）：

方案 A（改 list_all_global）：遍历完目录后，再遍历所有 team.json，对 members 里的 agent 补上 team_id：

```python
# 遍历所有 team，建立 agent_id → team_id 映射
agent_team_map: dict[str, str] = {}
if TEAMS_DIR.exists():
    for team_dir in TEAMS_DIR.iterdir():
        tj = team_dir / "team.json"
        if tj.exists():
            td = json.loads(tj.read_text())
            for member_id in td.get("members", []):
                agent_team_map[member_id] = td["id"]

# 给全局 agent 补上 team_id
results = [(defn, agent_team_map.get(defn.id)) for defn, _ in results]
```

方案 B（加入 team 时搬文件）：前端/后端 "add agent to team" 操作时，把 agent 目录从 `agents/{id}/` 移动到 `teams/{tid}/agents/{id}/`。但这改动大，涉及 agent 的 load/find 逻辑。

**推荐方案 A**，改动最小。

---

## Bug 3（P1）：像素办公室没加载

**现象**：Team 详情页的像素办公室区域空白，没有渲染。

**排查**：
- Phaser 代码是 procedural 渲染（不需要外部图片资源）
- `PixelOffice.tsx` 中 `game.events.once("ready", ...)` 可能没触发
- 或者 `members` 为空导致不渲染

**需要 CC 排查**：
1. 浏览器 console 是否有 Phaser 相关报错
2. `game.events.once("ready")` 是否可靠 —— Phaser 的 `ready` 事件在某些版本/配置下不触发。改为用 `scene.events.once("create")` 更可靠
3. 如果 team status 返回的 members 为空或 Phaser scene 没有 start，可能需要手动调 `scene.start("OfficeScene", data)`

**建议修复**：

```typescript
// PixelOffice.tsx — 改为更可靠的初始化方式
const game = new Phaser.Game({...});
gameRef.current = game;

// 不用 game.events.once("ready")，改为 scene 级别事件
game.scene.start("OfficeScene", { members, seating, onAgentClick });
```

---

## Bug 4（P1）：Skills Install 500 错误

**现象**：在 Skills 页面点击 "Install from ClawhHub"，输入 `arun-8687/tavily-search`，返回 500 Internal Server Error。

**根因**：`clawhub` CLI 没有安装。`subprocess.run(["clawhub", ...])` 抛 `FileNotFoundError`，后端没有 catch，变成 500。

**修复两步**：

**Step 1**：`see-agent install` 需要安装 `clawhub`：

```python
# cli/main.py install 命令末尾加：
# Install clawhub CLI
typer.echo("Installing clawhub CLI...")
subprocess.run(["npm", "install", "-g", "clawhub"], check=False)
```

**Step 2**：`skills.py` 的 install 路由 catch `FileNotFoundError`：

```python
@router.post("/skills/install")
async def install_skill(body: InstallSkillRequest) -> SkillInstallResponse:
    try:
        result = subprocess.run(
            ["clawhub", "install", body.name, "--target", str(SKILLS_DIR)],
            capture_output=True, text=True, timeout=60,
        )
    except FileNotFoundError:
        raise HTTPException(
            status_code=400,
            detail="clawhub CLI not found. Run: npm install -g clawhub",
        )
    if result.returncode != 0:
        raise HTTPException(status_code=400, detail=f"Install failed: {result.stderr}")
    return SkillInstallResponse(status="ok", name=body.name)
```

---

## Bug 5（P2）：端口未按 v3.2 设计变更

**现象**：默认端口仍是 8000，v3.2 设计要求改为 28789。

**修复**：`cli/main.py` 中所有 `8000` 默认值改为 `28789`：

```python
@app.command()
def start(
    port: int = typer.Option(28789, "--port", "-p"),
    ...
```

`restart` 命令同理。README 和 docs 中的端口也需要同步更新。

---

## Bug 6（P2）：日志不足

**现象**：排查问题时，后端日志只有 HTTP 请求日志，没有业务逻辑日志（team run 启动了哪些 agent、子进程 PID、sandbox profile 路径等）。

**建议**：在以下关键位置加 `logger.info()`：
- `team/manager.py` — agent 子进程启动/退出/结果
- `ipc/router.py` — UDS 连接建立/断开
- `server/routes/skills.py` — install 命令和结果
- `server/routes/agents.py` — agent 创建/删除/加入 team

---

## 执行优先级

| # | 问题 | 优先级 | 理由 |
|---|------|--------|------|
| 1 | async run_until_complete 崩溃 | **P0** | 所有 team run 不可用 |
| 2 | Agent team 显示为 "-" | P1 | 前端信息错误 |
| 3 | 像素办公室不加载 | P1 | 核心 UI 功能缺失 |
| 4 | Skills install 500 + clawhub 未装 | P1 | 功能不可用 |
| 5 | 端口未变更 | P2 | 和设计不一致 |
| 6 | 日志不足 | P2 | 影响排查效率 |

先修 Bug 1，否则其他功能都没法测。做完跑 `scripts/check.sh` 确保全过。
