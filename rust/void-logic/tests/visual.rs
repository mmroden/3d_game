//! The visual-contract suite: boot the REAL game at derived poses,
//! capture frames through the shot sequencer, and assert on the images.
//! One capture engine serves every contract; poses always derive from the
//! grammar and the generated level — never authored coordinates, never a
//! hardcoded level range.
//!
//! Runs as part of `make check` (the `check-visual` stage) and alone:
//!
//!     make check-visual                 # every contract
//!     LEVEL=8 SEED=2 make check-visual  # one level/seed, frames kept
//!
//! `KEEP_FRAMES=1` (implied by LEVEL=) retains frames under
//! `out/visual/`; otherwise they land in a scratch dir. Failing frames
//! are always named in the failure output.

use std::path::{Path, PathBuf};
use std::process::Command;

use void_logic::level_assembly::{flythrough_poses, interior_probe_poses};
use void_logic::level_spec::LevelSpec;
use void_logic::seed::Seed;

fn repo_root() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).to_path_buf()
}

fn godot() -> PathBuf {
    std::env::var_os("GODOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("tools/Godot.app/Contents/MacOS/Godot"))
}

/// The levels of one planet, derived from the grammar's own level algebra
/// (resize a planet and every consumer follows).
fn levels_of_planet(planet: u32) -> Vec<u32> {
    let roster = void_logic::roster::roster();
    let mut levels = Vec::new();
    for level in 1..=999 {
        let (p, _) = roster.planet_number_and_relative(level);
        if p == planet {
            levels.push(level);
        } else if p > planet {
            break;
        }
    }
    levels
}

/// Boot the game once and capture one frame per pose. Returns the frame
/// paths in pose order. `ambient` floods flat white light (`--ambient=1`)
/// so geometry is visible regardless of fixtures.
fn capture(level: u32, run_seed: i64, poses: &[[f32; 5]], ambient: bool, dir: &Path) -> Vec<PathBuf> {
    assert!(!poses.is_empty(), "capture needs poses");
    std::fs::create_dir_all(dir).expect("capture dir");
    for stale in std::fs::read_dir(dir).expect("capture dir readable") {
        let p = stale.expect("dir entry").path();
        if p.extension().is_some_and(|e| e == "png") {
            std::fs::remove_file(p).expect("stale frame removed");
        }
    }
    let shot = poses
        .iter()
        .map(|p| format!("{:.2},{:.2},{:.2},{:.2},{:.2}", p[0], p[1], p[2], p[3], p[4]))
        .collect::<Vec<_>>()
        .join(";");
    let status = Command::new(godot())
        .arg("--path")
        .arg(repo_root().join("godot"))
        .args(["--resolution", "1144x828", "--"])
        .arg(format!("--level={level}"))
        .arg(format!("--seed={run_seed}"))
        .args(["--sbs=0", "--populace=0", "--cull=0"])
        .args(ambient.then_some("--ambient=1"))
        .arg(format!("--shot={shot}"))
        .arg(format!("--shot-dir={}", dir.display()))
        // The engine's chatter is part of the run artifact: park/build/
        // save ordering questions get answered from the log, not rerun
        // by hand.
        .stdout(std::fs::File::create(dir.join("engine.log")).expect("engine log"))
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("Godot did not launch ({e}) — set GODOT or run make deps"));
    assert!(status.success(), "capture run failed (level {level} seed {run_seed})");

    let mut frames: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("capture dir readable")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    frames.sort();
    assert_eq!(
        frames.len(),
        poses.len(),
        "level {level} seed {run_seed}: one frame per pose"
    );
    frames
}

