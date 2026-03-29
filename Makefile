.PHONY: deps check demo clean run assets

# Project-local tool paths
TOOLS_DIR := $(CURDIR)/tools
GODOT_APP := $(TOOLS_DIR)/Godot.app
GODOT := $(GODOT_APP)/Contents/MacOS/Godot
ASSETS_DIR := $(CURDIR)/assets
RUST_DIR := $(CURDIR)/rust
GODOT_DIR := $(CURDIR)/godot

# Godot version - update these when upgrading
GODOT_VERSION := 4.4.1
GODOT_RELEASE := stable
GODOT_ZIP := Godot_v$(GODOT_VERSION)-$(GODOT_RELEASE)_macos.universal.zip
GODOT_URL := https://github.com/godotengine/godot/releases/download/$(GODOT_VERSION)-$(GODOT_RELEASE)/$(GODOT_ZIP)

# Rust
CARGO := cargo
RUST_LIB := $(RUST_DIR)/target/debug/libvoid_scavenger.dylib

# --- Targets ---

deps: deps-rust deps-godot deps-assets
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

deps-godot: $(GODOT)

$(GODOT):
	@echo "==> Downloading Godot $(GODOT_VERSION)..."
	@mkdir -p $(TOOLS_DIR)
	@curl -L -o $(TOOLS_DIR)/$(GODOT_ZIP) $(GODOT_URL)
	@unzip -o -q $(TOOLS_DIR)/$(GODOT_ZIP) -d $(TOOLS_DIR)
	@rm -f $(TOOLS_DIR)/$(GODOT_ZIP)
	@echo "Godot $(GODOT_VERSION) installed to $(GODOT_APP)"

deps-assets:
	@echo "==> Downloading assets..."
	@mkdir -p $(ASSETS_DIR)
	@./scripts/fetch-assets.sh $(ASSETS_DIR)
	@echo "Assets ready."

assets: deps-assets $(GODOT)
	@echo "==> Installing Godot addons from asset packs..."
	@./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR)
	@echo "Addons installed."
	@echo "==> Importing assets (pass 1: generate .import sidecars)..."
	@rm -f $(GODOT_DIR)/.godot/uid_cache.bin
	@chmod -R u+w $(GODOT_DIR)/.godot/imported 2>/dev/null; rm -rf $(GODOT_DIR)/.godot/imported || true
	@$(GODOT) --headless --import --path $(GODOT_DIR) 2>/dev/null || true
	@echo "==> Configuring Quaternius material import script..."
	@find $(GODOT_DIR)/addons/quaternius -name "*.gltf.import" \
		-exec sed -i '' 's|import_script/path=""|import_script/path="res://addons/quaternius/quaternius_import_script.gd"|' {} +
	@echo "==> Importing assets (pass 2: bake materials via import script)..."
	@$(GODOT) --headless --import --path $(GODOT_DIR) 2>/dev/null || true
	@echo "==> Restoring material definitions (undo Godot path rewrites)..."
	@./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR) --tres-only
	@echo "Import complete."

check: deps
	@echo "==> Running checks..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && \
		$(CARGO) clippy -- -D warnings && \
		$(CARGO) test
	@echo "All checks passed."

build:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) build
	@echo "Build complete."

run: build $(GODOT)
	@echo "==> Launching game..."
	@$(GODOT) --path $(GODOT_DIR)

demo: build $(GODOT)
	@echo "==> Running demo..."
	@$(GODOT) --path $(GODOT_DIR)

clean:
	@echo "==> Cleaning..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && cd $(RUST_DIR) && $(CARGO) clean
	@echo "Clean."

nuke: clean
	@echo "==> Removing all local tools and assets..."
	@rm -rf $(TOOLS_DIR) $(ASSETS_DIR)
	@echo "Nuked."
