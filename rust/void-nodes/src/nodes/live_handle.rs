//! Live handles to scene nodes — cached references that cannot dangle.
//!
//! Caching a `Gd<T>` in a struct field is a use-after-free waiting to happen:
//! Nodes are not reference-counted, so when the node is freed elsewhere (a
//! level regen, a parent clearing its children, a scene reload) the cached
//! handle dangles, and the next access — even a `.clone()` — panics. This bug
//! recurs because the hazard is invisible at the call site.
//!
//! These wrappers store only the node's [`InstanceId`] — a plain value that
//! cannot dangle — and re-resolve it on every access. A freed node resolves to
//! `None`/skip instead of crashing. There is no cached `Gd` to clone or call
//! into, so the use-after-free is *structurally unexpressible*, not merely
//! guarded. A source-scanning test (`no_raw_gd_node_fields`) forbids raw
//! `Gd<…>` struct fields so every cache is forced through here.
//!
//! See the `feedback_gd_handle_use_after_free` memory for the pattern.

use std::marker::PhantomData;

use godot::classes::Node;
use godot::obj::{Gd, Inherits, InstanceId};

/// A cached weak handle to a single scene node. The node can only be reached
/// through [`with`](LiveRef::with), which no-ops if it has been freed.
pub struct LiveRef<T: Inherits<Node>> {
    id: InstanceId,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Inherits<Node>> LiveRef<T> {
    /// Capture a node's identity for later, validity-checked access.
    pub fn new(node: &Gd<T>) -> Self {
        Self { id: node.instance_id(), _marker: PhantomData }
    }

    /// Whether the node is still in the scene tree (not freed).
    pub fn alive(&self) -> bool {
        Gd::<T>::try_from_instance_id(self.id).is_ok()
    }

    /// Resolve the node and run `f`, returning its result — or `None` if the
    /// node has been freed. Never panics.
    pub fn with<R>(&self, f: impl FnOnce(&mut Gd<T>) -> R) -> Option<R> {
        let mut node = Gd::<T>::try_from_instance_id(self.id).ok()?;
        Some(f(&mut node))
    }
}

/// Ergonomic access for an `Option<LiveRef<T>>` field (the common "set on
/// `ready`, used later" shape): `self.label.with(|l| l.set_text(...))`.
pub trait LiveOpt<T: Inherits<Node>> {
    fn with<R>(&self, f: impl FnOnce(&mut Gd<T>) -> R) -> Option<R>;
}

impl<T: Inherits<Node>> LiveOpt<T> for Option<LiveRef<T>> {
    fn with<R>(&self, f: impl FnOnce(&mut Gd<T>) -> R) -> Option<R> {
        self.as_ref().and_then(|r| r.with(f))
    }
}

/// A cached weak handle to a list of nodes, each carrying associated data `D`
/// (e.g. a blinking light's base energy). Freed nodes are skipped on iteration.
pub struct LiveVec<T: Inherits<Node>, D = ()> {
    entries: Vec<(InstanceId, D)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Inherits<Node>, D> LiveVec<T, D> {
    pub fn new() -> Self {
        Self { entries: Vec::new(), _marker: PhantomData }
    }

    /// Track `node`, attaching `data`.
    pub fn push(&mut self, node: &Gd<T>, data: D) {
        self.entries.push((node.instance_id(), data));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of tracked handles, alive or not (i.e. the next insertion index).
    /// Use [`live_count`](LiveVec::live_count) for how many are still in the tree.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Resolve the node at `index` for immediate use, or `None` if the index is
    /// out of range or that node has been freed. The returned `Gd` is a fresh
    /// resolution — use it now, don't cache it (that would reintroduce the
    /// dangling-handle hazard this type exists to prevent).
    pub fn get_live(&self, index: usize) -> Option<Gd<T>> {
        let (id, _) = self.entries.get(index)?;
        Gd::<T>::try_from_instance_id(*id).ok()
    }

    /// How many tracked nodes are still alive (resolves each id).
    pub fn live_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|(id, _)| Gd::<T>::try_from_instance_id(*id).is_ok())
            .count()
    }

    /// Run `f(index, node, data)` for each still-alive entry, in insertion
    /// order. `index` is the stable position in the list — dead entries are
    /// skipped, not reindexed — so callers can keep it aligned with a parallel
    /// `Vec` (e.g. room bounds). Never resolves a freed node.
    pub fn for_each_live(&self, mut f: impl FnMut(usize, &mut Gd<T>, &D)) {
        for (i, (id, data)) in self.entries.iter().enumerate() {
            if let Ok(mut node) = Gd::<T>::try_from_instance_id(*id) {
                f(i, &mut node, data);
            }
        }
    }

