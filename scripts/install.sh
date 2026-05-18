#!/bin/bash
set -e

# Zero-Agent Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/spenceriam/Zero-Agent/main/scripts/install.sh | bash

REPO="spenceriam/Zero-Agent"
INSTALL_DIR="${ZERO_HOME:-$HOME/.zero-agent}"
BIN_DIR="$INSTALL_DIR/bin"

echo "  Installing Zero-Agent..."
echo ""

# Check for required tools
check_command() {
    if ! command -v "$1" &> /dev/null; then
        echo "Error: $1 is required but not installed."
        echo "Please install $1 and try again."
        exit 1
    fi
}

check_command curl
check_command git

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "Rust not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "  Cloning repository..."
mkdir -p "$INSTALL_DIR"
if [ -d "$INSTALL_DIR/zero-agent" ]; then
    cd "$INSTALL_DIR/zero-agent"
    git pull --quiet
else
    git clone --quiet "https://github.com/$REPO.git" "$INSTALL_DIR/zero-agent"
    cd "$INSTALL_DIR/zero-agent"
fi

echo "  Building..."
cd bridge/rust
cargo build --release --quiet 2>&1

echo "  Installing binary..."
mkdir -p "$BIN_DIR"
cp target/release/zero-agent-bridge "$BIN_DIR/zero"

# Add to PATH if not already there
SHELL_RC="$HOME/.bashrc"
if [ -n "$ZSH_VERSION" ]; then
    SHELL_RC="$HOME/.zshrc"
fi

if ! echo "$PATH" | grep -q "$BIN_DIR"; then
    echo "" >> "$SHELL_RC"
    echo "# Zero-Agent" >> "$SHELL_RC"
    echo "export PATH=\"\$PATH:$BIN_DIR\"" >> "$SHELL_RC"
    echo ""
    echo "  Added $BIN_DIR to PATH in $SHELL_RC"
    echo "  Run 'source $SHELL_RC' or restart your terminal."
fi

# Create default config
CONFIG_DIR="$INSTALL_DIR/config"
mkdir -p "$CONFIG_DIR"
if [ ! -f "$CONFIG_DIR/config.json" ]; then
    cat > "$CONFIG_DIR/config.json" << 'EJSON'
{
  "data_dir": "~/.zero-agent/config",
  "default_provider": "openrouter",
  "providers": [
    {
      "id": "openrouter",
      "name": "OpenRouter",
      "api_format": "openai",
      "base_url": "https://openrouter.ai/api/v1",
      "api_key": "",
      "default_model": "anthropic/claude-sonnet-4",
      "models": []
    },
    {
      "id": "openai",
      "name": "OpenAI",
      "api_format": "openai",
      "base_url": "https://api.openai.com/v1",
      "api_key": "",
      "default_model": "gpt-4o",
      "models": []
    },
    {
      "id": "anthropic",
      "name": "Anthropic",
      "api_format": "anthropic",
      "base_url": "https://api.anthropic.com",
      "api_key": "",
      "default_model": "claude-sonnet-4-20250514",
      "models": []
    },
    {
      "id": "ollama",
      "name": "Ollama (Local)",
      "api_format": "openai",
      "base_url": "http://localhost:11434/v1",
      "api_key": "",
      "default_model": "llama3",
      "models": []
    }
  ],
  "tool_policy": {
    "allow_safe_without_prompt": true,
    "ask_before_mutating": true,
    "ask_before_destructive": true
  },
  "telegram": {
    "bot_token": "",
    "allowed_users": ""
  }
}
EJSON
    echo "  Created default config at $CONFIG_DIR/config.json"
fi

# Create .env template
if [ ! -f "$CONFIG_DIR/.env" ]; then
    cat > "$CONFIG_DIR/.env" << 'EENV'
# Zero-Agent API Keys
# Uncomment and add your keys:

# OPENROUTER_API_KEY=your_key_here
# OPENAI_API_KEY=your_key_here
# ANTHROPIC_API_KEY=your_key_here
# TELEGRAM_BOT_TOKEN=your_bot_token_here
# TELEGRAM_ALLOWED_USERS=your_user_id_here
EENV
    echo "  Created .env template at $CONFIG_DIR/.env"
fi

# Create SOUL.md template
if [ ! -f "$CONFIG_DIR/SOUL.md" ]; then
    cat > "$CONFIG_DIR/SOUL.md" << 'EMD'
# ZERO

You are ZERO, a personal AI assistant for developers.
You are running locally on the user's machine.
You have access to tools for reading/writing files, running shell commands, and searching files.
Be concise and direct. When you need to do something, use your tools.
EMD
    echo "  Created SOUL.md at $CONFIG_DIR/SOUL.md"
fi

echo ""
echo "  Zero-Agent installed successfully!"
echo ""
echo "  Quick start:"
echo "    1. Add your API key to $CONFIG_DIR/.env"
echo "    2. Run: $BIN_DIR/zero"
echo ""
echo "  Or use the interactive REPL:"
echo "    $ $BIN_DIR/zero"
echo ""
