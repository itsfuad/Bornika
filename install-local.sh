#!/bin/bash
set -e

echo "============================================="
echo "  Installing Bornika: Bangla Phonetic IME    "
echo "============================================="

# 1. Compile the workspace in release mode
echo "--> Compiling Bornika in release mode..."
cargo build --release

# 2. Deploy binary to ~/.local/bin
echo "--> Creating local bin directory..."
mkdir -p "$HOME/.local/bin"

echo "--> Copying binary..."
killall bornika-daemon >/dev/null 2>&1 || pkill -f bornika-daemon >/dev/null 2>&1 || true
cp target/release/bornika-daemon "$HOME/.local/bin/bornika-daemon"
chmod +x "$HOME/.local/bin/bornika-daemon"

# 3. Generate and deploy IBus component XML
echo "--> Generating bornika.xml with home directory: $HOME"
sed "s|__HOME__|${HOME}|g" component/bornika.xml.template > /tmp/bornika.xml

echo "--> Installing component XML to /usr/share/ibus/component/bornika.xml (requires sudo)..."
sudo cp /tmp/bornika.xml /usr/share/ibus/component/bornika.xml
sudo chmod 644 /usr/share/ibus/component/bornika.xml
rm /tmp/bornika.xml

# 4. Restart IBus daemon to apply changes
echo "--> Restarting IBus daemon..."
if command -v ibus >/dev/null 2>&1; then
    ibus restart
    echo "--> IBus restarted successfully!"
else
    echo "WARNING: 'ibus' command not found in your PATH."
    echo "If you are running Fedora/Ubuntu, IBus is typically active."
    echo "Please ensure the IBus daemon is restarted to load the engine."
fi

echo "============================================="
echo "  Bornika Installation Complete!             "
echo "============================================="
echo ""
echo "To activate Bornika on your system:"
echo "1. Open GNOME Settings (or your desktop settings)."
echo "2. Navigate to 'Keyboard' -> 'Input Sources'."
echo "3. Click '+' (Add Input Source)."
echo "4. Select 'Bengali' -> 'Bengali (Bornika)'."
echo "5. To toggle between English and Bangla typing modes:"
echo "   Use the shortcut 'Ctrl + Space' when Bornika is active."
echo "============================================="
