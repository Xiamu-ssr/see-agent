---
name: clawhub
description: Search and install skills from ClawHub into see-agent.
---

## Search Skills

Run in terminal:

```
clawhub search <keyword>
```

Or browse https://clawhub.com to find skills.

## Install a Skill

```
clawhub install <skill-name> --target ~/.see-agent/skills
```

The skill takes effect immediately — it will be loaded on the next agent conversation.

## View Installed Skills

```
ls ~/.see-agent/skills/
```

Each subdirectory is a skill containing a SKILL.md file.
