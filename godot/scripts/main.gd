extends Node3D

func _ready() -> void:
	print("Void Scavenger loaded.")
	print("Controls: WASD + Space/Ctrl for movement, Arrows + Q/E for rotation")
	print("Press F3 to toggle SBS stereo")

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and event.keycode == KEY_F3:
		var rig = $Player/StereoRig
		if rig and rig.has_method("toggle_stereo"):
			rig.toggle_stereo()
			# Swap which camera is active
			$Player/Camera3D.current = not rig.enabled
