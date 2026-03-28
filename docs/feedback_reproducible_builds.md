---
name: Reproducible local builds via Makefile
description: User requires make deps/check/demo pattern with all tools installed locally, never system-wide
type: feedback
---

All dependencies must be local to the project directory. Use `make deps` to fetch tools (Godot, assets), `make check` for lint+test, `make demo` to run. No system-wide installs — the project must be portable.

**Why:** User wants to take the project elsewhere and have it work. System installs create hidden dependencies that break reproducibility.

**How to apply:** Tools go in `./tools/`, assets in `./assets/`, both gitignored. The Makefile is the single entry point. Never suggest `brew install` or global installs for project dependencies.
