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
    capture_ex(level, run_seed, poses, ambient, false, &[], dir)
}

/// The full capture door: `sbs` renders side-by-side stereo (each frame
/// is a left|right pair at half width per eye), and `extra` passes
/// additional capture-diagnostic flags (e.g. the stereo overrides
/// `--convergence=`/`--interaxial=`).
fn capture_ex(
    level: u32,
    run_seed: i64,
    poses: &[[f32; 5]],
    ambient: bool,
    sbs: bool,
    extra: &[String],
    dir: &Path,
) -> Vec<PathBuf> {
    assert!(!poses.is_empty(), "capture needs poses");
    let shot = poses
        .iter()
        .map(|p| format!("{:.2},{:.2},{:.2},{:.2},{:.2}", p[0], p[1], p[2], p[3], p[4]))
        .collect::<Vec<_>>()
        .join(";");
    let mut args: Vec<String> = vec![
        format!("--level={level}"),
        format!("--seed={run_seed}"),
        (if sbs { "--sbs=1" } else { "--sbs=0" }).to_string(),
        "--populace=0".to_string(),
        "--cull=0".to_string(),
    ];
    args.extend(ambient.then(|| "--ambient=1".to_string()));
    args.extend(extra.iter().cloned());
    args.push(format!("--shot={shot}"));
    launch_capture(&args, poses.len(), &format!("level {level} seed {run_seed}"), dir)
}

/// The same door for a MENU screen (`--screen=credits`): the shot list
/// is the roll's offsets in pixels, one frame per offset, mono.
fn capture_screen(screen: &str, offsets: &[f32], dir: &Path) -> Vec<PathBuf> {
    assert!(!offsets.is_empty(), "capture needs offsets");
    let shot = offsets.iter().map(|o| format!("{o:.0}")).collect::<Vec<_>>().join(";");
    let args = vec![
        format!("--screen={screen}"),
        "--sbs=0".to_string(),
        format!("--shot={shot}"),
    ];
    launch_capture(&args, offsets.len(), &format!("screen {screen}"), dir)
}

/// Boot Godot once with the capture knobs, collect one frame per shot
/// from `dir` (cleared of stale frames first), and keep the engine's
/// chatter beside them as engine.log.
fn launch_capture(args: &[String], expected: usize, subject: &str, dir: &Path) -> Vec<PathBuf> {
    std::fs::create_dir_all(dir).expect("capture dir");
    for stale in std::fs::read_dir(dir).expect("capture dir readable") {
        let p = stale.expect("dir entry").path();
        if p.extension().is_some_and(|e| e == "png") {
            std::fs::remove_file(p).expect("stale frame removed");
        }
    }
    let status = Command::new(godot())
        .arg("--path")
        .arg(repo_root().join("godot"))
        .args(["--resolution", "1144x828", "--"])
        .args(args)
        .arg(format!("--shot-dir={}", dir.display()))
        // The engine's chatter is part of the run artifact: park/build/
        // save ordering questions get answered from the log, not rerun
        // by hand.
        .stdout(std::fs::File::create(dir.join("engine.log")).expect("engine log"))
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("Godot did not launch ({e}) — set GODOT or run make deps"));
    assert!(status.success(), "capture run failed ({subject})");

    let mut frames: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("capture dir readable")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    frames.sort();
    assert_eq!(frames.len(), expected, "{subject}: one frame per shot");
    frames
}

