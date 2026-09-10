.PHONY: deps deps-rust deps-godot deps-gut lsp-up require-rust check check-visual test-rust test-godot test-assets demo edit clean run build build-release assets assets-install assets-import assets-probe assets-materials

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
GLTF_TRANSFORM_VERSION := 4.5.0
NODE_DIR := $(TOOLS_DIR)/node
GLTF_TRANSFORM := $(NODE_DIR)/node_modules/.bin/gltf-transform

# Rust
CARGO := cargo
RUST_LIB := $(RUST_DIR)/target/debug/libvoid_scavenger.dylib

# Blender — used headless by `make assets` to decimate the cgtrader enemy
# meshes (21–26k tris) down to a game-weight glB.
BLENDER := /Applications/Blender.app/Contents/MacOS/Blender

# --- Targets ---

PYENV := tools/pyenv

# One-time / occasional setup: installs the toolchains and tools. NOT a
# prerequisite of build/run/check — those just use what's already installed
# (see require-rust). Re-run after a machine setup or to update the toolchain.
# One stage, everything inline: Blender (headless mesh work), git-lfs (provider
# files over GitHub's 100 MB limit — without it a clone gets pointer stubs),
# the asset-pipeline python venv (system python; the brew one has a broken
# ensurepip, 2026-07-12), the io_scene_max Blender extension (.max material
# recovery, installed via Blender's own extension system), and the headless
# Godot editor LSP (via lsp-up below — see that target's comment).
deps: deps-rust deps-godot deps-gut deps-node deps-blender lsp-up
	@if ! command -v git-lfs >/dev/null 2>&1; then \
		echo "==> Installing git-lfs (large provider assets)..."; \
		brew install git-lfs; \
	fi
	@git lfs install --local >/dev/null
	@if [ ! -x "$(PYENV)/bin/python3" ]; then \
		echo "==> Creating asset-pipeline python venv ($(PYENV))..."; \
		/usr/bin/python3 -m venv $(PYENV); \
	fi
	@$(PYENV)/bin/pip install --quiet "olefile==0.47" "pytest==8.4.1"
	@echo "All dependencies ready."

# Blender and the 3ds Max importer extension the .max material-table
# extraction needs. Extensions install per Blender version: a Blender
# upgrade (5.1 -> 5.2, 2026-09) left io_scene_max behind, and every .max
# extraction failed for days behind a summary grep while the converter
# read stale tables. The asset stage depends on this target, so the check
# runs before every install, not only on `make deps`.
deps-blender:
	@if [ -x "$(BLENDER)" ]; then \
		echo "Blender already installed ($$($(BLENDER) --version 2>/dev/null | head -1))."; \
	else \
		echo "==> Installing Blender (headless conversions for 'make assets')..."; \
		brew install --cask blender; \
	fi
	@if ! $(BLENDER) --command extension list 2>/dev/null | grep -q "io_scene_max.*\[installed\]"; then \
		echo "==> Installing io_scene_max via Blender extensions..."; \
		$(BLENDER) --online-mode --command extension install --sync --enable io_scene_max; \
	fi

# The headless Godot editor LSP Serena's GDScript support dials (TCP 6008,
# dialed ONCE at Serena MCP startup — so it must listen BEFORE Serena
# boots). scripts/serena-launch.sh (the .mcp.json entry point) runs this
# on every Serena start; `make deps` includes it for hand-run setups.
# pid -> out/godot-lsp.pid; kill that to stop it.
lsp-up:
	@if lsof -nP -iTCP:6008 -sTCP:LISTEN >/dev/null 2>&1; then \
		echo "Godot LSP already listening on 6008."; \
	else \
		echo "==> Starting headless Godot editor LSP on 6008..."; \
		mkdir -p out; \
		nohup $(GODOT) --headless --editor --lsp-port 6008 --path $(GODOT_DIR) > out/godot-lsp.log 2>&1 & echo $$! > out/godot-lsp.pid; \
		ok=0; for i in $$(seq 1 30); do \
			if lsof -nP -iTCP:6008 -sTCP:LISTEN >/dev/null 2>&1; then ok=1; break; fi; sleep 1; \
		done; \
		[ $$ok -eq 1 ] || { echo "ERROR: Godot LSP did not come up (see out/godot-lsp.log)"; exit 1; }; \
		echo "Godot LSP up (pid $$(cat out/godot-lsp.pid))."; \
	fi

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

