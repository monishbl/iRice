#!/usr/bin/env python3
"""Long real values for Settings → Software Update (the `about` feed), as
lines tessera-ui reads on stdin (tools/updater-layout.sh).
  updater-layout-feed.py <scenario> → one JSON line.
scenario: plan | running | build | failed"""
import json
import sys

sc = sys.argv[1] if len(sys.argv) > 1 else "plan"
long_commit = ("Lock after a sleep: fingerprint and face checks stop before the system sleeps and start fresh 1.5 s after it wakes "
               "(one left running across a sleep kept fprintd's reader claimed: every new check was refused and retried every 0.3 s, "
               "and the lock never recovered); \"reader busy\" retries back off 0.3 → 5 s; a password check that doesn't answer in 20 s is stopped")
pkgs = [{"name": n, "from": f"{i}.{i * 3 % 17}.{i % 9}-{i % 4 + 1}", "to": f"{i}.{i * 3 % 17 + 1}.0-1"} for i, n in enumerate(
    ["linux-cachyos-lts-headers", "lib32-mesa-vulkan-intel-git", "xdg-desktop-portal-hyprland-git", "python-cryptography", "qt6-declarative",
     "noto-fonts-cjk", "gst-plugins-bad-libs", "nvidia-open-dkms-beta", "hyprland-protocols", "wireplumber", "pipewire-pulse", "glibc",
     "gcc-libs", "systemd-libs", "networkmanager-openvpn", "bluez-utils-compat", "firefox-developer-edition", "thunderbird", "libreoffice-fresh",
     "texlive-latexextra", "rust", "cargo-nextest", "nodejs-lts-iron", "python-protobuf", "ttf-jetbrains-mono-nerd", "papirus-icon-theme",
     "kitty-terminfo", "openssh", "curl", "ffmpeg", "mpv", "yt-dlp", "imagemagick", "inkscape", "gimp", "blender", "obs-studio",
     "virtualbox-host-dkms", "docker-buildx", "podman-compose"], 1)]
su = {"running": sc == "running", "checking": False, "checked": True, "anything": True, "restartPending": True, "canUpdate": True,
      "plan": {"system": True, "aur": True, "sysCount": 38, "aurCount": 2, "tessera": True, "install": 2, "remove": 1, "setup": True},
      "hyprNow": "0.57.1 (git 4f3c2a1b, built from the hyprland-git AUR package)", "hyprTooNew": True, "hyprShort": "0.58.0-rc2",
      "hyprMax": "0.57.x", "hold": -1, "held": [], "heldHypr": [], "includeAur": True, "removeDropped": True,
      "snapshot": "@snapper/root 1287 \"before Tessera update 2026-11-18 10:52 (system + Tessera + glass plugin)\"", "undecided": 0,
      "steps": [{"name": n, "now": n == "Installing packages"} for n in ["Asking for your password", "Snapshot", "Downloading 40 packages (612 MB)",
                                                                       "Installing packages", "Building the glass plugin for Hyprland 0.58.0-rc2",
                                                                       "Updating Tessera", "Restarting the shell"]]}
tu = {"allowed": True, "available": True, "error": "",
      "report": {"current": "0.1.0+1c236def (beta, built 2026-11-18 from rust-rewrite)", "latest": "0.1.0+06fe2a55 (beta, 2026-11-18)",
                 "downgrade": True, "channelName": "Stable", "ui": "Rust shell (tessera-ui 0.1.0, x86-64-v3, release)", "ahead": 37,
                 "changelog": [{"title": long_commit[:140], "date": "18 Nov 2026 · 06fe2a55", "body": "- " + long_commit + "\n- `~/.local/share/tessera/bin/tessera-ui-1c236def85aff8775a6062089d8595ae1aae8401` kept"}] * 3,
                 "notes": [], "leaving": [long_commit[:90], "Partial rendering on FemtoVG: Slint 1.18.1's FemtoVG renderer vendored"]},
      "info": {"channel": "beta", "channelName": "Beta", "current": "0.1.0+1c236def85aff8775a6062089d8595ae1aae8401", "date": "2026-11-18 10:31",
               "branch": "worktree-agent-a104bb0fbacb03052-updater-layout-from-round2"},
      "conflicts": [{"id": "c1", "kind": "code", "title": "shell/assets/scripts/quickshare/nearby/service.py (and 3 more files under shell/assets/scripts)",
                     "detail": "You changed it on 17 Nov; the update changes the transfer timing and the Wi-Fi Direct channel list.",
                     "mine": "+42 −7 lines", "theirs": "+118 −36 lines", "change": "modified", "choices": ["mine", "theirs", "both"]},
                    {"id": "c2", "kind": "setting", "title": "appearance.glassDepth (the bevel and the shadow under every glass surface)",
                     "detail": "New default: depth on small surfaces only; yours: always thin.", "choices": ["keep", "new"],
                     "labels": {"keep": "Keep thin", "new": "Use the new default"}}],
      "choices": {}, "missing": ["python-zeroconf-and-a-long-package-name-git", "hyprland-glass-plugin-headers"],
      "reasons": {"python-zeroconf-and-a-long-package-name-git": "Quick Share finds phones on your network with it (mDNS)",
                  "hyprland-glass-plugin-headers": "builds the glass plugin for Hyprland 0.58.0-rc2"},
      "dropped": ["irice-legacy-helper-from-before-the-rename"]}
up = {"list": pkgs[:38], "aurList": pkgs[38:], "job": None, "running": sc == "running", "cancellable": sc == "running", "progress": 0.43,
      "fake": False, "showDetails": True,
      "log": "(12/40) upgrading lib32-mesa-vulkan-intel-git-1:25.3.0_devel.214522.6c1f2e7d8b9a-1 [###########--------]  57%\n" * 6}
if sc == "running":
    up["job"] = {"state": "running", "text": "Installing packages: lib32-mesa-vulkan-intel-git and 27 more, then Tessera", "phase": "install",
                 "package": "lib32-mesa-vulkan-intel-git 1:25.3.0_devel.214522.6c1f2e7d8b9a-1 → 1:25.3.0_devel.214611.0a1b2c3d4e5f-1 (12 of 40)", "aur": True}
build = None
if sc in ("build", "failed"):
    build = {"id": 3, "title": "Building tessera-ui for the branch updater-layout (x86-64-v3, release profile, fat LTO)",
             "step": "Compiling tessera-settings, tessera-host-system, tessera-surfaces-menubar and 14 more crates (412 of 503)",
             "time": "6:42 so far · about 4 minutes left on this machine (24 cores, 9 GB memory budget)",
             "fraction": 0.81 if sc == "build" else -1, "failed": sc == "failed", "details": True,
             "log": "   Compiling tessera-settings v0.1.0 (/home/user/tessera-build/native/tessera-ui/surfaces/settings)\n" * 8}
bs = {"booted": "", "snapshots": []}
about = {"su": su, "tu": tu, "up": up, "bs": bs, "build": build, "jobElapsed": 187.0}
print(json.dumps({"to": "settings", "feed": {"about": about}}))