/// The level the shell would build for `--level=N --seed=S` — the same
/// doors, the same seed algebra as a capture run. Generated planets:
/// structure-only, exactly what a `--populace=0` capture builds (the
/// boss arena rides populace; a probe posed in a room the capture never
/// built sees the void). Fixed environments: the FULL graph — populace
/// never moves a zone, and structure-only there is the menu-backdrop
/// contract (the start zone alone), which left every planet-3 vantage
/// in the first room (owner 2026-09-06: "add some different display
/// angles for the three new rooms").
fn build_graph(level: u32, run_seed: i64) -> (LevelSpec, void_logic::level_graph::LevelGraph) {
    let seed = Seed::from_i64(run_seed);
    let spec = LevelSpec::for_level(
        void_logic::roster::roster(),
        seed,
        level,
        &void_logic::unlocks::PermanentUnlocks::new(),
    );
    let fixed = matches!(spec.paradigm, void_logic::level_spec::Paradigm::Fixed(_));
    let graph = void_logic::generator::generate_for_spec(&spec, seed.for_level(level), !fixed)
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

/// The FIXED-environment bar: a fixed scene is a real place with no sky
/// behind it (fixed levels author no backdrop beyond the seller's), so
/// any sightline that escapes the level renders the sentinel — a hole
/// in the art or a missing pane, not a view. One percent of a frame is
/// a block some 90 x 90 px at the rig's size: a hole you could fly a
/// sightline through, never HUD trim (owner 2026-09-06: the villa's
/// doorway showed "pretty large blocks of sights to infinity" at 3.3%
/// of the frame, under every bar the rig then had).
const MAX_FIXED_ESCAPED: f32 = 0.01;

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
/// desaturate the background, so only direct sightlines stay this pure
/// (planet 2's contract needs exactly that: its metallic plates mirror
/// the sentinel as pink, and a mirrored sky is not an escape). Black:
/// every channel near zero, which no flood-lit panel renders.
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


/// Fraction of the pixels in the box [x0, x1) x [y0, y1) of frame
/// width and height that read as text: every channel bright, against a
/// panel whose fill is a mid blue and a backdrop that is darker still.
fn bright_fraction(path: &Path, x0: f32, x1: f32, y0: f32, y1: f32) -> f32 {
    let img = image::open(path).expect("frame opens").to_rgb8();
    let (w, h) = img.dimensions();
    let (col0, col1) = ((w as f32 * x0) as u32, (w as f32 * x1) as u32);
    let (row0, row1) = ((h as f32 * y0) as u32, (h as f32 * y1) as u32);
    let mut bright = 0u32;
    for y in row0..row1 {
        for x in col0..col1 {
            let px = img.get_pixel(x, y);
            if px[0] >= 170 && px[1] >= 170 && px[2] >= 170 {
                bright += 1;
            }
        }
    }
    bright as f32 / ((row1 - row0) * (col1 - col0)) as f32
}

/// Fraction of the frame reading as the sentinel on a direct sightline
/// OR through a pane: red and blue high, green well below both. A glazed
/// wall with nothing behind it tints the magenta toward pink, and the
/// hill house's east glazing read 0.000 on the pure test (2026-09-06).
/// For FIXED environments only — archviz ships no mirror-metal plates,
/// so a pink reading there is void, not a reflection.
fn tinted_void_fraction(path: &Path) -> f32 {
    let img = image::open(path).expect("frame opens").to_rgb8();
    let mut sentinel = 0u32;
    for px in img.pixels() {
        let (r, g, b) = (px[0] as u32, px[1] as u32, px[2] as u32);
        if r >= 150 && b >= 150 && g * 100 <= r.min(b) * 65 {
            sentinel += 1;
        }
    }
    sentinel as f32 / (img.width() * img.height()) as f32
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
/// and of planet 3 (fixed environments) boots, resolves every scene,
/// and renders geometry along the flythrough artery. A fixed planet
/// DEALS its locations per run, so its first level is captured once per
/// declared location — sweeping run seeds until every environment has
/// been dealt (owner 2026-09-06: planet 3 shows two of four per run;
/// a location that never boots must fail here, not on a player's run).
/// A fixed environment is also vantaged from INSIDE its zones — up to
/// `FIXED_PROBE_BUDGET` interior probes spread across the authored
/// boxes, each facing its zone's center (owner 2026-09-06: "different
/// display angles for the three new rooms"; the one artery frame hid a
/// chair with no seat behind a kitchen island). Planet 1 keeps the
/// smoke bar (`MAX_ESCAPED` on the pure sentinel: its megakit windows
/// legitimately sight out); a fixed environment is held to
/// `MAX_FIXED_ESCAPED` on the TINTED sentinel on every frame — a
/// vantage that sees the void, bare or through a pane, is broken
/// containment, whatever the pixel count.
#[test]
fn other_planets_boot_and_render() {
    use void_logic::level_spec::Paradigm;
    const SEED_SWEEP: i64 = 32;
    const FIXED_PROBE_BUDGET: usize = 8;

    let mut faults = Vec::new();
    let roster = void_logic::roster::roster();
    for planet in [1u32, 3] {
        let level = *levels_of_planet(planet)
            .first()
            .expect("the grammar declares the planet");
        // One run seed per distinct environment the level can deal (a
        // generated planet deals nothing: one seed).
        let wanted = roster.planet_for_level(level).kits.len();
        let mut seen = std::collections::BTreeMap::new();
        for run_seed in 1..=SEED_SWEEP {
            let spec = LevelSpec::for_level(
                roster,
                Seed::from_i64(run_seed),
                level,
                &void_logic::unlocks::PermanentUnlocks::new(),
            );
            let key = match &spec.paradigm {
                Paradigm::Fixed(env) => env.key.clone(),
                _ => String::new(),
            };
            seen.entry(key).or_insert(run_seed);
            if seen.len() >= wanted {
                break;
            }
        }
        if seen.len() < wanted {
            faults.push(format!(
                "planet {planet} level {level}: only {} of {wanted} declared locations \
                 dealt within {SEED_SWEEP} run seeds ({:?})",
                seen.len(),
                seen.keys().collect::<Vec<_>>(),
            ));
        }

        for (key, &run_seed) in &seen {
            let (spec, graph) = build_graph(level, run_seed);
            let fixed = matches!(spec.paradigm, Paradigm::Fixed(_));
            let bar = if fixed { MAX_FIXED_ESCAPED } else { MAX_ESCAPED };
            let mut poses = flythrough_poses(&graph, spec.pitch);
            assert!(!poses.is_empty(), "planet {planet}: artery poses derive");
            if fixed {
                poses.extend(interior_probe_poses(&graph, spec.pitch, FIXED_PROBE_BUDGET));
            }
            let dir = frames_dir(level, run_seed, "");
            let frames = capture(level, run_seed, &poses, true, &dir);

            let bytes: Vec<Vec<u8>> = frames
                .iter()
                .map(|f| std::fs::read(f).expect("frame reads"))
                .collect();
            if let Some(i) = (1..bytes.len()).find(|&i| bytes[i] == bytes[i - 1]) {
                faults.push(format!(
                    "planet {planet} level {level} seed {run_seed} ({key}): STUCK from \
                     frame {i} — identical to its predecessor; the capture rig \
                     stalled ({})",
                    dir.display(),
                ));
                continue;
            }
            for (frame, pose) in frames.iter().zip(&poses) {
                let sentinel = if fixed {
                    tinted_void_fraction(frame)
                } else {
                    void_fractions(frame).0
                };
                println!(
                    "visual: L{level} S{run_seed} env={key} {} sentinel={sentinel:.3} \
                     bar={bar:.2} pose={pose:?}",
                    frame.file_name().unwrap().to_string_lossy(),
                );
                if sentinel > bar {
                    faults.push(format!(
                        "planet {planet} level {level} seed {run_seed} ({key}) pose \
                         {pose:?}: {:.1}% escaped background ({}) — {}",
                        sentinel * 100.0,
                        if fixed {
                            "a fixed environment shows the sentinel through a hole: \
                             missing panes or no backdrop behind an opening"
                        } else {
                            "mostly escaped background"
                        },
                        frame.display(),
                    ));
                }
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


/// Horizontal disparity (right-eye x minus left-eye x, pixels) of the
/// patch centered at `(x, y)` in the LEFT half of an SBS pair, found by
/// SAD block-match along the same scanline band of the right half.
/// `None` when the best match is no better than half the flat-offset
/// cost — an untextured patch has no reliable disparity.
fn patch_disparity(pair: &Path, x: u32, y: u32, half: u32, search: (i32, i32)) -> Option<f32> {
    let img = image::open(pair).expect("pair opens").to_luma8();
    let (w, h) = img.dimensions();
    let eye_w = w / 2;
    assert!(x >= half && x + half < eye_w && y >= half && y + half < h);
    let sad_at = |dx: i32| -> Option<u64> {
        let rx = x as i32 + dx;
        if rx < half as i32 || (rx + half as i32) as u32 >= eye_w {
            return None;
        }
        let mut sum = 0u64;
        for py in (y - half)..=(y + half) {
            for px in 0..=(2 * half) {
                let lx = x - half + px;
                let rxx = (rx - half as i32) as u32 + px + eye_w;
                sum += img.get_pixel(lx, py)[0].abs_diff(img.get_pixel(rxx, py)[0]) as u64;
            }
        }
        Some(sum)
    };
    let mut best: Option<(i32, u64)> = None;
    for dx in search.0..=search.1 {
        if let Some(sad) = sad_at(dx) {
            if best.is_none_or(|(_, b)| sad < b) {
                best = Some((dx, sad));
            }
        }
    }
    let (dx, sad) = best?;
    // Confidence: the match must clearly beat the zero-offset cost —
    // otherwise the patch is flat and every offset looks alike.
    let flat = sad_at(0)?;
    if dx != 0 && sad * 2 > flat {
        return None;
    }
    Some(dx as f32)
}

#[test]
fn stereo_pairs_obey_the_off_axis_geometry() {
    let level = *levels_of_planet(2)
        .first()
        .expect("the grammar declares planet 2");
    let run_seed = 1i64;
    let (spec, graph) = build_graph(level, run_seed);
    let poses = interior_probe_poses(&graph, spec.pitch, 8);
    let pose = vec![poses[0]];
    // Vertical FOV of the game camera (engine default; nothing overrides
    // it). Focal length in pixels follows from the capture resolution.
    const VFOV_DEG: f32 = 75.0;
    const S_BASE: f32 = 0.065;

    let shoot = |tag: &str, s: f32, convergence: f32| -> PathBuf {
        let dir = frames_dir(level, run_seed, tag);
        let extra = [
            format!("--interaxial={s:.4}"),
            format!("--convergence={convergence:.3}"),
        ];
        capture_ex(level, run_seed, &pose, true, true, &extra, &dir)
            .pop()
            .expect("one frame")
    };

    let r1 = shoot("-stereo-r1", S_BASE, 0.0);
    let r2 = shoot("-stereo-r2", S_BASE * 2.0, 0.0);

    let img = image::open(&r1).expect("pair opens").to_rgb8();
    let (w, h) = img.dimensions();
    assert_eq!(w, 1144, "SBS pair should span the full window");
    let eye_w = w / 2;
    let f_px = (h as f32 / 2.0) / (VFOV_DEG / 2.0).to_radians().tan();

    // Candidate patches across the world band (the cockpit band below and
    // the HUD above are excluded: near-shell disparity outranges the
    // search window, and HUD pixels are screen-locked).
    let half = 10u32;
    let search = (-70i32, 10i32);
    let mut measured: Vec<(u32, u32, f32, f32)> = Vec::new();
    for fy in [0.34f32, 0.44, 0.54] {
        for fx in [0.30f32, 0.50, 0.70] {
            let (x, y) = ((eye_w as f32 * fx) as u32, (h as f32 * fy) as u32);
            let (Some(d1), Some(d2)) = (
                patch_disparity(&r1, x, y, half, search),
                patch_disparity(&r2, x, y, half, search),
            ) else {
                continue;
            };
            // Parallel capture: finite depth reads as crossed (negative)
            // disparity; a patch at ~zero is sky-distant — useless here.
            if d1 > -3.0 {
                continue;
            }
            measured.push((x, y, d1, d2));
        }
    }
    assert!(
        measured.len() >= 3,
        "matcher sanity: only {} textured patches matched — the stereo \
         contracts have nothing to measure ({})",
        measured.len(),
        r1.display()
    );

    // LINEARITY: doubling s doubles the disparity, patch by patch.
    for &(x, y, d1, d2) in &measured {
        let tolerance = (0.18 * (2.0 * d1).abs()).max(2.5);
        assert!(
            (d2 - 2.0 * d1).abs() <= tolerance,
            "off-axis linearity broken at ({x},{y}): s doubled but \
             disparity went {d1:.1} -> {d2:.1} (expected ~{:.1})",
            2.0 * d1
        );
    }

    // ZERO PARALLAX: converge at the strongest patch's inferred depth and
    // that patch must land on the screen plane.
    let &(bx, by, bd1, _) = measured
        .iter()
        .max_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).unwrap())
        .unwrap();
    let z_best = f_px * S_BASE / (-bd1);
    let r3 = shoot("-stereo-r3", S_BASE, z_best);
    let d3 = patch_disparity(&r3, bx, by, half, search)
        .expect("the converged patch still matches");
    assert!(
        d3.abs() <= 2.5,
        "zero-parallax broken: converged at inferred z={z_best:.2} but the \
         subject patch still shows {d3:.1}px ({})",
        r3.display()
    );

    // THE SPHERICALITY RULE, rendered: pick two patches at clearly
    // different depths and give each THE SHIPPED RULE's own pair —
    // `interaxial_for(z)` (sphericity constant, owner clamps and all)
    // converged at z. Each subject must land at the screen plane: the
    // sweep the director performs continuously, verified at its
    // extremes with the very function that ships.
    let mut by_depth: Vec<(u32, u32, f32)> = measured
        .iter()
        .map(|&(x, y, d1, _)| (x, y, f_px * S_BASE / (-d1)))
        .collect();
    by_depth.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    let (near, far) = (by_depth[0], by_depth[by_depth.len() - 1]);
    assert!(
        far.2 / near.2 >= 1.25,
        "the vantage offers no depth spread ({:.2} vs {:.2}) — probe pose \
         unfit for the sphericality sweep",
        near.2,
        far.2
    );
    for (label, (x, y, z)) in [("near", near), ("far", far)] {
        let s_rule = void_logic::director::interaxial_for(z);
        let pair = shoot(&format!("-stereo-rule-{label}"), s_rule, z);
        let d = patch_disparity(&pair, x, y, half, search)
            .expect("the rule-pair subject still matches");
        assert!(
            d.abs() <= 2.5,
            "sphericality rule broken at {label} (z={z:.2}, s={s_rule:.4}): \
             subject shows {d:.1}px off the screen plane ({})",
            pair.display()
        );
    }
}


/// The credits crawl renders, fits, and moves (owner 2026-09-09: "can we
/// do the same thing with the menu as a way to validate that credits are
/// displayed?"): a `--screen=credits` boot parks the crawl at three
/// offsets — the column risen that far from below the bottom edge; every
/// frame carries text (the menu's backdrop is space, mostly black by
/// design, so the text is the render proof), no text runs off the
/// frame's left or right edge (the centered column fits the window), and
/// the frame changes from one offset to the next — the crawl moved.
/// `KEEP_FRAMES=1` keeps the frames under out/visual/ for a look at the
/// layout itself.
#[test]
fn credits_roll_renders_and_scrolls() {
    // Risen 400 px: the title and the first pairs are on screen; 1000 and
    // 1600: deeper into the roll.
    const OFFSETS: [f32; 3] = [400.0, 1000.0, 1600.0];
    // The outermost columns of the frame, either side: a centered column
    // that fits leaves them empty of text.
    const EDGE: f32 = 0.03;
    const MIN_TEXT: f32 = 0.001;
    const MAX_EDGE_TEXT: f32 = 0.0005;
    const MIN_MOVE: f32 = 0.5;

    let dir = frames_dir(0, 0, "credits");
    let frames = capture_screen("credits", &OFFSETS, &dir);
    let mut faults = Vec::new();
    for (frame, offset) in frames.iter().zip(OFFSETS) {
        let text = bright_fraction(frame, 0.0, 1.0, 0.0, 1.0);
        if text < MIN_TEXT {
            faults.push(format!(
                "offset {offset:.0}: {:.2}% bright pixels — no text on the crawl ({})",
                text * 100.0, frame.display()));
        }
        let left = bright_fraction(frame, 0.0, EDGE, 0.0, 1.0);
        let right = bright_fraction(frame, 1.0 - EDGE, 1.0, 0.0, 1.0);
        if left > MAX_EDGE_TEXT || right > MAX_EDGE_TEXT {
            faults.push(format!(
                "offset {offset:.0}: text at the frame's edges (left {:.2}%, right {:.2}%) — \
                 the column is wider than the window ({})",
                left * 100.0, right * 100.0, frame.display()));
        }
    }
    for (pair, offsets) in frames.windows(2).zip(OFFSETS.windows(2)) {
        let moved = band_diff(&pair[0], &pair[1], 0.0, 1.0);
        if moved < MIN_MOVE {
            faults.push(format!(
                "offsets {:.0} -> {:.0}: the frame changed by {moved:.2} — the crawl did not move",
                offsets[0], offsets[1]));
        }
    }
    assert!(faults.is_empty(), "credits crawl:\n{}", faults.join("\n"));
}
