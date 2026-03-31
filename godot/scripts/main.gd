extends Node3D

func _ready() -> void:
	get_viewport().msaa_3d = Viewport.MSAA_4X
	get_viewport().use_taa = true
	print("AA status: MSAA=%s TAA=%s" % [get_viewport().msaa_3d, get_viewport().use_taa])
	print("Void Scavenger loaded.")
	print("Controls: WASD + Space/Ctrl for movement, Arrows + Q/E for rotation")
	print("Press F3 to toggle SBS stereo")

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and event.keycode == KEY_F3:
		$GameManager.on_sbs_toggled()
