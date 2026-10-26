#!/usr/bin/env bash
set -euo pipefail

for plugin_id in vesp.clipboard-history io.github.ipvesp.clipboard-history; do
  if [[ -e "$HOME/.config/omarchy/plugins/$plugin_id" || -L "$HOME/.config/omarchy/plugins/$plugin_id" ]]; then
    omarchy plugin remove "$plugin_id" --yes
  fi
done

systemctl --user disable --now clipboard-history.service 2>/dev/null || true
rm -f "$HOME/.config/systemd/user/clipboard-history.service"
systemctl --user daemon-reload

bindings="$HOME/.config/hypr/bindings.lua"
if [[ -f "$bindings" ]]; then
  cp "$bindings" "$bindings.bak.clipboard-history.$(date +%Y%m%d%H%M%S)"
  python3 - "$bindings" <<'PY'
from pathlib import Path
import sys
p = Path(sys.argv[1])
lines = p.read_text().splitlines()
start = '-- clipboard-history begin'
end = '-- clipboard-history end'
if start in lines and end in lines:
    first, last = lines.index(start), lines.index(end)
    lines = lines[:first] + lines[last + 1:]
p.write_text("\n".join(lines) + "\n")
PY
  hyprctl reload
  errors="$(hyprctl configerrors)"
  if [[ -n "$errors" ]]; then echo "$errors" >&2; exit 1; fi
fi

rm -f "$HOME/.local/bin/clipboard-history"
if [[ -f "$HOME/.local/bin/vesp" ]] &&
   grep -Fq 'clipboard-history" ch "$@"' "$HOME/.local/bin/vesp"; then
  rm -f "$HOME/.local/bin/vesp"
fi
echo "Clipboard History removed. Settings and history remain in your home directory."
