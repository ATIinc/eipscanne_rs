#!/bin/bash

set -euo pipefail

sudo apt-get update
sudo apt-get install -y iputils-ping

# Claude Code CLI (the VS Code extension bundles its own, this is for terminal use)
curl -fsSL https://claude.ai/install.sh | bash

# clear any `gh auth` helpers in the gitconfig inherited from outside the devcontainer, these point at host paths that
# don't exist inside, and we rely on vscode forwarding credentials instead.
git config --global --add credential.https://github.com.helper ''
git config --global --add credential.https://gist.github.com.helper ''
