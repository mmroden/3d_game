use godot::prelude::*;
use godot::classes::{Control, IControl, Node3D};

use crate::nodes::constants::{groups, map_flags};

/// Corner inset (px) between the panel border and the drawn map.
const MAP_MARGIN: f32 = 10.0;
/// Player heading-arrow size (px).
const MARKER_SIZE: f32 = 7.0;

/// The recon-map widget (the FogMap unlock): a corner panel drawing the
/// level's REAL rectilinear footprints — every visited room and corridor as
/// its top-down rectangle, uniformly scaled (playtest 2026-07-04: abstract
/// dots were useless). The current room draws near-opaque, explored space
/// translucent, frontier corridors faint — overlapping stories stay legible
/// through the alpha. Pure presentation — GameManager derives the rects
/// from the retained LevelGraph (`level_map::map_rects`) and pushes them as
/// packed arrays in unit-square coordinates; a redraw happens only when a
/// new room is visited, never per frame.
#[derive(GodotClass)]
#[class(base=Control)]
pub struct MapPanel {
    base: Base<Control>,
    /// Footprints, stride 4: `[x, z, w, d]` per node (unit square).
    rects: PackedFloat32Array,
    /// Per node: `constants::map_flags` bits (current / corridor / frontier).
    flags: PackedByteArray,
    /// World→unit XZ transform `[scale, off_x, off_z]` — places the LIVE
    /// player marker between room-change pushes.
    projection: PackedFloat32Array,
}

#[godot_api]
impl IControl for MapPanel {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            rects: PackedFloat32Array::new(),
            flags: PackedByteArray::new(),
            projection: PackedFloat32Array::new(),
        }
    }

    fn process(&mut self, _delta: f64) {
        // The map rects change per room, but the player marker rides the
        // ship — redraw while shown (a corner panel of tens of rects).
        if self.base().is_visible_in_tree() && !self.projection.is_empty() {
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let size = self.base().get_size();
        let scale = (size.x - 2.0 * MAP_MARGIN).min(size.y - 2.0 * MAP_MARGIN);
        if scale <= 0.0 {
            return;
        }
        let origin = Vector2::new(MAP_MARGIN, MAP_MARGIN);

        let rects = self.rects.clone();
        let flags = self.flags.clone();
        let mut base = self.base_mut();
        // Faint backdrop so the map reads against the world behind it.
        base.draw_rect(Rect2::new(Vector2::ZERO, size), Color::from_rgba(0.0, 0.05, 0.08, 0.5));

        for i in 0..flags.len() {
            let Some(x) = rects.get(4 * i) else { continue };
            let (Some(z), Some(w), Some(d)) =
                (rects.get(4 * i + 1), rects.get(4 * i + 2), rects.get(4 * i + 3))
            else {
                continue;
            };
            let flag = flags[i];
            let current = flag & map_flags::CURRENT != 0;
            let corridor = flag & map_flags::CORRIDOR != 0;
            let frontier = flag & map_flags::FRONTIER != 0;

            // Opacity is the fog: the room you stand in is nearly solid,
            // explored space shows through, the frontier barely glows.
            let color = if current {
                Color::from_rgba(0.85, 0.95, 1.0, 0.95)
            } else if frontier {
                Color::from_rgba(1.0, 0.75, 0.25, 0.25)
            } else if corridor {
                Color::from_rgba(0.45, 0.6, 0.75, 0.4)
            } else {
                Color::from_rgba(0.35, 0.85, 0.8, 0.45)
            };
            let rect = Rect2::new(
                origin + Vector2::new(x, z) * scale,
                Vector2::new(w, d) * scale,
            );
            base.draw_rect(rect, color);
            if current {
                base.draw_rect_ex(rect.grow(2.0), Color::from_rgba(1.0, 1.0, 1.0, 0.95))
                    .filled(false)
                    .width(2.0)
                    .done();
            }
        }
        drop(base);

        // The live player marker: a heading triangle at the ship's actual
        // position (north-up map, the 6DOF-friendly convention — the arrow
        // turns, the map doesn't).
        if let Some((unit, yaw)) = self.player_map_pose() {
            let center = origin + Vector2::new(unit[0], unit[1]) * scale;
            // Ship forward is -Z rotated by yaw: (-sin, -cos) on the XZ
            // plane, which is exactly screen (x right, z down).
            let dir = Vector2::new(-yaw.sin(), -yaw.cos());
            let side = Vector2::new(-dir.y, dir.x);
            let points = PackedVector2Array::from(&[
                center + dir * MARKER_SIZE,
                center - dir * MARKER_SIZE * 0.6 + side * MARKER_SIZE * 0.6,
                center - dir * MARKER_SIZE * 0.6 - side * MARKER_SIZE * 0.6,
            ][..]);
            let mut base = self.base_mut();
            base.draw_colored_polygon(&points, Color::from_rgb(1.0, 1.0, 1.0));
        }
    }
}

#[godot_api]
impl MapPanel {
    /// Cache the freshly derived footprints and redraw. Called by the HUD
    /// when GameManager pushes a new-room update.
    #[func]
    pub fn update_map(
        &mut self,
        rects: PackedFloat32Array,
        flags: PackedByteArray,
        projection: PackedFloat32Array,
    ) {
        self.rects = rects;
        self.flags = flags;
        self.projection = projection;
        self.base_mut().queue_redraw();
    }

    /// The player's map-space pose: unit-square position + yaw, or None
    /// without a projection or a player in the tree.
    fn player_map_pose(&self) -> Option<([f32; 2], f32)> {
        if self.projection.len() < 3 {
            return None;
        }
        let (s, ox, oz) = (self.projection[0], self.projection[1], self.projection[2]);
        let player = self
            .base()
            .get_tree()
            .get_first_node_in_group(groups::PLAYER)?
            .try_cast::<Node3D>()
            .ok()?;
        let p = player.get_global_position();
        Some(([p.x * s + ox, p.z * s + oz], player.get_rotation().y))
    }
}
