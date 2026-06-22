.PHONY: deps deps-rust deps-godot deps-gut require-rust check test-rust test-godot demo edit clean run build build-release assets assets-materials

# Project-local tool paths
TOOLS_DIR := $(CURDIR)/tools
GODOT_APP := $(TOOLS_DIR)/Godot.app
GODOT := $(GODOT_APP)/Contents/MacOS/Godot
ASSETS_DIR := $(CURDIR)/assets
RUST_DIR := $(CURDIR)/rust
GODOT_DIR := $(CURDIR)/godot

# Godot version - update these when upgrading
GODOT_VERSION := 4.6.1
GODOT_RELEASE := stable
GODOT_ZIP := Godot_v$(GODOT_VERSION)-$(GODOT_RELEASE)_macos.universal.zip
GODOT_URL := https://github.com/godotengine/godot/releases/download/$(GODOT_VERSION)-$(GODOT_RELEASE)/$(GODOT_ZIP)

# GUT (Godot Unit Test)
GUT_VERSION := 9.6.0
GUT_URL := https://github.com/bitwes/Gut/archive/refs/tags/v$(GUT_VERSION).tar.gz
GUT_DIR := $(GODOT_DIR)/addons/gut

# Rust
CARGO := cargo
RUST_LIB := $(RUST_DIR)/target/debug/libvoid_scavenger.dylib

# Blender — used headless by `make assets` to decimate the cgtrader enemy
# meshes (21–26k tris) down to a game-weight glB.
BLENDER := /Applications/Blender.app/Contents/MacOS/Blender

# --- Targets ---

# One-time / occasional setup: installs the toolchains and tools. NOT a
# prerequisite of build/run/check — those just use what's already installed
# (see require-rust). Re-run after a machine setup or to update the toolchain.
deps: deps-rust deps-godot deps-gut
	@if [ -x "$(BLENDER)" ]; then \
		echo "Blender already installed ($$($(BLENDER) --version 2>/dev/null | head -1))."; \
	else \
		echo "==> Installing Blender (headless mesh decimation for `make assets`)..."; \
		brew install --cask blender; \
	fi
	@echo "All dependencies ready."

# Bootstrap + update the Rust toolchain (network). Explicit only — kept out of
# the build/run hot path so a flaky download can't break every command. The
# whole body runs in one shell with ~/.cargo/bin on PATH, so the rustup check
# actually sees an existing install instead of trying to reinstall it.
deps-rust:
	@echo "==> Setting up Rust toolchain..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		if ! command -v rustup >/dev/null 2>&1; then \
			echo "Installing rustup..."; \
			curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path; \
		fi && \
		rustup default stable && \
		rustup update stable && \
		(rustup component add clippy 2>/dev/null || true) && \
		echo "Rust $$(rustc --version) ready."

# Cheap guard for the build hot path: just confirm cargo exists (no network,
# no install). Points at `make deps` if the toolchain isn't set up yet.
require-rust:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && command -v cargo >/dev/null 2>&1 || { \
		echo "ERROR: Rust toolchain not found. Run 'make deps' once to install it."; exit 1; }

deps-godot:
	@if [ -x "$(GODOT)" ] && $(GODOT) --version 2>/dev/null | grep -q "^$(GODOT_VERSION)\."; then \
		echo "Godot $(GODOT_VERSION) already installed."; \
	else \
		echo "==> Downloading Godot $(GODOT_VERSION)..."; \
		rm -rf $(GODOT_APP); \
		mkdir -p $(TOOLS_DIR); \
		curl -L -o $(TOOLS_DIR)/$(GODOT_ZIP) $(GODOT_URL); \
		unzip -o -q $(TOOLS_DIR)/$(GODOT_ZIP) -d $(TOOLS_DIR); \
		rm -f $(TOOLS_DIR)/$(GODOT_ZIP); \
		echo "Godot $(GODOT_VERSION) installed to $(GODOT_APP)"; \
	fi

assets: build deps-godot
	@test -d $(ASSETS_DIR)/quaternius-megakit || { echo "ERROR: assets/ not found. Download paid assets manually into assets/."; exit 1; }
	@echo "==> Installing Godot addons from asset packs..."
	@BLENDER="$(BLENDER)" ./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR)
	@echo "Addons installed."
	@rm -f $(GODOT_DIR)/.godot/uid_cache.bin
	@chmod -R u+w $(GODOT_DIR)/.godot/imported 2>/dev/null; rm -rf $(GODOT_DIR)/.godot/imported || true
	@echo "==> Importing assets (pass 1: generates .import sidecars)..."
	$(GODOT) --headless --import --path $(GODOT_DIR)
	@echo "==> Configuring Quaternius material import script..."
	@find $(GODOT_DIR)/addons/quaternius -name "*.gltf.import" \
		-exec sed -i '' 's|import_script/path=""|import_script/path="res://addons/quaternius/quaternius_import_script.gd"|' {} +
	@echo "==> Configuring 3D texture settings (mipmaps + VRAM compression)..."
	@find $(GODOT_DIR)/addons/quaternius/materials -name "*.png.import" \
		-exec sed -i '' 's|mipmaps/generate=false|mipmaps/generate=true|' {} + \
		-exec sed -i '' 's|compress/mode=0|compress/mode=2|' {} +
	@echo "==> Reimporting assets (pass 2: with material script + mipmaps)..."
	$(GODOT) --headless --import --path $(GODOT_DIR)
	@$(MAKE) assets-materials
	@echo "==> Reimporting assets (pass 3: with restored materials)..."
	@rm -f $(GODOT_DIR)/.godot/uid_cache.bin
	$(GODOT) --headless --import --path $(GODOT_DIR)
	@echo "Import complete."

