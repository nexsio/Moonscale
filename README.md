# Moonscale

A Plasma 6 widget and standalone desktop app for Tailscale connection controls, device status, and Moonlight streaming.

## Installation

Install the Plasma QML compatibility module first:

```sh
sudo dnf install plasma5support
```

Then run this from the project directory:

```sh
kpackagetool6 --type Plasma/Applet --install .
```

If an earlier broken installation causes `--upgrade` to fail with a package-structure error, remove only this widget's installed directory and install it again:

```sh
rm -r ~/.local/share/plasma/plasmoids/org.local.tailscale-widget
kpackagetool6 --type Plasma/Applet --install .
```

Find the widget in **Add Widgets** and place it on the desktop. To update an existing installation after making changes, run:

```sh
kpackagetool6 --type Plasma/Applet --upgrade .
```

The widget requires `tailscale` in `PATH`, `pkexec` (Polkit), and the Plasma QML module `org.kde.plasma.plasma5support`. It reads status with `tailscale status --json`. When toggling the connection, it runs `pkexec tailscale up` or `pkexec tailscale down`; the native KDE Polkit dialog handles authentication, and the widget never sees your password.

To avoid authentication prompts, you can grant your user Tailscale operator access once:

```sh
sudo tailscale set --operator="$USER"
```

After that, your user can run `tailscale up` and `tailscale down` directly.

## Desktop app

The standalone desktop app lives in `app/` and uses Tauri 2, React, and TypeScript. On Fedora, install Rust, Node.js/npm, the Tauri Linux prerequisites (GTK 3, WebKitGTK 4.1, and OpenSSL development packages), `ksshaskpass`, plus the Tailscale and Moonlight clients.

Run it from the `app` directory:

```sh
npm install
npm run tauri dev
```

The app reads Tailscale devices with `tailscale status --json`. Connection changes use `pkexec tailscale up/down`, so the system Polkit dialog handles authorization. The streaming profile is stored in the app's local configuration directory. Configure the target PC, Moonlight executable, SSH access to the home server, and its Wake-on-LAN command in **Settings**. For encrypted SSH keys, Moonscale uses the system Askpass helper to show a graphical passphrase prompt for each wake; it does not use `ssh-agent` or store the passphrase. SSH public-key authentication is required, the host key must already be trusted in `~/.ssh/known_hosts`, and Moonscale will not accept a new host key automatically. Starting a stream wakes the target when needed, waits for it to appear online in Tailscale, and then launches Moonlight. Pair the target with Moonlight once before using the app.

When the Moonlight executable is left as `moonlight`, Moonscale falls back to the installed Moonlight Flatpak (`com.moonlight_stream.Moonlight`) if the command is not available directly. This also supports development from a Flatpak-sandboxed editor when `flatpak-spawn --host` is available.