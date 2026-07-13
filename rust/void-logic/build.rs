//! Embeds every `rosters/planets/*.toml` and `rosters/environments/*.toml`
//! into the crate — a new file wires itself in by existing; forgetting is
//! impossible (the array was hand-listed once, and a hand-listed include is
//! a silent gap waiting to happen). Content edits retrigger builds through
//! the `include_str!` deps rustc reports; this script's rerun guard covers
//! files appearing and disappearing.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Embed a rosters/ subdirectory's *.toml files as a `&[&str]` const.
/// Numeric filename order where trailing numbers exist (lexical would file
/// planet_10 before planet_2), lexical otherwise — the linker keys entries
/// by their DECLARED identity anyway; this just keeps embedding
/// deterministic.
fn embed(dir: &Path, const_name: &str, doc: &str) -> String {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} must exist ({e})", dir.display()))
        .map(|e| e.expect("readable dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort_by_key(|p| {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let numeric = stem
            .rsplit('_')
            .next()
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(u32::MAX);
        (numeric, stem)
    });

    let entries: Vec<String> = files
        .iter()
        .map(|p| {
            let abs = p.canonicalize().expect("embedded path resolves");
            format!("    include_str!({:?}),", abs.display())
        })
        .collect();
    format!(
        "/// {doc}\nconst {const_name}: &[&str] = &[\n{}\n];\n",
        entries.join("\n")
    )
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = format!(
        "{}\n{}",
        embed(
            &manifest.join("../../rosters/planets"),
            "PLANET_TOMLS",
            "Every rosters/planets/*.toml on disk, embedded by build.rs.",
        ),
        embed(
            &manifest.join("../../rosters/environments"),
            "ENVIRONMENT_TOMLS",
            "Every rosters/environments/*.toml on disk, embedded by build.rs.",
        ),
    ) + &embed(
        &manifest.join("../../rosters/windows"),
        "WINDOW_TOMLS",
        "Every rosters/windows/<env-key>.toml (GENERATED window panes, \
         derived from each scene's glass), embedded by build.rs.",
    );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("planet_tomls.rs"),
        out,
    )
    .expect("write planet_tomls.rs");
}