# The asset pipeline, assembled from its three stages and the audit that
# judges their product — each stage independently runnable when only its
# stage changed (a Transform edit needs only `make assets-install
# assets-probe`; a material fix only `make assets-import assets-probe`).
# The audit (test-assets) always runs last: a red contract fails the
# stage itself, never a step chained by hand afterwards (owner
# 2026-09-07). Correctness pressure and reproducibility pressure,
# granularly.
assets: assets-install assets-import assets-probe credits test-assets

# Attributed downloads (catalog/attributions.toml): every third-party
# file the game fetches by URL rather than buys, pinned by checksum and
# credited on the generated credits page. Runs before the install stage;
# a missing pin or a mismatch fails the stage (reproducibility first).
assets-fetch:
	@test -x "$(PYENV)/bin/python3" || { echo "ERROR: python venv missing — run 'make deps'"; exit 1; }
	@echo "==> Fetching attributed downloads (catalog/attributions.toml)..."
	@$(PYENV)/bin/python3 scripts/fetch_attributed.py catalog/attributions.toml $(CURDIR)

# The credits, two steps. First the ingestion pipeline's first reading:
# every sound file's own credits (RIFF INFO / bext, ID3v2 tags) into
# catalog/audio_credits.generated.toml, so a new pack's artist reaches the
# credits audit the moment it lands (owner 2026-09-09: "we will want to
# have that new data carried forward"). Then the page: docs/CREDITS.md
# rendered from catalog/attributions.toml, generated and committed. Pure
# Python, seconds; the audit holds the committed reading to a fresh one,
# every tagged artist to the source that claims the file, and the page to
# its render. Runs before the install stage and again inside `make assets`.
credits:
	@test -x "$(PYENV)/bin/python3" || { echo "ERROR: python venv missing — run 'make deps'"; exit 1; }
	@echo "==> Reading the audio files' own credits (catalog/audio_credits.generated.toml)..."
	@$(PYENV)/bin/python3 scripts/audio_credits.py $(CURDIR) catalog/audio_credits.generated.toml
	@$(PYENV)/bin/python3 scripts/credits.py catalog/attributions.toml docs/CREDITS.md

# Door 1: provider packs -> installed addons (Blender conversions,
# panel splits + role bakes; split logs land in out/split-<kit>.log).
assets-install: build deps-godot deps-node deps-blender assets-fetch credits
	@test -d $(ASSETS_DIR)/quaternius-megakit || { echo "ERROR: assets/ not found. Download paid assets manually into assets/."; exit 1; }
	@echo "==> Installing Godot addons from asset packs..."
	@mkdir -p out; set -o pipefail; BLENDER="$(BLENDER)" GLTF_TRANSFORM="$(GLTF_TRANSFORM)" GLTF_TRANSFORM_VERSION="$(GLTF_TRANSFORM_VERSION)" ./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR) 2>&1 | tee out/assets-install.log
	@python3 scripts/log-histogram.py assets-install out/metrics/log_assets-install.toml out/assets-install.log $$(ls out/convert-*.log out/split-*.log out/decimate-*.log out/cockpit-*.log out/hull-*.log out/manifest-*.log out/max-*.log 2>/dev/null)
	@echo "Addons installed."

