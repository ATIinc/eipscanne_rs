FROM mcr.microsoft.com/devcontainers/rust:2-trixie

# ping for checking device reachability; gh for GitHub operations (and git auth, see devcontainer.json)
RUN apt-get update \
	&& apt-get install -y --no-install-recommends iputils-ping gh \
	&& rm -rf /var/lib/apt/lists/*

# Keep Claude Code's config file inside the bind-mounted ~/.claude (as ~/.claude/.claude.json) instead of the
# container-local ~/.claude.json, which is lost on rebuild. initializeHost.sh points the host at the same file.
ENV CLAUDE_CONFIG_DIR=/home/vscode/.claude

USER vscode

# Claude Code CLI for terminal use (the VS Code extension bundles its own)
RUN curl -fsSL https://claude.ai/install.sh | bash
