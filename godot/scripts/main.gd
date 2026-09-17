extends Node3D

func _ready() -> void:
	# Anti-aliasing is owned by ViewManager and driven by the persisted
	# GameOptions (loaded from disk by GameManager, then broadcast) onto the
	# root viewport, which renders the world through the display interface
	# (use_xr). Setting AA here would be a second writer to the same
	# viewport — a parallel pathway that once mis-measured.
	print("Void Scavenger loaded.")
	print("Controls: WASD + Space/Ctrl for movement, Arrows + Q/E for rotation")
	print("Press F3 to toggle SBS stereo, F4 to toggle dynamic 3D")
func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and event.keycode == KEY_F3:
		$GameManager.on_sbs_toggled()
	elif event is InputEventKey and event.pressed and event.keycode == KEY_F4:
		$GameManager.on_dynamic_stereo_toggled()