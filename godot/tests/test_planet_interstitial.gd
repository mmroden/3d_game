extends GutTest
## The loading veil doubles as the planet-arrival interstitial (B10): the
## first sector of every planet past the first announces itself with a
## title and a flavor line (the story channel); ordinary sector entries
## stay a plain build veil.

func _veil_text(ui: LoadingUI) -> String:
	var out := ""
	for label in ui.find_children("*", "Label", true, false):
		out += label.text
	return out

func test_the_veil_announces_new_planets_and_only_those():
	var ui := LoadingUI.new()
	add_child_autofree(ui)

	ui.show_loading(7)
	assert_string_contains(_veil_text(ui), "PLANET 2",
		"level 7 is the first step onto planet 2")
	assert_string_contains(_veil_text(ui), "ENTERING SECTOR 7",
		"the interstitial still reads as the build veil")

	ui.show_loading(8)
	var plain := _veil_text(ui)
	assert_false(plain.contains("PLANET"),
		"mid-planet sectors are ordinary entries: %s" % plain)

	ui.show_loading(1)
	assert_false(_veil_text(ui).contains("PLANET"),
		"planet 1 needs no introduction")

	ui.show_loading(13)
	assert_string_contains(_veil_text(ui), "PLANET 3")
