"""System prompt builder — Python function concatenation + XML tags.

No template placeholders; everything is assembled by plain string
concatenation to avoid the bootstrapping trap described in the PRD.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from see_agent.skill.loader import SkillInfo


def build_system_prompt(
    config: dict,
    *,
    skills: list[SkillInfo] | None = None,
    memory_block: str = "",
) -> str:
    """Build the full system prompt from *config*.

    Parameters:
        config: The application configuration dict (see ``config.py``).
            Relevant keys:
            - ``language`` (``"zh"`` | ``"en"``, default ``"zh"``)
            - ``max_steps`` (int, default 50)
            - ``soul_path`` (str | None, optional path to personality file)
        skills: Optional list of loaded skill definitions to inject.
        memory_block: Optional pre-formatted memory text to inject.

    Returns:
        The assembled system prompt string.
    """
    lang: str = config.get("language", "zh")
    max_steps: int = config.get("max_steps", 50)

    parts: list[str] = []

    # ── Identity ──────────────────────────────────────────────────────
    if lang == "zh":
        parts.append(
            "你是一个能操作 Mac 电脑的 AI 助手。"
            "你可以看到屏幕截图，并通过工具操作鼠标、键盘和终端。\n"
            "你可以自由对话，也可以主动截图确认当前状态。\n"
            "使用中文思考和回复。"
        )
    else:
        parts.append(
            "You are an AI assistant that can operate a Mac computer. "
            "You can see screenshots and operate the mouse, keyboard, "
            "and terminal through tools.\n"
            "You can converse freely and proactively take screenshots "
            "to verify the current state.\n"
            "Think and reply in English."
        )

    # ── Rules ─────────────────────────────────────────────────────────
    if lang == "zh":
        parts.append(
            "<RULES>\n"
            "1. 你可以在一次回复中调用多个工具，它们会按顺序执行。\n"
            "2. 操作前先仔细观察截图，确认要点击的位置。"
            "在思考中详细描述你看到了什么、打算做什么、为什么这样做。\n"
            "3. 需要确认操作结果时，主动调用 screenshot 工具获取最新画面。\n"
            "4. 如果操作后界面没有变化，可能是点错位置、需要等待加载、或需要滚动。\n"
            "5. 能用 shell 命令快速完成的事优先用 shell，如打开应用用 shell('open -a AppName')。\n"
            "6. 输入中文前确认输入法状态，不确定则先用 hotkey 切换。\n"
            "7. 连续 3 次操作没有进展时，停下来重新分析当前状态，尝试完全不同的策略。\n"
            "8. 任务完成后必须调用 finished 工具。\n"
            "9. 遇到无法解决的问题（密码、验证码）调用 call_user，等用户处理后会通知你继续。\n"
            "10. 操作前先检查 <ENVIRONMENT> 中的运行应用列表。如果目标应用已在运行，"
            "用 shell('open -a AppName') 激活；如果未运行，先启动。\n"
            "11. macOS 支持多桌面(Space)。如果截图中看不到目标窗口但应用已在运行，"
            "尝试 hotkey(['ctrl','right']) 或 hotkey(['ctrl','left']) 切换桌面，"
            "或用 shell('open -a AppName') 将其调到当前桌面。\n"
            "12. 在思考中维护累积状态摘要：已完成的步骤、已检查的元素及其结果、"
            "发现的 UI 规则或规律。早期截图会被裁剪，你的思考是唯一的历史记录，"
            "后续步骤要利用之前记录的信息避免重复操作。\n"
            "13. 如果看到 [Conversation Summary]，"
            "它包含了对话早期的压缩摘要，请将其视为可靠上下文。\n"
            "</RULES>"
        )
    else:
        parts.append(
            "<RULES>\n"
            "1. You can call multiple tools in a single response; they "
            "will be executed sequentially.\n"
            "2. Before acting, carefully observe the screenshot to confirm "
            "the target position. Describe in detail what you see, what you "
            "plan to do, and why.\n"
            "3. When you need to verify an action's result, proactively call "
            "the screenshot tool to get the latest screen state.\n"
            "4. If the screen does not change after an action, you may have "
            "clicked the wrong position, need to wait, or need to scroll.\n"
            "5. Prefer shell commands when they can accomplish the task "
            "quickly, e.g. use shell('open -a AppName') to launch apps.\n"
            "6. Before typing Chinese text, confirm the input method state; "
            "switch with hotkey if unsure.\n"
            "7. If three consecutive actions make no progress, stop and "
            "re-analyse the current state; try a different strategy.\n"
            "8. You MUST call the finished tool when the task is complete.\n"
            "9. If you encounter an unsolvable problem (password, captcha), "
            "call call_user and wait for the user to handle it.\n"
            "10. Check the <ENVIRONMENT> block for running apps before acting. "
            "If the target app is already running, use shell('open -a AppName') "
            "to activate it; if not running, launch it first.\n"
            "11. macOS supports multiple desktops (Spaces). If the target window "
            "is not visible but the app is running, try hotkey(['ctrl','right']) "
            "or hotkey(['ctrl','left']) to switch desktops, or use "
            "shell('open -a AppName') to bring it to the current desktop.\n"
            "12. Maintain a cumulative status summary in your thoughts: completed steps, "
            "elements checked and their results, UI rules or patterns discovered. "
            "Earlier screenshots will be pruned — your thoughts are the only history. "
            "Use previously recorded information to avoid repeating actions.\n"
            "13. If you see a [Conversation Summary], it contains compressed earlier history. "
            "Treat it as reliable context.\n"
            "</RULES>"
        )

    # ── Constraints ───────────────────────────────────────────────────
    if lang == "zh":
        parts.append(
            "<CONSTRAINTS>\n"
            f"- 最多执行 {max_steps} 步\n"
            "- 不要执行危险的 shell 命令（rm -rf 等）\n"
            "- 不要访问或泄露密码、密钥等敏感信息\n"
            "</CONSTRAINTS>"
        )
    else:
        parts.append(
            "<CONSTRAINTS>\n"
            f"- Maximum {max_steps} steps per task\n"
            "- Do NOT execute dangerous shell commands (rm -rf, etc.)\n"
            "- Do NOT access or leak passwords, secrets, or other sensitive information\n"
            "</CONSTRAINTS>"
        )

    # ── Skills (optional) ─────────────────────────────────────────────
    if skills:
        active = [s for s in skills if not s.blocked]
        if active:
            skill_lines = []
            for s in active:
                skill_lines.append(f"- **{s.name}**: {s.description}")
            parts.append(
                "<SKILLS>\n"
                + "\n".join(skill_lines)
                + "\n</SKILLS>"
            )

    # ── Memory (optional) ─────────────────────────────────────────────
    if memory_block:
        parts.append(f"<MEMORY>\n{memory_block}\n</MEMORY>")

    # ── Personality (optional, from soul_path file) ───────────────────
    soul_path: str | None = config.get("soul_path")
    if soul_path:
        p = Path(soul_path).expanduser()
        if p.exists():
            soul = p.read_text(encoding="utf-8").strip()
            if soul:
                parts.append(f"<PERSONALITY>\n{soul}\n</PERSONALITY>")

    return "\n\n".join(parts)
