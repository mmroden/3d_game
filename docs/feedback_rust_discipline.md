---
name: Rust discipline - lean into the compiler
description: Never suppress Rust compiler/clippy warnings with mut/unwrap/unsafe workarounds; treat warnings as errors; fix the underlying issue
type: feedback
---

Lean into Rust's compiler. Don't make everything `mut` to avoid borrow checker issues — those errors are signal. Don't `.unwrap()` spam or use `unsafe` to dodge ownership. Treat warnings as errors (`#![deny(warnings)]` at crate level, `clippy -- -D warnings` in CI).

**Why:** User's Monkey's Paw philosophy — agentic coding that silences the compiler to "make it work" produces code that compiles but is architecturally broken. The compiler is a collaborator, not an obstacle.

**How to apply:** When the borrow checker complains, restructure the code (copy values out, split borrows, rethink ownership). When clippy warns, fix the code. Never add `#[allow(...)]` without an explicit, justified reason.
