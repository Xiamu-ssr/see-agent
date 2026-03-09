"""Unit tests for skill loading and prompt injection."""

from pathlib import Path

from see_agent.brain.prompts import build_system_prompt
from see_agent.skill.loader import SkillInfo, gate_skills, load_skills


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


class TestSkillLoaderEdgeCases:
    """Additional edge cases for skill loading."""

    def test_nested_directory_skill(self, tmp_path):
        """SKILL.md in nested subdirectories should be found by glob."""
        nested = tmp_path / "category" / "sub" / "my-skill"
        nested.mkdir(parents=True)
        (nested / "SKILL.md").write_text(
            "---\nname: nested-skill\ndescription: Deep\n---\nBody"
        )
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 1
        assert skills[0].name == "nested-skill"

    def test_tilde_expansion(self, tmp_path):
        """Paths with ~ should be expanded."""
        # load_skills should handle ~ without crashing.
        # We pass a real ~ path but it may not have skills — just no crash.
        skills = load_skills(["~/nonexistent_skill_dir_xyz"])
        assert isinstance(skills, list)

    def test_empty_body_skill(self, tmp_path):
        """SKILL.md with valid frontmatter but empty body should load."""
        skill_dir = tmp_path / "empty-body"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text(
            "---\nname: empty-body\ndescription: No body.\n---\n"
        )
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 1
        assert skills[0].body == ""

    def test_parse_metadata_requires_bins(self, tmp_path):
        """SKILL.md with metadata JSON should populate requires_bins."""
        skill_dir = tmp_path / "docker-skill"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text(
            '---\nname: docker-skill\ndescription: Run docker.\n'
            'metadata: {"requires_bins": ["docker"]}\n---\nStep 1: run it',
            encoding="utf-8",
        )
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 1
        assert skills[0].requires_bins == ["docker"]

    def test_invalid_metadata_ignored(self, tmp_path):
        """Invalid JSON in metadata should not prevent skill loading."""
        skill_dir = tmp_path / "bad-meta"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text(
            "---\nname: bad-meta\ndescription: Bad metadata.\n"
            "metadata: {not valid json}\n---\nBody",
            encoding="utf-8",
        )
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 1
        assert skills[0].requires_bins == []

    def test_unicode_skill_content(self, tmp_path):
        """SKILL.md with CJK and emoji should parse correctly."""
        skill_dir = tmp_path / "unicode"
        skill_dir.mkdir()
        (skill_dir / "SKILL.md").write_text(
            "---\nname: 中文技能\ndescription: 打开浏览器🌐\n---\n步骤一：打开Safari",
            encoding="utf-8",
        )
        skills = load_skills([str(tmp_path)])
        assert len(skills) == 1
        assert skills[0].name == "中文技能"
        assert "🌐" in skills[0].description


class TestFilterSkills:
    """Tests for filter_skills()."""

    def test_filter_with_disabled(self):
        from see_agent.skill.loader import SkillInfo, filter_skills

        skills = [
            SkillInfo(name="a", description="A", body="", path=Path(".")),
            SkillInfo(name="b", description="B", body="", path=Path(".")),
            SkillInfo(name="c", description="C", body="", path=Path(".")),
        ]
        filtered = filter_skills(skills, disabled=["b"])
        assert len(filtered) == 2
        assert {s.name for s in filtered} == {"a", "c"}

    def test_filter_none_disabled(self):
        from see_agent.skill.loader import SkillInfo, filter_skills

        skills = [
            SkillInfo(name="a", description="A", body="", path=Path(".")),
        ]
        assert filter_skills(skills, disabled=None) == skills
        assert filter_skills(skills) == skills


class TestSkillGating:
    """Tests for gate_skills() requirement checking."""

    def test_gate_blocks_missing_bin(self):
        skill = SkillInfo(
            name="needs-docker", description="", body="", path=Path("."),
            requires_bins=["nonexistent_xyz_bin_12345"],
        )
        gate_skills([skill])
        assert skill.blocked is True
        assert "nonexistent_xyz_bin_12345" in skill.block_reason

    def test_gate_passes_available_bin(self):
        skill = SkillInfo(
            name="needs-python", description="", body="", path=Path("."),
            requires_bins=["python3"],
        )
        gate_skills([skill])
        assert skill.blocked is False

    def test_gate_blocks_missing_env(self):
        skill = SkillInfo(
            name="needs-env", description="", body="", path=Path("."),
            requires_env=["XYZZY_UNSET_12345"],
        )
        gate_skills([skill])
        assert skill.blocked is True
        assert "XYZZY_UNSET_12345" in skill.block_reason

    def test_gate_any_bins_one_available(self):
        skill = SkillInfo(
            name="needs-any", description="", body="", path=Path("."),
            requires_any_bins=["python3", "nonexistent_xyz"],
        )
        gate_skills([skill])
        assert skill.blocked is False

    def test_gate_any_bins_none_available(self):
        skill = SkillInfo(
            name="needs-any", description="", body="", path=Path("."),
            requires_any_bins=["nonexistent_a", "nonexistent_b"],
        )
        gate_skills([skill])
        assert skill.blocked is True

    def test_blocked_skill_excluded_from_prompt(self):
        config = {"language": "en", "max_steps": 10}
        skills = [
            SkillInfo(name="active", description="Works", body="", path=Path(".")),
            SkillInfo(
                name="blocked-one", description="Nope", body="", path=Path("."),
                blocked=True, block_reason="missing bin",
            ),
        ]
        prompt = build_system_prompt(config, skills=skills)
        assert "active" in prompt
        assert "blocked-one" not in prompt
