.PHONY: dev test lint format install

install:
	uv sync

dev:
	uv run see-agent chat

serve:
	uv run see-agent serve

test:
	uv run pytest tests/ -v

lint:
	uv run ruff check src/ tests/

format:
	uv run ruff format src/ tests/
