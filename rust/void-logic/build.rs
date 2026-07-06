//! Embeds every `rosters/planets/*.toml` into the crate — a new planet
//! file wires itself in by existing; forgetting is impossible (the array
//! was hand-listed once, and a hand-listed include is a silent gap waiting
//! to happen). Content edits retrigger builds through the `include_str!`
//! deps rustc reports; this script's rerun guard covers files appearing
//! and disappearing.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let planets_dir = manifest.join("../../rosters/planets");
    println!("cargo:rerun-if-changed={}", planets_dir.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&planets_dir)
        .expect("rosters/planets/ must exist")
        .map(|e| e.expect("readable dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    // Numeric filename order — lexical would file planet_10 before
    // planet_2. The linker keys planets by their DECLARED number anyway;
    // this just keeps the embedding deterministic.
    files.sort_by_key(|p| {
        p.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit('_').next())
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });

    let entries: Vec<String> = files
        .iter()
        .map(|p| {
            let abs = p.canonicalize().expect("planet path resolves");
            format!("    include_str!({:?}),", abs.display())
        })
        .collect();
    let out = format!(
        "/// Every rosters/planets/*.toml on disk, embedded by build.rs.\n\
         const PLANET_TOMLS: &[&str] = &[\n{}\n];\n",
        entries.join("\n")
    );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("planet_tomls.rs"),
        out,
    )
    .expect("write planet_tomls.rs");
}
