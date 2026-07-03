extends Node3D

func _ready() -> void:
	# Anti-aliasing is owned by ViewManager and driven by the persisted
	# GameOptions (loaded from disk by GameManager, then broadcast). The root
	# viewport doesn't render the 3D world — the eye sub-viewports do — so we
	# must NOT set AA here; doing so was a parallel pathway that mis-measured.
	print("Void Scavenger loaded.")
	print("Controls: WASD + Space/Ctrl for movement, Arrows + Q/E for rotation")
	print("Press F3 to toggle SBS stereo")

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and event.keycode == KEY_F3:
		$GameManager.on_sbs_toggled()
