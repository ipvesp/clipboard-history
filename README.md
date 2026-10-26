# Clipboard History

A clipboard history popup for Omarchy. It saves text, links, code snippets, and common image formats. Press **Super+V** to open it beside the pointer. The Omarchy bar icon opens a compact settings panel.

## Install

For the [v1.1.2 release](https://github.com/ipvesp/clipboard-history/releases/tag/v1.1.2), replace the value below with the reviewed commit SHA shown in the release notes, then run:

```sh
CLIPBOARD_HISTORY_COMMIT='PASTE_REVIEWED_40_CHARACTER_SHA_HERE'
set -o pipefail
curl -fsSL "https://raw.githubusercontent.com/ipvesp/clipboard-history/$CLIPBOARD_HISTORY_COMMIT/install.sh" | CLIPBOARD_HISTORY_COMMIT="$CLIPBOARD_HISTORY_COMMIT" bash
```

The installer verifies that the application and Omarchy plugin come from the reviewed commit before installing either one. It requires Omarchy with Hyprland and systemd user services, plus `curl`, `git`, `jq`, `python3`, `pacman`, and `sudo`. It installs missing `gtk4`, `wl-clipboard`, `pkgconf`, and Rust packages with `sudo pacman -S --needed`, then builds the Rust application with `cargo build --release --locked`. Cargo downloads the crates in `Cargo.lock` from its configured registry when they are not cached.

The installer writes executables to `~/.local/bin`, a user service to `~/.config/systemd/user`, a backed-up Super+V binding in `~/.config/hypr/bindings.lua`, and the plugin checkout to `~/.config/omarchy/plugins`. It enables the clipboard watcher and bar widget. Upgrades replace the former `io.github.ipvesp.clipboard-history` plugin ID with `vesp.clipboard-history`. Only package installation uses `sudo`; the application and plugin run with your user permissions. Omarchy plugins run unsandboxed in the long-running shell process. Review [install.sh](install.sh), [uninstall.sh](uninstall.sh), and the locked dependencies before running the installer.

## Settings

Click the clipboard icon in the bar to open history or change the saved item count and bar visibility. The item count can be 5–200. If the icon is hidden, show it again from a terminal:

```sh
vesp ch bar show
```

Other commands:

```sh
vesp ch --help
vesp ch history-limit 50
vesp ch bar hide
vesp ch status
vesp ch clear
```

The settings file is `$XDG_CONFIG_HOME/clipboard-history/settings.json` (or `~/.config/clipboard-history/settings.json`). History is stored in `$XDG_DATA_HOME/clipboard-history/history.db` (or `~/.local/share/clipboard-history/history.db`). The history directory uses mode `0700`; the database and SQLite sidecar files use `0600`. Upgrading also repairs permissions on any legacy `clipper/history.db` copy before starting the watcher. Clipboard contents stay on the local computer. Images larger than 10 MB and text larger than 1 MB are skipped. Links and code are recognized from text and can be copied back without changes.

## Remove

```sh
set -o pipefail
curl -fsSL "https://raw.githubusercontent.com/ipvesp/clipboard-history/$CLIPBOARD_HISTORY_COMMIT/uninstall.sh" | bash
```

The uninstaller removes the plugin, watcher, keybinding, and executables. It keeps the database and settings so you can preserve your history. Use the same reviewed commit SHA from the installation command.

## Development

```sh
omarchy plugin validate .
QMLLINT="$(command -v qmllint || printf /usr/lib/qt6/bin/qmllint)"
"$QMLLINT" -I "$OMARCHY_PATH/shell" BarWidget.qml Panel.qml
cargo test --locked
cargo build --release --locked
```

`manifest.json`, `BarWidget.qml`, and `Panel.qml` are the Omarchy plugin. The Rust application provides clipboard storage, the popup, and the `vesp ch` commands.

The Cargo and plugin versions follow SemVer together. Each published version receives a `vX.Y.Z` GitHub Release.
