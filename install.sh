#!/bin/bash
# Build and install Cycle (user space, no sudo).
set -e; cd "$(dirname "$0")"
cargo build --release
install -Dm755 target/release/cycle ~/.local/bin/cycle.new && mv -f ~/.local/bin/cycle.new ~/.local/bin/cycle
mkdir -p ~/.config/systemd/user
cp packaging/cycle-*.service packaging/cycle-*.timer ~/.config/systemd/user/
install -Dm644 packaging/cycle.desktop ~/.local/share/applications/cycle.desktop
systemctl --user daemon-reload
cycle engine-is-on && systemctl --user enable --now cycle-engine.timer || true
echo "installed ~/.local/bin/cycle"
