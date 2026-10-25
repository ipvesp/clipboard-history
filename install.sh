#!/usr/bin/env bash
set -euo pipefail

repo_url="https://github.com/ipvesp/clipboard-history.git"
plugin_id="io.github.ipvesp.clipboard-history"
root=""
if [[ -n "${BASH_SOURCE[0]:-}" && -f "${BASH_SOURCE[0]}" ]]; then
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi
temporary=""

if [[ -z "$root" || ! -f "$root/Cargo.toml" || ! -f "$root/manifest.json" ]]; then
  command -v git >/dev/null || { echo "Missing command: git" >&2; exit 1; }
  temporary="$(mktemp -d)"
  trap 'rm -rf "$temporary"' EXIT
  git clone --depth 1 "$repo_url" "$temporary/repo"
  root="$temporary/repo"
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
install -Dm755 "$root/target/release/clipboard-history" "$HOME/.local/bin/clipboard-history"

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
ExecStart=/usr/bin/wl-paste --type text --watch %h/.local/bin/clipboard-history store
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

if ! omarchy plugin list --json | jq -e --arg id "$plugin_id" 'any(.[]; .id == $id)' >/dev/null; then
  omarchy plugin add "$repo_url" --enable --yes
else
  omarchy plugin update "$plugin_id" --yes
  omarchy plugin enable "$plugin_id" || true
fi

echo "Installed Clipboard History $(jq -r '.version' "$root/manifest.json"). Press Super+V or run: vesp ch --help"
