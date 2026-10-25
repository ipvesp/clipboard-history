#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin" "$tmp/data/clipboard-history"

cat > "$tmp/bin/wl-paste" <<'EOF'
#!/usr/bin/env bash
if [[ "$1" == "--list-types" ]]; then
  if [[ "$CLIP_TEST_MODE" == "image" ]]; then echo image/png; else echo text/plain; fi
elif [[ "$CLIP_TEST_MODE" == "image" ]]; then
  cat "$CLIP_TEST_IMAGE"
else
  printf '%s' "$CLIP_TEST_TEXT"
fi
EOF
chmod +x "$tmp/bin/wl-paste"

python3 - "$tmp/data/clipboard-history/history.db" <<'PY'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.execute('CREATE TABLE clipboard_items (id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (unixepoch()))')
db.execute('INSERT INTO clipboard_items(content) VALUES (?)', ('old entry',))
db.commit()
PY

python3 - "$tmp/pixel.png" <<'PY'
import base64, pathlib, sys
pathlib.Path(sys.argv[1]).write_bytes(base64.b64decode(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9aS0sAAAAASUVORK5CYII='
))
PY

export PATH="$tmp/bin:$PATH" XDG_DATA_HOME="$tmp/data" XDG_CONFIG_HOME="$tmp/config"
export CLIP_TEST_IMAGE="$tmp/pixel.png"
binary="$root/target/debug/clipboard-history"

CLIP_TEST_MODE=text CLIP_TEST_TEXT='https://example.org/path' "$binary" capture </dev/null
CLIP_TEST_MODE=text CLIP_TEST_TEXT=$'fn main() {\n    println!("hi");\n}' "$binary" capture </dev/null
CLIP_TEST_MODE=image "$binary" capture </dev/null
CLIP_TEST_MODE=image "$binary" capture </dev/null

python3 - "$tmp/data/clipboard-history/history.db" "$tmp/pixel.png" <<'PY'
import pathlib, sqlite3, sys
db = sqlite3.connect(sys.argv[1])
items = db.execute('SELECT kind, content, data FROM clipboard_items ORDER BY id').fetchall()
assert [item[0] for item in items] == ['text', 'link', 'code', 'image'], items
assert items[-1][1] == 'image/png'
assert items[-1][2] == pathlib.Path(sys.argv[2]).read_bytes()
print('Migration, link, code, image storage, and image deduplication passed.')
PY
