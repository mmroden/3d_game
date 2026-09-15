#!/bin/sh
# Launch door for this repo's Serena MCP server — .mcp.json points here.
#
# Serena's language-server manager initializes ONCE at MCP startup and
# dials the Godot editor LSP (TCP 6008) for GDScript. If 6008 is down at
# that moment, the --project activation fails at boot: the instance comes
# up projectless — or on the shared LAST-ACTIVE project, i.e. a DIFFERENT
# REPO — and every tool errors for the rest of the session. So the LSP is
# guaranteed first, through its own make door, and only then does the
# process hand itself over to Serena.
#
# Everything writes to stderr: stdout is the MCP stdio channel, and a
# single stray line there corrupts the protocol.
set -eu
cd "$(dirname "$0")/.."
make lsp-up >&2
exec /opt/homebrew/bin/uvx -p 3.13 \
    --from git+https://github.com/oraios/serena \
    serena start-mcp-server \
    --context agent \
    --project /Users/mroden/src/3d_game
