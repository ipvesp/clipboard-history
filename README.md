# Clipboard History

A clipboard history popup for Omarchy. Press **Super+V** to open it beside the pointer. The Omarchy bar icon opens a compact settings panel.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/ipvesp/clipboard-history/v1.0.1/install.sh | bash
```

The installer checks for `gtk4`, `wl-clipboard`, `pkgconf` and Rust, installs missing packages with `sudo pacman`, builds the application, creates a user service to watch copied text, adds the Super+V binding, and enables the Omarchy bar plugin. Review `install.sh` before running it. It does not require root for app or config files.

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

The settings file is `$XDG_CONFIG_HOME/clipboard-history/settings.json` (or `~/.config/clipboard-history/settings.json`). History is stored in `$XDG_DATA_HOME/clipboard-history/history.db` (or `~/.local/share/clipboard-history/history.db`). Clipboard text stays on the local computer.

## Remove

```sh
curl -fsSL https://raw.githubusercontent.com/ipvesp/clipboard-history/main/uninstall.sh | bash
```

The uninstaller removes the plugin, watcher, keybinding, and executables. It keeps the database and settings so you can preserve your history.

## Development

```sh
omarchy plugin validate .
cargo test --locked
cargo build --release --locked
```

`manifest.json`, `BarWidget.qml`, and `Panel.qml` are the Omarchy plugin. The Rust application provides clipboard storage, the popup, and the `vesp ch` commands.

The Cargo and plugin versions follow SemVer together. Each published version receives a `vX.Y.Z` GitHub Release.