/// The level the shell would build for `--level=N --seed=S`, structure
/// only — the same doors, the same seed algebra as a capture run.
fn build_graph(level: u32, run_seed: i64) -> (LevelSpec, void_logic::level_graph::LevelGraph) {
    let seed = Seed::from_i64(run_seed);
    let spec = LevelSpec::for_level(
        void_logic::roster::roster(),
        seed,
        level,
        &void_logic::unlocks::PermanentUnlocks::new(),
    );
    let graph = void_logic::generator::generate_for_spec(&spec, seed.for_level(level), true)
        .expect("level generates");
    (spec, graph)
}

/// An interior frame showing more void than this leaks — on EITHER
/// signature. The capture paints the background SENTINEL MAGENTA (see
/// LevelManager's `--ambient=1`): an escaping sightline renders that
/// color exactly, while no lit surface does — vol03's metallic plates
/// mirror the environment, which is why "dark = outside" alone was wrong
/// (metal walls mirrored the old dark sky). But black is not thereby
/// innocent: the panel kits author no black albedo and the capture
/// floods flat white ambient, so a black EXPANSE is also an escaped
/// sightline — fog extinguished the sentinel into exactly this black
/// past ~12 m until the override disabled it (2026-07-18). Both
/// signatures count; the threshold absorbs HUD outlines and dark trim.
const MAX_VOID: f32 = 0.05;

/// The cross-planet SMOKE bar: an artery vantage mostly showing escaped
/// background is a broken level, but planets 1 and 3 legitimately sight
/// out of the level (megakit windows, apartment panes) — renders-or-not,
/// never sealed-or-not.
const MAX_ESCAPED: f32 = 0.5;

/// Mean absolute per-channel difference between two frames within the
/// horizontal band [y0, y1) of frame height — how much that slice of the
/// view changed between two vantages.
fn band_diff(a: &Path, b: &Path, y0: f32, y1: f32) -> f32 {
    let a = image::open(a).expect("frame opens").to_rgb8();
    let b = image::open(b).expect("frame opens").to_rgb8();
    assert_eq!(a.dimensions(), b.dimensions(), "frames share a resolution");
    let (w, h) = a.dimensions();
    let (row0, row1) = ((h as f32 * y0) as u32, (h as f32 * y1) as u32);
    let mut sum = 0u64;
    for y in row0..row1 {
        for x in 0..w {
            let (pa, pb) = (a.get_pixel(x, y), b.get_pixel(x, y));
            for c in 0..3 {
                sum += pa[c].abs_diff(pb[c]) as u64;
            }
        }
    }
    sum as f32 / ((row1 - row0) as u64 * w as u64 * 3) as f32
}

/// Fractions of the frame reading as void: (sentinel, black). Sentinel:
/// red and blue high, green low — rough-metal reflections blur and
/// desaturate the background, so only direct sightlines stay this pure.
/// Black: every channel near zero, which no flood-lit panel renders.
fn void_fractions(path: &Path) -> (f32, f32) {
    let img = image::open(path).expect("frame opens").to_rgb8();
    let (mut sentinel, mut black) = (0u32, 0u32);
    for px in img.pixels() {
        if px[0] >= 180 && px[1] <= 80 && px[2] >= 180 {
            sentinel += 1;
        } else if px[0] < 20 && px[1] < 20 && px[2] < 20 {
            black += 1;
        }
    }
    let n = (img.width() * img.height()) as f32;
    (sentinel as f32 / n, black as f32 / n)
}

fn frames_dir(level: u32, run_seed: i64, tag: &str) -> PathBuf {
    // LEVEL-scoped runs and failures are for humans: keep under out/.
    // Full-suite runs use a scratch dir per level/seed. The tag keeps
    // contracts that shoot the same level/seed out of each other's dirs
    // (tests run concurrently; capture() clears stale frames on entry).
    let keep = std::env::var("KEEP_FRAMES").is_ok() || std::env::var("LEVEL").is_ok();
    let base = if keep {
        repo_root().join("out/visual")
    } else {
        std::env::temp_dir().join("void-visual")
    };
    base.join(format!("L{level}-S{run_seed}{tag}"))
}

