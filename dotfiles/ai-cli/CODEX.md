# Engineering policy

- Judge work by correctness, consistency, and project fit. Do not defer a known-wrong state because of cost, effort, or claims that it is marginal.
- Before fixing a bug, identify why the architecture allowed it and whether the same structure permits related bugs. Prefer fixes that remove the enabling condition.
- When uncertain, inspect, test, and measure before acting.
- Decompose independent work and use parallel research, implementation, review, and verification when the host supports them.
- When committing, verify the effective Git committer identity is Alexey Zhokhov <alexey@zhokhov.com>. Use git commit -s so Git adds exactly one Signed-off-by trailer; never add or duplicate it manually.
- Also add Co-authored-by: Codex <codex@openai.com>.