# Door 2: the three-pass Godot import (sidecars -> material script +
# mipmaps -> restored materials; invokes assets-materials between
# passes 2 and 3).
assets-import: deps-godot
	@rm -f $(GODOT_DIR)/.godot/uid_cache.bin
	@chmod -R u+w $(GODOT_DIR)/.godot/imported 2>/dev/null; rm -rf $(GODOT_DIR)/.godot/imported || true
	@echo "==> Importing assets (pass 1: generates .import sidecars)..."
	@# Every pass's output is kept (out/assets-import-passN.log) and
	@# condensed by log-histogram.py below into an error/warning histogram
	@# the audit holds to zero: Godot exits 0 while dropping 11,315 hill
	@# house surfaces past its 256-per-mesh cap (22,630 ERROR lines that
	@# sat unread for days, owner 2026-09-07).
	@mkdir -p out; set -o pipefail; $(GODOT) --headless --import --path $(GODOT_DIR) 2>&1 | tee out/assets-import-pass1.log
	@echo "==> Configuring Quaternius material import script..."
	@find $(GODOT_DIR)/addons/quaternius -name "*.gltf.import" \
		-exec sed -i '' 's|import_script/path=""|import_script/path="res://addons/quaternius/quaternius_import_script.gd"|' {} +
	@echo "==> Configuring fixed-environment mesh import (no LODs, no attribute compression)..."
	@# A scene is ONE monolithic mesh (the hill house: 3.9M triangles in a
	@# 175 m box). Godot's default LOD generation simplifies that mesh as a
	@# whole and thins its small parts away — lamp chains, seat cushions
	@# (owner 2026-09-06: "lots of details didn't make it through") — and
	@# 16-bit attribute compression quantizes positions over the whole box
	@# and UVs over their full tiled range. Both are for props, not places.
	@find $(GODOT_DIR)/addons/environments -name "*.glb.import" \
		-exec sed -i '' 's|meshes/generate_lods=true|meshes/generate_lods=false|; s|meshes/force_disable_compression=false|meshes/force_disable_compression=true|' {} +
	@echo "==> Configuring 3D texture imports (mipmaps on, no VRAM compression, no detect-3D flip)..."
	@# Every imported texture under addons/ (the kit's, the scenes' and the
	@# models' extracted maps, the hulls'): mipmaps are filtering and stay
	@# on; VRAM compression is a memory trade the owner never asked for (it
	@# came in with a 2026-04-02 refactor of mine; owner 2026-09-08: "I
	@# don't recall wanting to use compression"); detect_3d would flip a
	@# texture to compressed the first time an editor session used it in
	@# 3D. The audit holds every sidecar to all three.
	@find $(GODOT_DIR)/addons \( -name "*.png.import" -o -name "*.jpg.import" -o -name "*.jpeg.import" \) \
		-exec sed -i '' -e 's|^mipmaps/generate=.*|mipmaps/generate=true|' \
			-e 's|^compress/mode=.*|compress/mode=0|' \
			-e 's|^detect_3d/compress_to=.*|detect_3d/compress_to=0|' {} +
	@# Skies (godot/addons/sky, catalog/kits.toml [skies]): HDR float
	@# panoramas keep their range only as VRAM Uncompressed (mode 3) —
	@# lossless and lossy quantize to 8 bits, VRAM Compressed blocks the
	@# point stars. Mipmaps on for the horizon's glancing angles. The
	@# audit holds every sky sidecar to it.
	@if [ -d $(GODOT_DIR)/addons/sky ]; then \
		find $(GODOT_DIR)/addons/sky \( -name "*.exr.import" -o -name "*.hdr.import" \) \
			-exec sed -i '' -e 's|^mipmaps/generate=.*|mipmaps/generate=true|' \
				-e 's|^compress/mode=.*|compress/mode=3|' \
				-e 's|^detect_3d/compress_to=.*|detect_3d/compress_to=0|' {} + ; \
	fi
	@echo "==> Reimporting assets (pass 2: with material script + mipmaps)..."
	@set -o pipefail; $(GODOT) --headless --import --path $(GODOT_DIR) 2>&1 | tee out/assets-import-pass2.log
	@$(MAKE) assets-materials
	@echo "==> Reimporting assets (pass 3: with restored materials)..."
	@rm -f $(GODOT_DIR)/.godot/uid_cache.bin
	@set -o pipefail; $(GODOT) --headless --import --path $(GODOT_DIR) 2>&1 | tee out/assets-import-pass3.log
	@python3 scripts/log-histogram.py assets-import out/metrics/log_assets-import.toml out/assets-import-pass1.log out/assets-import-pass2.log out/assets-import-pass3.log
	@echo "Import complete."