/// CONTAINMENT: from any interior vantage of a PLANET 2 level, a flat-lit
/// capture shows geometry everywhere. Dark is a sightline escaping the
/// level; a level that lets the player see the void is not contained.
/// Planet 2 because its panel paradigm authors no dark textures — planet
/// 1's megakit does, legitimately, so this contract would be vacuous there.
#[test]
fn planet_two_interiors_show_no_void() {
    let level_filter: Option<u32> =
        std::env::var("LEVEL").ok().and_then(|v| v.parse().ok());
    let seeds: Vec<i64> = match std::env::var("SEED").ok().and_then(|v| v.parse().ok()) {
        Some(s) => vec![s],
        None => vec![1, 2],
    };

    let all = levels_of_planet(2);
    assert!(!all.is_empty(), "the grammar declares planet 2");
    // The planet's levels share one paradigm, one kit set, one generator —
    // containment is a property of that machinery, not of each level
    // number. Its extremes (smallest and largest room budget) cover it.
    let levels: Vec<u32> = if all.len() > 1 {
        vec![all[0], *all.last().unwrap()]
    } else {
        all
    };
    let mut leaks = Vec::new();
    for &level in &levels {
        if level_filter.is_some_and(|l| l != level) {
            continue;
        }
        for &run_seed in &seeds {
            let (spec, graph) = build_graph(level, run_seed);
            let poses = interior_probe_poses(&graph, spec.pitch, 8);
            let dir = frames_dir(level, run_seed, "");
            let frames = capture(level, run_seed, &poses, true, &dir);

            // Rig self-check: distinct poses must yield distinct frames.
            // Byte-identical neighbors mean the rig stalled and later
            // "results" are one vantage wearing several names (full-run
            // and mid-run stalls both observed 2026-07-14).
            let bytes: Vec<Vec<u8>> = frames
                .iter()
                .map(|f| std::fs::read(f).expect("frame reads"))
                .collect();
            if let Some(i) = (1..bytes.len()).find(|&i| bytes[i] == bytes[i - 1]) {
                leaks.push(format!(
                    "level {level} seed {run_seed}: STUCK from frame {i} — identical \
                     to its predecessor; the capture rig stalled ({})",
                    dir.display(),
                ));
                continue;
            }

            for (frame, pose) in frames.iter().zip(&poses) {
                let (sentinel, black) = void_fractions(frame);
                let leak = sentinel > MAX_VOID || black > MAX_VOID;
                let verdict = if leak { "LEAK" } else { "ok" };
                println!(
                    "visual: L{level} S{run_seed} {} sentinel={sentinel:.3} \
                     black={black:.3} {verdict} pose={pose:?}",
                    frame.file_name().unwrap().to_string_lossy(),
                );
                if leak {
                    leaks.push(format!(
                        "level {level} seed {run_seed} pose {pose:?}: \
                         {:.0}% sentinel + {:.0}% black — {}",
                        sentinel * 100.0,
                        black * 100.0,
                        frame.display(),
                    ));
                }
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "{} interior vantage(s) see the void:\n{}",
        leaks.len(),
        leaks.join("\n")
    );
}

/// Cross-planet SMOKE: the first level of planet 1 (layered megakit)
/// and of planet 3 (fixed environment) boots, resolves every scene,
/// and renders geometry along the flythrough artery. The containment
/// contract stays planet 2's — these paradigms legitimately sight out
/// of the level — so this asserts RENDERS: the rig advanced and no
/// artery vantage is mostly escaped background (`MAX_ESCAPED`).
#[test]
fn other_planets_boot_and_render() {
    let mut faults = Vec::new();
    for planet in [1u32, 3] {
        let level = *levels_of_planet(planet)
            .first()
            .expect("the grammar declares the planet");
        let run_seed = 1i64;
        let (spec, graph) = build_graph(level, run_seed);
        let poses = flythrough_poses(&graph, spec.pitch);
        assert!(!poses.is_empty(), "planet {planet}: artery poses derive");
        let dir = frames_dir(level, run_seed, "");
        let frames = capture(level, run_seed, &poses, true, &dir);

        let bytes: Vec<Vec<u8>> = frames
            .iter()
            .map(|f| std::fs::read(f).expect("frame reads"))
            .collect();
        if let Some(i) = (1..bytes.len()).find(|&i| bytes[i] == bytes[i - 1]) {
            faults.push(format!(
                "planet {planet} level {level}: STUCK from frame {i} — identical \
                 to its predecessor; the capture rig stalled ({})",
                dir.display(),
            ));
            continue;
        }
        for (frame, pose) in frames.iter().zip(&poses) {
            let (sentinel, _) = void_fractions(frame);
            println!(
                "visual: L{level} S{run_seed} {} sentinel={sentinel:.3} pose={pose:?}",
                frame.file_name().unwrap().to_string_lossy(),
            );
            if sentinel > MAX_ESCAPED {
                faults.push(format!(
                    "planet {planet} level {level} pose {pose:?}: {:.0}% escaped \
                     background — {}",
                    sentinel * 100.0,
                    frame.display(),
                ));
            }
        }
    }
    assert!(
        faults.is_empty(),
        "cross-planet smoke faults:\n{}",
        faults.join("\n")
    );
}

/// COCKPIT: first-person frames are framed by the shell. The cockpit is
/// rigid to the camera, so across DISTINCT artery vantages the bottom
/// band of the frame (the console) must stay near-invariant while the
/// world band changes — that pose-invariance IS a rendered cockpit, and
/// it derives from the shell's own definition: no color, no coordinate,
/// no part name pinned. A missing or invisible shell fails the ratio
/// (the bottom band would change like the world does).
#[test]
fn cockpit_console_is_pose_invariant_while_the_world_moves() {
    let level = *levels_of_planet(2)
        .first()
        .expect("the grammar declares planet 2");
    let run_seed = 1i64;
    let (spec, graph) = build_graph(level, run_seed);
    let poses = flythrough_poses(&graph, spec.pitch);
    assert!(poses.len() >= 2, "cockpit contract needs two artery vantages");
    let dir = frames_dir(level, run_seed, "-cockpit");
    let frames = capture(level, run_seed, &poses, true, &dir);

    // Same rig self-check as the other contracts: a stalled capture
    // would satisfy any invariance claim vacuously.
    let bytes: Vec<Vec<u8>> = frames
        .iter()
        .map(|f| std::fs::read(f).expect("frame reads"))
        .collect();
    assert!(
        !(1..bytes.len()).any(|i| bytes[i] == bytes[i - 1]),
        "capture rig stalled — adjacent frames identical ({})",
        dir.display()
    );

    // Bottom sixth = console; the band above the vertical center = world
    // through the canopy. Averaged over every adjacent pose pair.
    let pairs = frames.len() - 1;
    let (mut console, mut world) = (0.0f32, 0.0f32);
    for i in 1..frames.len() {
        console += band_diff(&frames[i - 1], &frames[i], 5.0 / 6.0, 1.0);
        world += band_diff(&frames[i - 1], &frames[i], 0.25, 0.5);
    }
    console /= pairs as f32;
    world /= pairs as f32;
    println!("visual: cockpit L{level} S{run_seed} console-drift={console:.2} world-drift={world:.2}");
    assert!(
        world > 2.0 * console,
        "the bottom band moves like the world — no cockpit is framing the \
         view (console {console:.2} vs world {world:.2}, frames in {})",
        dir.display()
    );
    // Absolute cap: glass-edge antialiasing wiggles a static console by a
    // few counts; a band that drifts more than ~5% of channel range is
    // not static furniture.
    assert!(
        console < 12.0,
        "console band drifts {console:.2}/255 across poses — the shell is \
         not rigid to the camera ({})",
        dir.display()
    );
}
