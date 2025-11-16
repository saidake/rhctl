#!/bin/sh

# bash ./scripts/init/setup-hooks.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR=$SCRIPT_DIR/../..

HOOK_DIR="$ROOT_DIR/.git/hooks"
HOOK_PATH="$HOOK_DIR/pre-commit"
mkdir -p "$HOOK_DIR"

if [ -f "$HOOK_PATH" ]; then
    echo "⚠️ pre-commit hook already exists at $HOOK_PATH."
    read -p "Do you want to overwrite it? (y/N): " answer
    case "$answer" in
        [yY][eE][sS]|[yY])
            echo "✅ Overwriting existing pre-commit hook..."
            ;;
        *)
            echo "❌ Installation aborted."
            exit 1
            ;;
    esac
fi

cp "$SCRIPT_DIR/assets/pre-commit" "$HOOK_PATH"
chmod +x "$HOOK_PATH"

echo "✅ pre-commit hook installed."
