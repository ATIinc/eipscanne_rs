#!/bin/bash

set -euo pipefail

# Create the bind mount source up front, to avoid Docker creating it root-owned
mkdir -p "${HOME}/.claude"

# Converge Claude config onto ONE real file at ~/.claude/.claude.json (inside the directory bind shared with
# containers, which use CLAUDE_CONFIG_DIR). Host-side claude keeps working via a symlink at the old ~/.claude.json path:
# Claude Code reads and writes through symlinks without replacing them. A real file at ~/.claude.json wins; if that
# clobbers an existing shared config, the loser is backed up rather than deleted.
if [ ! -L "${HOME}/.claude.json" ]; then
	if [ -f "${HOME}/.claude.json" ]; then
		if [ -f "${HOME}/.claude/.claude.json" ]; then
			bak="${HOME}/.claude/.claude.json.bak.$(date +%s)"
			mv "${HOME}/.claude/.claude.json" "${bak}"
			echo "WARNING: both ~/.claude.json and ~/.claude/.claude.json existed; kept ~/.claude.json, backed up the other to ${bak}" 1>&2
		fi
		mv "${HOME}/.claude.json" "${HOME}/.claude/.claude.json"
	fi
	touch "${HOME}/.claude/.claude.json"
	ln -s ".claude/.claude.json" "${HOME}/.claude.json"
fi

# Make gh work inside the container: on the host, gh keeps its token in the system keyring, so pass it in as GH_TOKEN
# via an env file (devcontainer.json --env-file). See https://github.com/cli/cli/discussions/7611.
# DANGER: this puts the token in plaintext in .devcontainer/.env (gitignored, mode 600) and in the container's config,
# where anyone with docker access can read it via `docker inspect`. It's also only refreshed on container rebuild.
# The file is always (re)created, since docker refuses to start if an --env-file is missing.
ENV_FILE="$(dirname "$0")/.env"
(umask 077 && : > "${ENV_FILE}")
if GH_TOKEN="$(gh auth token 2>/dev/null)" && [ -n "${GH_TOKEN}" ]; then
	echo "GH_TOKEN=${GH_TOKEN}" >> "${ENV_FILE}"
else
	echo "WARNING: couldn't get a token from 'gh auth token', gh won't be authenticated inside the container" 1>&2
fi