# Door 3: the final accounting — probe the installed assets into the
# generated catalogs the linkers resolve against (the enemy-model map,
# each kit's grid + metrics). Runs alone after any Transform-only change:
# `make assets-install assets-probe`.
assets-probe:
	@echo "==> Probing installed assets into the asset catalogs..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) test -p void_logic --quiet probe_installed_assets -- --ignored >/dev/null
	@echo "Catalogs regenerated."

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

# glTF-Transform (Node): the glb-stage inspector and optimizer the
# environment stage runs after export — inspect/validate for the metrics,
# dedup + join + prune so a scene's clone materials merge and its
# surfaces stay under Godot's 256-per-mesh cap. Pinned and project-local
# (tools/node), never a global npm install (owner 2026-09-07).
deps-node:
	@if ! command -v node >/dev/null 2>&1; then \
		echo "==> Installing Node (glTF-Transform host)..."; \
		brew install node; \
	fi
	@if [ -x "$(GLTF_TRANSFORM)" ] && $(GLTF_TRANSFORM) --version 2>/dev/null | grep -q "$(GLTF_TRANSFORM_VERSION)"; then \
		echo "glTF-Transform $(GLTF_TRANSFORM_VERSION) already installed."; \
	else \
		echo "==> Installing @gltf-transform/cli@$(GLTF_TRANSFORM_VERSION) into $(NODE_DIR)..."; \
		mkdir -p $(NODE_DIR); \
		npm install --prefix $(NODE_DIR) --no-audit --no-fund --silent @gltf-transform/cli@$(GLTF_TRANSFORM_VERSION); \
		echo "glTF-Transform $$($(GLTF_TRANSFORM) --version) installed to $(NODE_DIR)"; \
	fi

check: build deps-godot deps-gut
	@echo "==> Running Rust checks..."
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && \
		$(CARGO) clippy -- -D warnings && \
		$(CARGO) test --lib
	@$(MAKE) test-assets
	@$(MAKE) test-godot
	@$(MAKE) check-visual
	@echo "All checks passed."

# The visual-contract suite (rust/void-logic/tests/visual.rs): boots the
# real game at grammar-derived poses, captures frames, asserts on the
# images (e.g. planet 2 containment). Part of `make check`; isolatable:
#   make check-visual                  # every contract
#   LEVEL=8 SEED=2 make check-visual   # one level/seed, frames kept in out/visual/
check-visual: build-release deps-godot
	@echo "==> Running visual contracts (log: out/visual/last-run.log)..."
	@mkdir -p $(CURDIR)/out/visual
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && GODOT="$(GODOT)" $(CARGO) test -p void_logic --test visual $(FILTER) -- --nocapture \
			> $(CURDIR)/out/visual/last-run.log 2>&1; \
		code=$$?; cat $(CURDIR)/out/visual/last-run.log; exit $$code

