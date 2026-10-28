# Packaging

Radiadash ships as an RPM (primary) with an AppImage planned.

## Build

```bash
pnpm install
pnpm tauri build --bundles rpm
```

Requirements: the GTK/WebKit development packages from the README, plus
`rpm-build` (Tauri invokes `rpmbuild`). The release build compiles the Rust
workspace and embeds the built frontend.

Output:

```
apps/desktop/src-tauri/target/release/bundle/rpm/Radiadash-0.1.0-1.x86_64.rpm
```

## Contents

| Path | Purpose |
| --- | --- |
| `/usr/bin/radiadash` | application binary |
| `/usr/share/applications/Radiadash.desktop` | desktop entry (category Education) |
| `/usr/share/icons/hicolor/*/apps/radiadash.png` | icons |
| `/usr/lib/udev/rules.d/60-radiacode.rules` | USB access without root |

Runtime dependencies are only `libgtk-3.so.0` and `libwebkit2gtk-4.1.so.0`; SQLite
is bundled into the binary.

## Install

```bash
sudo dnf install ./Radiadash-0.1.0-1.x86_64.rpm
sudo udevadm control --reload-rules && sudo udevadm trigger
```

The udev reload is needed once after installation (or unplug/replug the detector).
Data lives in `~/.local/share/io.github.martintondel.radiadash/`.

## Notes

- The RPM is unsigned; sign it in CI for distribution.
- An AppImage target is configured in `tauri.conf.json` but has not been built
  yet — it needs `linuxdeploy` download access. Flatpak is deferred because USB
  vendor devices require broad device access inside the sandbox.
- A release workflow (tag → RPM artifact) is not yet in place; builds are currently
  local.
