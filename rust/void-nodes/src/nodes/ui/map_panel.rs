use godot::prelude::*;
use godot::classes::{Control, IControl};

/// Corner inset (px) between the panel border and the drawn map.
const MAP_MARGIN: f32 = 10.0;
/// Side length (px) of a drawn room square.
const ROOM_SIZE: f32 = 8.0;

/// The recon-map widget (the FogMap unlock): a corner panel drawing the
/// rooms the player has visited and frontier stubs where unexplored
/// corridors leave them. Pure presentation — GameManager derives the view
/// from the retained LevelGraph in void-logic (`level_map::map_view`) and
/// pushes it here as packed arrays in unit-square coordinates; a redraw
/// happens only when a new room is visited, never per frame.
#[derive(GodotClass)]
#[class(base=Control)]
pub struct MapPanel {
    base: Base<Control>,
    /// Room centers (unit square).
    rooms: PackedVector2Array,
    /// Per room: bit0 = the player's current room.
    room_flags: PackedByteArray,
    /// Edge endpoint pairs (unit square): [from0, to0, from1, to1, …].
    edges: PackedVector2Array,
    /// Per edge: bit0 = frontier stub (unexplored corridor).
    edge_flags: PackedByteArray,
}

#[godot_api]
impl IControl for MapPanel {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            rooms: PackedVector2Array::new(),
            room_flags: PackedByteArray::new(),
            edges: PackedVector2Array::new(),
            edge_flags: PackedByteArray::new(),
        }
    }

    fn draw(&mut self) {
        let size = self.base().get_size();
        let scale = Vector2::new(size.x - 2.0 * MAP_MARGIN, size.y - 2.0 * MAP_MARGIN);
        if scale.x <= 0.0 || scale.y <= 0.0 {
            return;
        }
        let place = |unit: Vector2| Vector2::new(MAP_MARGIN, MAP_MARGIN) + unit * scale;

        // Faint backdrop so the map reads against the world behind it.
        let rooms = self.rooms.clone();
        let room_flags = self.room_flags.clone();
        let edges = self.edges.clone();
        let edge_flags = self.edge_flags.clone();
        let mut base = self.base_mut();
        base.draw_rect(Rect2::new(Vector2::ZERO, size), Color::from_rgba(0.0, 0.05, 0.08, 0.45));

        // Corridors first, under the rooms. Frontier stubs read amber —
        // "something unexplored leaves here" — full corridors cool grey-blue.
        for i in 0..edge_flags.len() {
            let (Some(from), Some(to)) = (edges.get(2 * i), edges.get(2 * i + 1)) else { continue };
            let frontier = edge_flags[i] & 1 != 0;
            let color = if frontier {
                Color::from_rgba(1.0, 0.75, 0.25, 0.9)
            } else {
                Color::from_rgba(0.45, 0.6, 0.75, 0.8)
            };
            base.draw_line_ex(place(from), place(to), color).width(2.0).done();
        }

        for i in 0..rooms.len() {
            let Some(pos) = rooms.get(i) else { continue };
            let center = place(pos);
            let half = ROOM_SIZE / 2.0;
            let rect = Rect2::new(center - Vector2::new(half, half), Vector2::new(ROOM_SIZE, ROOM_SIZE));
            base.draw_rect(rect, Color::from_rgba(0.35, 0.85, 0.8, 0.95));
            let is_current = room_flags.get(i).unwrap_or(0) & 1 != 0;
            if is_current {
                let ring = rect.grow(3.0);
                base.draw_rect_ex(ring, Color::from_rgba(1.0, 1.0, 1.0, 0.95))
                    .filled(false)
                    .width(2.0)
                    .done();
            }
        }
    }
}

#[godot_api]
impl MapPanel {
    /// Cache the freshly derived view and redraw. Called by the HUD when
    /// GameManager pushes a new-room update.
    #[func]
    pub fn update_map(
        &mut self,
        rooms: PackedVector2Array,
        room_flags: PackedByteArray,
        edges: PackedVector2Array,
        edge_flags: PackedByteArray,
    ) {
        self.rooms = rooms;
        self.room_flags = room_flags;
        self.edges = edges;
        self.edge_flags = edge_flags;
        self.base_mut().queue_redraw();
    }
}
