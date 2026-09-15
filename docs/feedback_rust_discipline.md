---
name: Rust discipline - lean into the compiler
description: Never suppress Rust compiler/clippy warnings with mut/unwrap/unsafe workarounds; treat warnings as errors; fix the underlying issue
type: feedback
---

Lean into Rust's compiler. Don't make everything `mut` to avoid borrow checker issues — those errors are signal. Don't `.unwrap()` spam or use `unsafe` to dodge ownership. Treat warnings as errors (`#![deny(warnings)]` at crate level, `clippy -- -D warnings` in CI).

**Why:** The Zen of the Tool (the Monkey's Paw's third failure mode: a tool used syntactically correctly while its design assumptions are violated). Rust was chosen because its compiler and clippy eliminate entire classes of problems that other languages leave to discipline and luck: ownership, aliasing, exhaustiveness, unhandled errors. Leaning into the compiler is leaning into the Zen of Rust. Silencing it with `mut`, `unwrap`, `unsafe`, or `#[allow]` throws away the reason the language was picked and leaves code that compiles but carries exactly the bugs the tool exists to prevent.

**How to apply:** When the borrow checker complains, restructure the code (copy values out, split borrows, rethink ownership). When clippy warns, fix the code. Never add `#[allow(...)]` without an explicit, justified reason. Review Rust code against `docs/review/principles/rust_idioms.md`.