deps-gut:
	@if [ -d "$(GUT_DIR)" ]; then \
		echo "GUT $(GUT_VERSION) already installed."; \
	else \
		echo "==> Downloading GUT $(GUT_VERSION)..."; \
		curl -sL $(GUT_URL) -o /tmp/gut-$(GUT_VERSION).tar.gz; \
		tar xzf /tmp/gut-$(GUT_VERSION).tar.gz -C /tmp; \
		mkdir -p $(GUT_DIR); \
		cp -r /tmp/Gut-$(GUT_VERSION)/addons/gut/* $(GUT_DIR)/; \
		rm -rf /tmp/gut-$(GUT_VERSION).tar.gz /tmp/Gut-$(GUT_VERSION); \
		echo "==> Importing GUT class_names..."; \
		$(GODOT) --headless --import --path $(GODOT_DIR); \
		echo "GUT $(GUT_VERSION) installed to $(GUT_DIR)"; \
	fi

check: build deps-godot deps-gut
	@echo "==> Running Rust checks..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && \
		$(CARGO) clippy -- -D warnings && \
		$(CARGO) test
	@$(MAKE) test-godot
	@echo "All checks passed."

# Re-copies sanitized .tres materials from asset packs and re-applies
# local material patches (no reimport).
assets-materials:
	@echo "==> Restoring material definitions from asset packs..."
	@./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR) --tres-only
	@echo "==> Enabling anisotropic texture filtering on VisualShader materials..."
	@python3 -c "import re,sys;p=sys.argv[1];t=open(p).read();t=re.sub(r'(\[sub_resource type=\"VisualShaderNodeTexture2DParameter\"[^\]]*\]\nparameter_name = [^\n]+)',r'\1\ntexture_filter = 6',t);open(p,'w').write(t)" \
		$(GODOT_DIR)/addons/quaternius/materials/M_Trim_Base.tres

# Filtered Rust tests with output: make test-rust FILTER=test_name
test-rust:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) test $(FILTER) -- --nocapture

# Runs GUT against the currently installed dylib (no rebuild). Optional filters
# for the fast inner loop (skip the full suite): F selects scripts by filename
# substring, T narrows to a single test by name. With neither set, runs all:
#   make test-godot
#   make test-godot F=test_ship_select_backdrop
#   make test-godot F=test_ship_select_backdrop T=test_backdrop_is_structure_only
test-godot: deps-godot deps-gut
	@echo "==> Running Godot tests (GUT)$(if $(F), [F=$(F) T=$(T)])..."
	@GODOT_DISABLE_LEAK_CHECKS=1 $(GODOT) --headless --path $(GODOT_DIR) \
		-s res://addons/gut/gut_cmdln.gd \
		-gdir=res://tests -ginclude_subdirs \
		$(if $(F),-gselect=$(F)) $(if $(T),-gunit_test_name=$(T)) -gexit

build: require-rust
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) build
	@rm -f $(GODOT_DIR)/libvoid_scavenger.debug.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@cp $(RUST_DIR)/target/debug/libvoid_scavenger.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@echo "Build complete (debug)."

build-release: require-rust
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) build --release
	@rm -f $(GODOT_DIR)/libvoid_scavenger.debug.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@cp $(RUST_DIR)/target/release/libvoid_scavenger.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@echo "Build complete (release)."

run: build-release deps-godot
	@echo "==> Launching game (release)..."
	@$(GODOT) --path $(GODOT_DIR)

demo: build deps-godot
	@echo "==> Launching game (debug)..."
	@$(GODOT) --path $(GODOT_DIR)

# Godot editor: Debugger -> Monitors graphs the kinetics/* counters live.
edit: build deps-godot
	@echo "==> Opening Godot editor..."
	@$(GODOT) --editor --path $(GODOT_DIR)

clean:
	@echo "==> Cleaning..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && cd $(RUST_DIR) && $(CARGO) clean
	@echo "Clean."

nuke: clean
	@echo "==> Removing local tools and derived Godot files..."
	@chmod -R u+w $(TOOLS_DIR) 2>/dev/null; rm -rf $(TOOLS_DIR)
	@chmod -R u+w $(GODOT_DIR)/addons 2>/dev/null; rm -rf $(GODOT_DIR)/addons
	@chmod -R u+w $(GODOT_DIR)/.godot 2>/dev/null; rm -rf $(GODOT_DIR)/.godot
	@echo "Nuked. (assets/ preserved)"
