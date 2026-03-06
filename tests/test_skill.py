"""Unit tests for skill loading and prompt injection."""

from pathlib import Path

from see_agent.brain.prompts import build_system_prompt
from see_agent.skill.loader import SkillInfo, load_skills


def _write_skill(path: Path, name: str, description: str, body: str) -> Path:
    """Write a SKILL.md file with frontmatter."""
    skill_dir = path / name
    skill_dir.mkdir(parents=True, exist_ok=True)
    skill_file = skill_dir / "SKILL.md"
    skill_file.write_text(
        f"---\nname: {name}\ndescription: {description}\n---\n{body}",
        encoding="utf-8",
    )
    return skill_file


class TestSkillLoader:
    """Tests for the skill loader."""

    def test_load_valid_skill(self, tmp_path):
        _write_skill(tmp_path, "open-browser", "Open a URL", "Step 1: open Safari")
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 1
        assert skills[0].name == "open-browser"
        assert skills[0].description == "Open a URL"
        assert "Step 1" in skills[0].body

    def test_load_multiple_skills(self, tmp_path):
        _write_skill(tmp_path, "skill-a", "Desc A", "Body A")
        _write_skill(tmp_path, "skill-b", "Desc B", "Body B")
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 2
        names = {s.name for s in skills}
        assert names == {"skill-a", "skill-b"}

    def test_missing_name_skipped(self, tmp_path):
        skill_dir = tmp_path / "bad"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text(
            "---\ndescription: no name\n---\nbody",
            encoding="utf-8",
        )
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 0

    def test_no_frontmatter_skipped(self, tmp_path):
        skill_dir = tmp_path / "no-fm"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text("Just plain text", encoding="utf-8")
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 0

    def test_nonexistent_dir_skipped(self):
        skills = load_skills(["/nonexistent/path/that/does/not/exist"])
        assert len(skills) == 0

    def test_duplicate_name_skipped(self, tmp_path):
        dir1 = tmp_path / "d1"
        dir2 = tmp_path / "d2"
        _write_skill(dir1, "same", "First", "body1")
        _write_skill(dir2, "same", "Second", "body2")
        skills = load_skills([str(dir1), str(dir2)])
        assert len(skills) == 1
        assert skills[0].description == "First"


class TestSkillPromptInjection:
    """Tests for skill injection into system prompt."""

    def test_skills_in_prompt(self):
        config = {"language": "en", "max_steps": 10}
        skills = [
            SkillInfo(name="open-browser", description="Open URL", body="...", path=Path(".")),
        ]
        prompt = build_system_prompt(config, skills=skills)
        assert "<SKILLS>" in prompt
        assert "open-browser" in prompt
        assert "Open URL" in prompt
        assert "</SKILLS>" in prompt

    def test_no_skills_no_section(self):
        config = {"language": "en", "max_steps": 10}
        prompt = build_system_prompt(config, skills=None)
        assert "<SKILLS>" not in prompt
