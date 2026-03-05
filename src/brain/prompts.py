"""System prompt builder — Python function concatenation + XML tags.

No template placeholders; everything is assembled by plain string
concatenation to avoid the bootstrapping trap described in the PRD.
"""

from pathlib import Path


def build_system_prompt(config: dict) -> str:
    """Build the full system prompt from *config*.

    Parameters:
        config: The application configuration dict (see ``config.py``).
            Relevant keys:
            - ``language`` (``"zh"`` | ``"en"``, default ``"zh"``)
            - ``max_steps`` (int, default 50)
            - ``soul_path`` (str | None, optional path to personality file)

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
            "使用中文思考和回复。"
        )
    else:
        parts.append(
            "You are an AI assistant that can operate a Mac computer. "
            "You can see screenshots and operate the mouse, keyboard, "
            "and terminal through tools.\n"
            "Think and reply in English."
        )

    # ── Rules ─────────────────────────────────────────────────────────
    if lang == "zh":
        parts.append(
            "<RULES>\n"
            "1. 每次只调用一个工具。调用后会收到新的截图，根据截图决定下一步。\n"
            "2. 操作前先仔细观察截图，确认要点击的位置。描述你看到了什么、打算做什么。\n"
            "3. 每次操作后，仔细对比前后截图，确认操作是否生效。没有生效则分析原因重试或换方式。\n"
            "4. 如果操作后界面没有变化，可能是点错位置、需要等待加载、或需要滚动。\n"
            "5. 能用 shell 命令快速完成的事优先用 shell，如打开应用用 shell('open -a AppName')。\n"
            "6. 输入中文前确认输入法状态，不确定则先用 hotkey 切换。\n"
            "7. 连续 3 次操作没有进展时，停下来重新分析当前状态，尝试完全不同的策略。\n"
            "8. 任务完成后必须调用 finished 工具。\n"
            "9. 遇到无法解决的问题（密码、验证码）调用 call_user，等用户处理后会通知你继续。\n"
            "</RULES>"
        )
    else:
        parts.append(
            "<RULES>\n"
            "1. Call only one tool at a time. You will receive a new "
            "screenshot after each call; decide the next step based on it.\n"
            "2. Before acting, carefully observe the screenshot to confirm "
            "the target position. Describe what you see and plan to do.\n"
            "3. After each action, compare before/after screenshots to "
            "verify the action took effect. If not, analyse and retry.\n"
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

    # ── Personality (optional, from soul_path file) ───────────────────
    soul_path: str | None = config.get("soul_path")
    if soul_path:
        p = Path(soul_path).expanduser()
        if p.exists():
            soul = p.read_text(encoding="utf-8").strip()
            if soul:
                parts.append(f"<PERSONALITY>\n{soul}\n</PERSONALITY>")

    return "\n\n".join(parts)