# The zone-authoring loop's stage: re-read every installed scene and model
# into out/metrics/ (TOML + section images) against the CURRENT rosters —
# no Blender, nothing converted, seconds. Edit rosters/environments/
# <key>.toml, run this, read out/metrics/<key>_plan.png and the
# <key>_long_NN / _cross_NN cuts (owner 2026-09-06: a metrics run outside
# the stage is not reproducible; this is the stage).
metrics:
	@echo "==> Measuring installed scenes and models against the rosters..."
	@GLTF_TRANSFORM="$(GLTF_TRANSFORM)" GLTF_TRANSFORM_VERSION="$(GLTF_TRANSFORM_VERSION)" ./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR) --metrics-only
	@echo "==> Condensing the stages' last logs into error/warning histograms..."
	@logs=$$(ls out/assets-install.log out/convert-*.log out/split-*.log out/decimate-*.log out/cockpit-*.log out/hull-*.log out/manifest-*.log out/max-*.log 2>/dev/null); \
		[ -n "$$logs" ] && python3 scripts/log-histogram.py assets-install out/metrics/log_assets-install.toml $$logs \
		|| echo "  (no assets-install logs yet — run make assets-install)"
	@logs=$$(ls out/assets-import-pass*.log 2>/dev/null); \
		[ -n "$$logs" ] && python3 scripts/log-histogram.py assets-import out/metrics/log_assets-import.toml $$logs \
		|| echo "  (no assets-import logs yet — run make assets-import)"
	@$(MAKE) test-assets

