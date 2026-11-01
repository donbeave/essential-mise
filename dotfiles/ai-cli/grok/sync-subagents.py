#!/usr/bin/env python3
"""Refresh ~/.grok/agents from ~/.claude/agents.

Keeps each agent's procedure and tool list. Sets the Grok Build model and
reasoning effort for that role.

The Grok Build picker is limited to grok-4.7. Older ids and the fast variant
are disabled in ~/.grok/config.toml. An unknown model id falls back to the
parent. Effort is the cost control: low, medium, high, xhigh.

The map follows the Claude roster. Haiku seats use medium. Sonnet seats use
high. The Fable seat is the only xhigh. grok-4.7 is the coding model xAI
recommends, so every seat uses it.

Grok loads ~/.grok/agents ahead of ~/.claude/agents. Claude Code keeps reading
~/.claude/agents.
"""

from __future__ import annotations

import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
REPO_CLAUDE = HERE.parent / "claude" / "agents"
if REPO_CLAUDE.is_dir():
    SRC = REPO_CLAUDE
    DST = HERE / "agents"
else:
    SRC = pathlib.Path.home() / ".claude" / "agents"
    DST = pathlib.Path.home() / ".grok" / "agents"

# name -> (model, effort). Unlisted agents use DEFAULT.
ASSIGNMENTS = {
    "locator": ("grok-4.7", "medium"),
    "mechanic": ("grok-4.7", "medium"),
    "verifier": ("grok-4.7", "medium"),
    "docs-researcher": ("grok-4.7", "medium"),
    "docs-writer": ("grok-4.7", "medium"),
    "fable-architect": ("grok-4.7", "xhigh"),
}
DEFAULT = ("grok-4.7", "high")


def assignment(name: str) -> tuple[str, str]:
    model, effort = ASSIGNMENTS.get(name.removesuffix(".md"), DEFAULT)
    if model != "grok-4.7":
        raise SystemExit(f"{name}: refusing model {model}; only grok-4.7 is allowed")
    return model, effort


def rewrite(name: str, text: str) -> str:
    if not text.startswith("---\n"):
        raise SystemExit(f"{name}: missing frontmatter")
    end = text.find("\n---", 3)
    if end < 0:
        raise SystemExit(f"{name}: unclosed frontmatter")
    model, effort = assignment(name)
    fm = text[:end]
    body = text[end:]
    fm = re.sub(r"(?m)^# Grok overlay of .*\n", "", fm)
    note = (
        f"# Grok overlay of ~/.claude/agents/{name}. "
        f"Seat: {model} @ {effort}.\n"
    )
    fm = "---\n" + note + fm[len("---\n") :]
    if re.search(r"(?m)^model:", fm):
        fm = re.sub(r"(?m)^model:.*$", f"model: {model}", fm)
    else:
        fm += f"\nmodel: {model}"
    if re.search(r"(?m)^effort:", fm):
        fm = re.sub(r"(?m)^effort:.*$", f"effort: {effort}", fm)
    else:
        fm += f"\neffort: {effort}"
    return fm + body


def main() -> None:
    DST.mkdir(exist_ok=True)
    written = []
    names = []
    for path in sorted(SRC.glob("*.md")):
        out = DST / path.name
        model, effort = assignment(path.name)
        out.write_text(rewrite(path.name, path.read_text()))
        names.append(path.name)
        written.append(f"{path.stem}: {model} @ {effort}")
    stale = sorted(p.name for p in DST.glob("*.md") if p.name not in names)
    print(f"wrote {len(written)} agents to {DST}")
    for line in written:
        print(line)
    if stale:
        print("left in place, no Claude source:", ", ".join(stale))


if __name__ == "__main__":
    main()
