.PHONY: deps check demo clean run assets test-godot

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

# Rust
CARGO := cargo
RUST_LIB := $(RUST_DIR)/target/debug/libvoid_scavenger.dylib

# --- Targets ---

deps: deps-rust deps-godot
	@echo "All dependencies ready."

deps-rust:
	@echo "==> Checking Rust toolchain..."
	@if ! command -v rustup >/dev/null 2>&1; then \
		echo "Installing rustup..."; \
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path; \
	fi
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		rustup default stable && \
		rustup update stable && \
		rustup component add clippy 2>/dev/null || true && \
		echo "Rust $$(rustc --version) ready."

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

assets: build
	@test -d $(ASSETS_DIR)/quaternius-megakit || { echo "ERROR: assets/ not found. Download paid assets manually into assets/."; exit 1; }
	@echo "==> Installing Godot addons from asset packs..."
	@./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR)
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
	@echo "==> Restoring material definitions (undo Godot path rewrites)..."
	@./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR) --tres-only
	@echo "==> Enabling anisotropic texture filtering on VisualShader materials..."
	@python3 -c "import re,sys;p=sys.argv[1];t=open(p).read();t=re.sub(r'(\[sub_resource type=\"VisualShaderNodeTexture2DParameter\"[^\]]*\]\nparameter_name = [^\n]+)',r'\1\ntexture_filter = 6',t);open(p,'w').write(t)" \
		$(GODOT_DIR)/addons/quaternius/materials/M_Trim_Base.tres
	@echo "Import complete."

test-godot: build
	@echo "==> Running Godot tests (GUT)..."
	@GODOT_DISABLE_LEAK_CHECKS=1 $(GODOT) --headless --path $(GODOT_DIR) \
		-s res://addons/gut/gut_cmdln.gd \
		-gdir=res://tests -ginclude_subdirs -gexit
	@echo "Godot tests complete."

check: deps
	@echo "==> Running checks..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && \
		$(CARGO) clippy -- -D warnings && \
		$(CARGO) test
	@echo "All checks passed."

build: deps
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) build
	@echo "Build complete."

run: build
	@echo "==> Launching game..."
	@$(GODOT) --path $(GODOT_DIR)

demo: build
	@echo "==> Running demo..."
	@$(GODOT) --path $(GODOT_DIR)

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
