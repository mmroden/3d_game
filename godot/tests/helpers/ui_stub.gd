extends CanvasLayer
# Consumed via preload: `const UiStub := preload("res://tests/helpers/ui_stub.gd")`
# — explicit, and immune to the editor's script-class cache (class_name
# registration is not guaranteed in headless runs).
## The union of UI-node contracts GameManager wires (signals) and calls
## (methods), as no-op stubs — one per UI node name lets a minimal scene
## exercise the real phase machinery without real screens. Tests that need
## to observe a push extend this and override the one method they record.

@warning_ignore("unused_signal")
signal new_game_selected
@warning_ignore("unused_signal")
signal continue_selected
@warning_ignore("unused_signal")
signal sbs_toggled(enabled: bool)
@warning_ignore("unused_signal")
signal msaa_toggled(enabled: bool)
@warning_ignore("unused_signal")
signal resume_selected
@warning_ignore("unused_signal")
signal quit_selected
@warning_ignore("unused_signal")
signal continue_pressed
@warning_ignore("unused_signal")
signal buy_pressed(item_id: int)
@warning_ignore("unused_signal")
signal return_pressed
@warning_ignore("unused_signal")
signal save_exit_pressed
@warning_ignore("unused_signal")
signal respawn_pressed
@warning_ignore("unused_signal")
signal ship_color_selected(id: int)
@warning_ignore("unused_signal")
signal ship_type_selected(id: int)
@warning_ignore("unused_signal")
signal bestiary_paged(delta: int)
@warning_ignore("unused_signal")
signal back_pressed


func set_continue_available(_a: bool, _restarts: bool) -> void: pass
func show_loading(_level: int) -> void: pass
func hide_loading() -> void: pass
func set_unlock_flags(_r: bool, _m: bool, _t: bool) -> void: pass
func set_radar_contacts(_ids: PackedInt64Array) -> void: pass
func show_death(_a: String, _b: String, _c: int) -> void: pass
func show_life_lost(_lives: int, _level: int) -> void: pass
func show_shop(_c: int, _o: int, _ids: PackedInt32Array, _labels: PackedStringArray, _details: PackedStringArray, _costs: PackedInt64Array, _flags: PackedByteArray) -> void: pass
func refresh_shop(_c: int, _o: int, _ids: PackedInt32Array, _labels: PackedStringArray, _details: PackedStringArray, _costs: PackedInt64Array, _flags: PackedByteArray) -> void: pass
func show_ship_select(_ship_id: int, _color_id: int, _owned: PackedByteArray) -> void: pass
func show_bestiary(_t: String, _b: String, _p: String, _h: String) -> void: pass
func begin_briefing() -> void: pass
func hide_bestiary() -> void: pass
func show_summary(_k: Dictionary, _c: int, _l: int) -> void: pass
func update_health(_h: float, _m: float) -> void: pass
func update_shield(_c: float, _m: float) -> void: pass
func update_power_mode(_m: int) -> void: pass
func update_components(_c: int) -> void: pass
func update_lives(_l: int) -> void: pass
func update_charges(_c: int) -> void: pass
func update_map(_rects: PackedFloat32Array, _flags: PackedByteArray, _projection: PackedFloat32Array) -> void: pass
func update_organics(_o: int) -> void: pass
func update_laser(_n: String, _c: Color) -> void: pass
func update_slow(_a: bool) -> void: pass
func update_level(_l: int) -> void: pass