# The picture beside a model's metrics: one rendered preview per
# installed hull and enemy under out/metrics/ (<model>_preview.png),
# framed from the model's own bounds by scripts/preview_plan.py and
# rendered headless by Blender (scripts/render-preview.py). A reading
# aid for identification and review — a seller's listing matched by eye
# (owner 2026-09-09), a hull checked before it reaches the roster —
# never a gate. MODELS=<glb ...> narrows the run.
previews: MODELS ?= $(wildcard $(GODOT_DIR)/addons/ships/*.glb $(GODOT_DIR)/addons/enemies/*.glb)
previews: deps-blender
	@mkdir -p out/metrics
	@for glb in $(MODELS); do \
		stem=$$(basename "$$glb" .glb); \
		if $(BLENDER) --background --python-exit-code 1 --python scripts/render-preview.py -- \
				"$$glb" "$(CURDIR)/out/metrics/$${stem}_preview.png" > "out/preview-$$stem.log" 2>&1; then \
			grep -i "render-preview:" "out/preview-$$stem.log"; \
		else \
			echo "  ERROR: $$stem preview failed (see out/preview-$$stem.log)"; \
		fi; \
	done

# Re-copies sanitized .tres materials from asset packs and re-applies
# local material patches (no reimport).
assets-materials:
	@echo "==> Restoring material definitions from asset packs..."
	@./scripts/install-addons.sh $(ASSETS_DIR) $(GODOT_DIR) --tres-only
	@echo "==> Enabling anisotropic texture filtering on VisualShader materials..."
	@python3 -c "import re,sys;p=sys.argv[1];t=open(p).read();t=re.sub(r'(\[sub_resource type=\"VisualShaderNodeTexture2DParameter\"[^\]]*\]\nparameter_name = [^\n]+)',r'\1\ntexture_filter = 6',t);open(p,'w').write(t)" \
		$(GODOT_DIR)/addons/quaternius/materials/M_Trim_Base.tres

# Asset-pipeline conservation audit: pytest over the generated extracts
# (fbx_materials.json / max_materials.json), the material plan, and the
# built .glb. Answers "why does this surface have no texture/reflection"
# BEFORE a playtest does. Extracts are produced by `make assets`.
# Syntax gate for every pipeline script, the Blender-hosted ones included
# (they import bpy and cannot be run outside Blender, but they compile):
# a typo in convert-environment.py fails here in a second, not at the
# top of a half-hour conversion. The audit and `make check` run it.
lint:
	@test -x "$(PYENV)/bin/python3" || { echo "ERROR: python venv missing — run 'make deps'"; exit 1; }
	@echo "==> Compiling pipeline scripts..."
	@$(PYENV)/bin/python3 -m py_compile scripts/*.py scripts/tests/*.py && echo "Scripts compile."

test-assets: lint
	@test -x "$(PYENV)/bin/python3" || { echo "ERROR: python venv missing — run 'make deps'"; exit 1; }
	@echo "==> Running asset-pipeline audit (pytest)..."
	@# TESTS=<path or node id> narrows the run (the stage takes a selector;
	@# python is never invoked directly — a settings hook refuses it).
	@$(PYENV)/bin/python3 -m pytest $(or $(TESTS),scripts/tests) -q

# Filtered Rust tests with output: make test-rust FILTER=test_name
# Library tests only — the visual suite (tests/visual.rs) boots Godot and
# has its own stage, `make check-visual`.
test-rust:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) test --lib $(FILTER) -- --nocapture $(TESTFLAGS)

# Regenerate rosters/VOCABULARY.md from the closed-vocabulary enums.
# `make build` runs this; the standalone target is the fast manual path.
roster-vocab:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) test -p void_logic regenerate_vocabulary_reference -- --ignored

# Regenerate rosters/TEMPLATE.toml — the complete authoring scaffold (every
# field of every entry kind, defaults spelled out; itself a loadable
# grammar, test-enforced). `make build` runs this too.
roster-template:
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) test -p void_logic regenerate_template -- --ignored

# Runs GUT against the currently installed dylib (no rebuild). Optional filters
# for the fast inner loop (skip the full suite): F selects scripts by filename
# substring, T narrows to a single test by name. With neither set, runs all:
#   make test-godot
#   make test-godot F=test_ship_select_backdrop
#   make test-godot F=test_ship_select_backdrop T=test_backdrop_is_structure_only
# GUT runs under an isolated HOME so its user:// (savegame.cfg, options.cfg)
# never touches the developer's real profile — tests exercise real
# persistence, and real persistence must not wipe real progress.
# build first: GUT drives the COMPILED extension — a stale dylib tests
# yesterday's code (84 phantom failures, 2026-07-18).
test-godot: build deps-godot deps-gut
	@echo "==> Running Godot tests (GUT)$(if $(F), [F=$(F) T=$(T)])..."
	@mkdir -p $(GODOT_DIR)/.godot/test_home
	@GODOT_DISABLE_LEAK_CHECKS=1 HOME=$(abspath $(GODOT_DIR)/.godot/test_home) \
		$(GODOT) --headless --path $(GODOT_DIR) \
		-s res://addons/gut/gut_cmdln.gd \
		-gdir=res://tests -ginclude_subdirs \
		$(if $(F),-gselect=$(F)) $(if $(T),-gunit_test_name=$(T)) -gexit

build: require-rust
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) build
	@rm -f $(GODOT_DIR)/libvoid_scavenger.debug.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@cp $(RUST_DIR)/target/debug/libvoid_scavenger.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@# Re-render the generated roster artifacts (TEMPLATE.toml, VOCABULARY.md)
	@# so they can never drift from the code — no golden pins to trip.
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) test -p void_logic --quiet regenerate_ -- --ignored >/dev/null
	@echo "Build complete (debug)."

build-release: require-rust
	@export PATH="$$HOME/.cargo/bin:$$PATH" && \
		cd $(RUST_DIR) && $(CARGO) build --release
	@rm -f $(GODOT_DIR)/libvoid_scavenger.debug.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@cp $(RUST_DIR)/target/release/libvoid_scavenger.dylib $(GODOT_DIR)/libvoid_scavenger.dylib
	@echo "Build complete (release)."

# Dev knobs: `make run LEVEL=7` starts new games at level 7 (rendering /
# roster / boss-staging inspection); `SEED=1` pins the run seed. F9/F10
# hop levels in flight.
run: build-release deps-godot
	@echo "==> Launching game (release)..."
	@$(GODOT) --path $(GODOT_DIR) $(if $(LEVEL)$(SEED),-- $(if $(LEVEL),--level=$(LEVEL)) $(if $(SEED),--seed=$(SEED)))

demo: build deps-godot
	@echo "==> Launching game (debug)..."
	@$(GODOT) --path $(GODOT_DIR) $(if $(LEVEL)$(SEED),-- $(if $(LEVEL),--level=$(LEVEL)) $(if $(SEED),--seed=$(SEED)))

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
