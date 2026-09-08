extends GutTest
## The texture-size limits the scenes ship against, read from the engine
## that runs them, never from memory (owner 2026-09-06: "it's a godot
## limit? ... gonna need to see some evidence of that"). Two limits
## exist: the Image class's (what a texture may be in RAM) and the GPU's
## (what the rendering device will upload); the environment door's cap
## (install-addons.sh ENV_TEX_CAP) must sit under both. The GPU number
## also prints in the game's startup display line.

const ENV_TEX_CAP = 16384  # mirrors install-addons.sh ENV_TEX_CAP


func test_the_environment_cap_fits_the_engine_image_limit():
	gut.p("Image.MAX_WIDTH = %d, Image.MAX_HEIGHT = %d" % [Image.MAX_WIDTH, Image.MAX_HEIGHT])
	assert_true(ENV_TEX_CAP <= Image.MAX_WIDTH and ENV_TEX_CAP <= Image.MAX_HEIGHT,
		"the environment texture cap must be an image the engine can hold")


func test_the_environment_cap_fits_the_gpu_texture_limit():
	var device := RenderingServer.get_rendering_device()
	if device == null:
		gut.p("no rendering device in this run (headless dummy renderer): the GPU limit prints at game startup instead")
		pass_test("headless: GPU limit not queryable here")
		return
	var side: int = device.limit_get(RenderingDevice.LIMIT_MAX_TEXTURE_SIZE_2D)
	gut.p("RenderingDevice LIMIT_MAX_TEXTURE_SIZE_2D = %d" % side)
	assert_true(ENV_TEX_CAP <= side,
		"the environment texture cap (%d) must not exceed the GPU's 2D texture side (%d)"
			% [ENV_TEX_CAP, side])
