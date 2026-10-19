FROM mcr.microsoft.com/devcontainers/rust:2-trixie

# ping for checking device reachability; gh for GitHub operations (and git auth, see devcontainer.json);
# poppler-utils for reading PDF references (scripts/pdf-to-text.sh, and PDF page rendering in Claude Code);
# tshark for dissecting packets with Wireshark (scripts/dissect.sh)
RUN apt-get update \
	&& apt-get install -y --no-install-recommends iputils-ping gh poppler-utils tshark \
	&& rm -rf /var/lib/apt/lists/*

# Keep Claude Code's config file inside the bind-mounted ~/.claude (as ~/.claude/.claude.json) instead of the
# container-local ~/.claude.json, which is lost on rebuild. initializeHost.sh points the host at the same file.
ENV CLAUDE_CONFIG_DIR=/home/vscode/.claude

USER vscode

# Claude Code CLI for terminal use (the VS Code extension bundles its own)
RUN curl -fsSL https://claude.ai/install.sh | bash

# cargo tab completion. The stock script only globs `examples/*.rs` next to the root manifest, so it misses the
# scanner's examples (a workspace member, with directory examples); ask cargo for the list instead
RUN cat >> ~/.bashrc <<'BASHRC'

# cargo tab completion, with example names taken from cargo (covers workspace members and directory examples)
source "$(rustc --print sysroot)/etc/bash_completion.d/cargo"
_get_examples() { cargo run --example 2>&1 | sed -n 's/^    //p'; }
BASHRC
