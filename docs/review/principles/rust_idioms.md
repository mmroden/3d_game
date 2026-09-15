# Principle: Rust and godot-rust idioms

The Zen of the Tool, the Monkey's Paw's third failure mode, applied to the
language itself. Rust was chosen because its compiler and clippy remove
entire classes of problems that other languages leave to discipline and
luck. Leaning into the compiler is leaning into the Zen of Rust; silencing it
throws away the reason the language was picked. This reviewer reads every
Rust hunk for places the code fights the tool instead of using it, and for
godot-rust used against its grain. It does not run clippy or tests; clippy
clean under `-D warnings` is a gate the invoker runs via `make check`.

## Checks

1. **Ownership and borrowing.** `mut` added to appease the checker rather
   than because the value changes; `.clone()` where a borrow, a `Copy`
   derive, or restructured ownership would do; `Rc`/`RefCell`/`Arc` where
   the ownership graph is a tree; values copied out and written back when a
   split borrow would work. The borrow checker error was signal; cite what it
   was pointing at.
2. **Error handling.** New `.unwrap()` or `.expect()` on a production path;
   `panic!` where a `Result` belongs; stringly errors (`Err(String)`,
   `anyhow!` in the model crate) where a typed error enum exists or should;
   `?` missing where a manual `match` re-wraps; errors swallowed by
   `let _ =` or `.ok()` without a stated reason.
3. **Exhaustiveness.** New `_ =>` arms on domain enums (the model crate
   denies `match_wildcard_for_single_variants`); `if let` chains where a
   `match` would force the compiler to check every variant; boolean flags
   where an enum names the states.
4. **Types over comments.** Raw `f32`/`u32`/`String` where a newtype names
   the domain; `as` casts where `From`/`TryFrom` belong; `Option<T>` where the
   absence is impossible by construction; tuples where a struct names the
   fields; `bool` parameters that read as `foo(true, false)` at call sites.
5. **Iterators and slices.** Index loops over `iter()`/`iter_mut()`;
   `Vec` where a slice parameter suffices; collect-then-iterate where a lazy
   chain reads better; `.len() == 0` for `.is_empty()`; manual `Option`
   plumbing where combinators (`map`, `and_then`, `ok_or`) say it in one
   line. Do not flag combinator chains that hurt readability; the yardstick
   is clarity, not cleverness.
6. **Traits and generics.** A trait with one impl and no planned second;
   generics where a concrete type is the only instantiation; `dyn` where
   static dispatch is available and the set is closed; missing `Default`,
   `Debug`, `Clone`, `PartialEq` derives that callers then hand-roll.
7. **Lints.** New `#[allow(...)]` without a written justification on the
   line; `#![deny(warnings)]` weakened; a crate-level allow widened to cover
   a local case.
8. **Module hygiene.** Imports at the top of the module, never mid-file or
   inside a function; policy numbers as function defaults or type attrs, not
   bare module constants; `pub` no wider than callers require; doc comments
   on public items that say what, not how.
9. **godot-rust idioms.** `Gd<T>` handles: stored handles checked with
   `is_instance_valid` and accessed with `iter_mut`, never cloned per frame;
   `base_mut()` scope kept minimal; `OnReady` for node references, not
   `get_node` in every method; signals through typed constants and
   `connect` at the right lifecycle point; `call_deferred` for anything the
   engine forbids mid-callback; `Variant` conversion at the boundary in one
   place; no `instantiate()` on a play path (Faucet Principle); `queue_free`
   only for the accepted cosmetic exception.
10. **Determinism and purity in the model crate.** No `std::time`, no
    `rand::rng()`, no I/O, no `HashMap` iteration order affecting output
    (use `BTreeMap` or sort) in `void-logic`.
11. **Unsafe.** Any `unsafe` block: is it necessary, is the invariant
    written on the block, is there a safe API that does the same?
12. **Tests as Rust.** Test code is held to the same rules: no leaked
    `'static` fixtures, no `unwrap` where `?` in a `Result`-returning test
    reads better, property tests via the existing harness rather than
    hand-rolled loops.

## Procedure

For each touched `.rs` file, `get_symbols_overview` then read the whole
file. For each new `pub`, `find_referencing_symbols`. For each `clone`,
`unwrap`, `as`, `mut`, `_ =>`, `#[allow]` in the diff, decide whether the
tool was leaned into or worked around, and cite the line.

## Severity guidance

`unwrap`/`expect`/`panic` on a gameplay or pipeline path: BLOCKER. New `_ =>`
on a domain enum: BLOCKER. `#[allow]` without justification: BLOCKER. Impurity
in the model crate: BLOCKER. `Gd<T>` clone-per-frame or missing validity
check: BLOCKER (it is a use-after-free). Unnecessary `clone`/`mut`, index
loops, `as` casts away from the boundary: CONCERN. Missing derives, `bool`
parameters, doc gaps: NIT.