    /// Keep each still-alive entry only while `f` returns `true`; drop freed
    /// nodes and the ones `f` rejects. For self-pruning lists like aging beams
    /// (`f` ages the node and returns whether it's still alive).
    pub fn retain_live(&mut self, mut f: impl FnMut(&mut Gd<T>, &mut D) -> bool) {
        self.entries.retain_mut(|(id, data)| {
            match Gd::<T>::try_from_instance_id(*id) {
                Ok(mut node) => f(&mut node, data),
                Err(_) => false,
            }
        });
    }
}

impl<T: Inherits<Node>, D> Default for LiveVec<T, D> {
    fn default() -> Self {
        Self::new()
    }
}

// --- Test seam: a node that exercises LiveRef from GUT (which can't construct a
// Rust LiveRef directly). Lets the wrapper's safety be tested once, here, and
// inherited by every cache built on it. ---

use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct LiveHandleProbe {
    base: Base<Node>,
    tracked: Option<LiveRef<Node>>,
}

#[godot_api]
impl INode for LiveHandleProbe {
    fn init(base: Base<Node>) -> Self {
        Self { base, tracked: None }
    }
}

#[godot_api]
impl LiveHandleProbe {
    /// Start tracking `node` by identity.
    #[func]
    fn track(&mut self, node: Gd<Node>) {
        self.tracked = Some(LiveRef::new(&node));
    }

    /// Whether the tracked node is still alive.
    #[func]
    fn tracked_alive(&self) -> bool {
        self.tracked.as_ref().is_some_and(|r| r.alive())
    }

    /// Touch the tracked node (reads its name). Returns whether the access ran;
    /// after the node is freed this must return `false`, never panic.
    #[func]
    fn touch(&self) -> bool {
        self.tracked.with(|n| n.get_name()).is_some()
    }
}

// --- The chokepoint enforcement: a source-scanning lint that forbids any raw
// `Gd<…>` struct field in the node crate. Cached node handles must go through
// LiveRef/LiveVec above, so a freed node is structurally unreachable. There is
// deliberately no escape hatch — `#[total ban]` per the design decision. ---
#[cfg(test)]
mod lint {
    use std::path::Path;

    #[test]
    fn no_raw_gd_node_fields() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut violations = Vec::new();
        scan_dir(&src, &mut violations);
        assert!(
            violations.is_empty(),
            "raw `Gd<…>` struct fields are forbidden — a cached node handle \
             dangles when the node is freed elsewhere (use-after-free). Store an \
             InstanceId via `LiveRef`/`LiveVec` from `nodes::live_handle` \
             instead:\n{}",
            violations.join("\n"),
        );
    }

    fn scan_dir(dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                scan_text(&text, &name, out);
            }
        }
    }

    /// Parse one source string and append any `Gd<…>` struct fields to `out`.
    /// The file scanner and the self-test below share this so the test exercises
    /// the real detection logic.
    fn scan_text(text: &str, file: &str, out: &mut Vec<String>) {
        if let Ok(parsed) = syn::parse_file(text) {
            for item in &parsed.items {
                scan_item(item, file, out);
            }
        }
    }

    fn scan_item(item: &syn::Item, file: &str, out: &mut Vec<String>) {
        match item {
            syn::Item::Struct(s) => {
                for field in &s.fields {
                    // Whitespace-stripped so both `Gd<T>` and `Gd < T >` match,
                    // while `GdSomething<T>` (no `Gd<`) does not.
                    let ty = quote::quote!(#field).to_string().replace(' ', "");
                    if ty.contains("Gd<") {
                        let fname = field
                            .ident
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default();
                        out.push(format!("  {file}: {}.{fname}", s.ident));
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    for it in items {
                        scan_item(it, file, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// The scanner itself is the thing that has to work — pin that it flags a
    /// raw `Gd<…>` field (in plain, `Option`, `Vec`, and nested-module form) and
    /// leaves `LiveRef`/`LiveVec`/`Base` alone. Guards against a lint that
    /// silently passes everything.
    #[test]
    fn scanner_flags_raw_gd_and_allows_live_handles() {
        let mut bad = Vec::new();
        scan_text(
            "struct A { x: Gd<Node>, y: Option<Gd<Label>>, z: Vec<Gd<Node3D>> }\
             mod m { struct B { w: Gd<Camera3D> } }",
            "synthetic.rs",
            &mut bad,
        );
        assert_eq!(bad.len(), 4, "must flag every raw Gd field, nested too: {bad:?}");

        let mut good = Vec::new();
        scan_text(
            "struct C { base: Base<Node>, a: Option<LiveRef<Label>>, \
             b: LiveVec<Node3D>, c: GdSomethingElse<i32> }",
            "synthetic.rs",
            &mut good,
        );
        assert!(good.is_empty(), "must not flag LiveRef/LiveVec/Base/GdSomething: {good:?}");
    }
}
