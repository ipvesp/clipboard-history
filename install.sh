#!/usr/bin/env bash
set -euo pipefail

repo_url="https://github.com/ipvesp/clipboard-history.git"
plugin_id="io.github.ipvesp.clipboard-history"
release_ref="v1.1.1"
root=""
if [[ -n "${BASH_SOURCE[0]:-}" && -f "${BASH_SOURCE[0]}" ]]; then
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi
temporary=""
stage=""
cleanup() {
  if [[ -n "$stage" && -d "$stage" ]]; then rm -rf "$stage"; fi
  if [[ -n "$temporary" && -d "$temporary" ]]; then rm -rf "$temporary"; fi
}
trap cleanup EXIT

if [[ -z "$root" || ! -f "$root/Cargo.toml" || ! -f "$root/manifest.json" ]]; then
  command -v git >/dev/null || { echo "Missing command: git" >&2; exit 1; }
  temporary="$(mktemp -d)"
  git clone --depth 1 --branch "$release_ref" "$repo_url" "$temporary/repo"
  root="$temporary/repo"
fi

# A pipe cannot tell the installer which commit supplied its own bytes. The
# published command must pass the reviewed SHA, and both checkouts must match.
if [[ -z "${CLIPBOARD_HISTORY_COMMIT:-}" ]]; then
  echo "Set CLIPBOARD_HISTORY_COMMIT to the reviewed 40-character commit SHA." >&2
  exit 1
fi
if [[ ! "$CLIPBOARD_HISTORY_COMMIT" =~ ^[0-9a-f]{40}$ ]]; then
  echo "CLIPBOARD_HISTORY_COMMIT must be a full lowercase Git SHA." >&2
  exit 1
fi
actual_commit="$(git -C "$root" rev-parse HEAD)"
if [[ "$actual_commit" != "$CLIPBOARD_HISTORY_COMMIT" ]]; then
  echo "Source commit mismatch: expected $CLIPBOARD_HISTORY_COMMIT, got $actual_commit" >&2
  exit 1
fi
if [[ -n "$(git -C "$root" status --porcelain)" ]]; then
  echo "Source checkout has local changes; refusing to install unreviewed files." >&2
  exit 1
fi

if [[ -e "$HOME/.local/bin/vesp" ]] &&
   ! grep -Fq 'clipboard-history" ch "$@"' "$HOME/.local/bin/vesp"; then
  echo "Existing ~/.local/bin/vesp belongs to another program; installation stopped." >&2
  exit 1
fi

for command in git omarchy hyprctl systemctl; do
  command -v "$command" >/dev/null || { echo "Missing command: $command" >&2; exit 1; }
done

packages=()
for package in gtk4 wl-clipboard pkgconf; do
  pacman -Q "$package" >/dev/null 2>&1 || packages+=("$package")
done
if ! command -v cargo >/dev/null; then packages+=(rust); fi
if ((${#packages[@]})); then
  sudo pacman -S --needed "${packages[@]}"
fi

omarchy plugin validate "$root"
(cd "$root" && cargo build --release --locked)

# Omarchy's add/update commands fetch the moving default branch. Stage a Git
# checkout of the verified commit ourselves before changing the live plugin.
plugins_dir="$HOME/.config/omarchy/plugins"
mkdir -p "$plugins_dir"
stage="$(mktemp -d "$plugins_dir/.clipboard-history-stage.XXXXXX")"
rm -rf "$stage"
git clone --quiet --no-local --no-checkout "$root" "$stage"
git -C "$stage" checkout --quiet --detach "$CLIPBOARD_HISTORY_COMMIT"
git -C "$stage" remote set-url origin "$repo_url"
if [[ "$(git -C "$stage" rev-parse HEAD)" != "$CLIPBOARD_HISTORY_COMMIT" ]]; then
  echo "Staged plugin commit mismatch." >&2
  exit 1
fi
omarchy plugin validate "$stage"

install -Dm755 "$root/target/release/clipboard-history" "$HOME/.local/bin/clipboard-history"
# GTK keeps one application process alive; stop the old instance after replacing its binary.
pkill -u "$(id -u)" -f "^$HOME/.local/bin/clipboard-history$" 2>/dev/null || true

cat > "$HOME/.local/bin/vesp" <<'EOF'
#!/usr/bin/env bash
# clipboard-history launcher
if [[ "${1:-}" == "ch" ]]; then shift; fi
exec "$HOME/.local/bin/clipboard-history" ch "$@"
EOF
chmod 755 "$HOME/.local/bin/vesp"

mkdir -p "$HOME/.config/systemd/user"
cat > "$HOME/.config/systemd/user/clipboard-history.service" <<'EOF'
[Unit]
Description=Clipboard History watcher

[Service]
Type=simple
ExecStart=/usr/bin/wl-paste --watch %h/.local/bin/clipboard-history capture
Restart=always
RestartSec=2

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now clipboard-history.service

bindings="$HOME/.config/hypr/bindings.lua"
mkdir -p "$(dirname "$bindings")"
touch "$bindings"
cp "$bindings" "$bindings.bak.clipboard-history.$(date +%Y%m%d%H%M%S)"
python3 - "$bindings" <<'PY'
from pathlib import Path
import sys
p = Path(sys.argv[1])
lines = p.read_text().splitlines()
lines = [line for line in lines if line.strip() not in {
    'hl.unbind("SUPER + V")',
    'o.bind("SUPER + V", "Clipboard History", "~/.local/bin/clipper")',
    'o.bind("SUPER + V", "Clipboard History", "~/.local/bin/clipboard-history")',
}]
start = '-- clipboard-history begin'
end = '-- clipboard-history end'
if start in lines and end in lines:
    first, last = lines.index(start), lines.index(end)
    lines = lines[:first] + lines[last + 1:]
lines.extend([
    start,
    'hl.unbind("SUPER + V")',
    'o.bind("SUPER + V", "Clipboard History", "~/.local/bin/clipboard-history")',
    end,
])
p.write_text("\n".join(lines) + "\n")
PY
hyprctl reload
errors="$(hyprctl configerrors)"
if [[ -n "$errors" ]]; then echo "$errors" >&2; exit 1; fi

target="$plugins_dir/$plugin_id"
backup=""
if [[ -e "$target" || -L "$target" ]]; then
  backup="$(mktemp -d "$plugins_dir/.clipboard-history-backup.XXXXXX")"
  rmdir "$backup"
  mv "$target" "$backup"
fi
if ! mv "$stage" "$target"; then
  if [[ -n "$backup" ]]; then mv "$backup" "$target"; fi
  exit 1
fi
if ! omarchy-shell shell rescanPlugins >/dev/null; then
  rm -rf "$target"
  if [[ -n "$backup" ]]; then mv "$backup" "$target"; fi
  echo "Plugin rescan failed; restored previous checkout." >&2
  exit 1
fi
if ! omarchy plugin enable "$plugin_id"; then
  rm -rf "$target"
  if [[ -n "$backup" ]]; then mv "$backup" "$target"; fi
  omarchy-shell shell rescanPlugins >/dev/null || true
  echo "Plugin enable failed; restored previous checkout." >&2
  exit 1
fi
if [[ -n "$backup" ]]; then rm -rf "$backup"; fi

echo "Installed Clipboard History $(jq -r '.version' "$root/manifest.json") at $CLIPBOARD_HISTORY_COMMIT. Press Super+V or run: vesp ch --help"
