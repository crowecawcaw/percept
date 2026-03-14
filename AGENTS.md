# percept — design tenets

## 1. Agent-first, human-debuggable

The primary user of this tool is an AI agent, not a human. Design decisions should favor agent-centric workflows:

- Default output is JSON (structured, parseable)
- Human-readable alternatives exist for debugging (`--format tree`, etc.) but are not the default
- Help text should be terse and precise — agents read it literally
- Error messages should be actionable: tell the agent exactly what to do next

Human debugging should be *possible*, not *prioritized*.

## 2. Support what agents guess; prefer one canonical path

Agents may try things that aren't documented. Support reasonable variations rather than returning errors that force a specific invocation pattern:

- Both `percept --help` and `percept help` work
- Both `percept observe` (no args) and `percept observe --app <name>` work, with sensible defaults
- Lightweight aliases are fine when the cost is low and the benefit is fewer agent failures

Document the canonical workflow clearly. Support the rest silently.
