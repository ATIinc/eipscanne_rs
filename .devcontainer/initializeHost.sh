#!/bin/bash

set -euo pipefail

# Create directories for bind mounts up front, to avoid Docker creating them root-owned
mkdir -p "${HOME}/.anthropic"
mkdir -p "${HOME}/.claude"
mkdir -p "${HOME}/.config/claude-code"
mkdir -p "${HOME}/.config/gh"

# Converge Claude config onto ONE real file at ~/.claude/.claude.json (inside the directory bind shared with
# containers, which use CLAUDE_CONFIG_DIR — see devcontainer.json). Host-side claude keeps working via a symlink at
# the old ~/.claude.json path: Claude Code reads and writes through symlinks without replacing them. A real file at
# ~/.claude.json (pre-migration, or recreated by an installer since) wins; if that clobbers an existing shared config,
# the loser is backed up rather than deleted. Same logic as firefly's initializeHost.sh.
if [ ! -L "${HOME}/.claude.json" ]; then
	if [ -f "${HOME}/.claude.json" ]; then
		if [ -f "${HOME}/.claude/.claude.json" ]; then
			bak="${HOME}/.claude/.claude.json.bak.$(date +%s)"
			mv "${HOME}/.claude/.claude.json" "${bak}"
			printf '\033[31mWARNING: both ~/.claude.json and ~/.claude/.claude.json existed; keeping ~/.claude.json, backed the other up to %s\033[0m\n' "${bak}"
		fi
		mv "${HOME}/.claude.json" "${HOME}/.claude/.claude.json"
	fi
	touch "${HOME}/.claude/.claude.json"
	ln -s ".claude/.claude.json" "${HOME}/.claude.json"
fi
