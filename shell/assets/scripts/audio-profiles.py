#!/usr/bin/env python3
"""Sound outputs that need the sound card switched over first.

A laptop's sound chip carries both its own speakers and the audio of a
display plugged into it (HDMI / DisplayPort), but only one profile at a time:
with the speakers' profile active there's no output for the monitor at all,
and the other way round. This lists the outputs a profile switch would bring
(a monitor with speakers that's plugged in, or the speakers while the
monitor's profile is active) and does the switch.

  audio-profiles.py list
      JSON [{card, profile, name, kind: "display" | "speakers" | "headphones"}]
  audio-profiles.py switch <card> <profile>
      switches the card, then makes its new output the default
"""
import json
import subprocess
import sys
import time


def pactl(*args, timeout=5):
    try:
        p = subprocess.run(["pactl", *args], capture_output=True, text=True, timeout=timeout)
        return p.stdout if p.returncode == 0 else ""
    except (OSError, subprocess.TimeoutExpired):
        return ""


def cards():
    try:
        return json.loads(pactl("-f", "json", "list", "cards") or "[]")
    except ValueError:
        return []


def output_part(profile):
    return next((p for p in profile.split("+") if p.startswith("output:")), "")


def input_part(profile):
    return next((p for p in profile.split("+") if p.startswith("input:")), "")


def alternatives(card_list):
    """Outputs a switch of their card would bring, one per port."""
    out = []
    for c in card_list:
        active = c.get("active_profile") or ""
        profiles = c.get("profiles") or {}
        keep_input = input_part(active)
        for port_name, port in (c.get("ports") or {}).items():
            if not (port_name.startswith("hdmi-output") or port_name.startswith("analog-output")):
                continue
            if port.get("availability") == "not available":
                continue
            own = [p for p in port.get("profiles") or [] if profiles.get(p, {}).get("available", True)]
            if not own or active in own:
                continue  # (already playing through this port's profile)
            # Stereo, keeping the input the card has now (a laptop's microphone)
            stereo = [p for p in own if "surround" not in p]
            pick = next((p for p in stereo if input_part(p) == keep_input and keep_input), None) \
                or next((p for p in stereo if not input_part(p)), None) or (stereo or own)[0]
            if output_part(pick) == output_part(active):
                continue
            props = port.get("properties") or {}
            if port_name.startswith("hdmi-output"):
                product = props.get("device.product.name") or port.get("description") or "Display"
                out.append({"card": c["name"], "profile": pick, "name": product + " speakers", "kind": "display"})
            else:
                desc = (port.get("description") or "Speakers").lower()
                kind = "headphones" if "headphone" in desc else "speakers"
                out.append({"card": c["name"], "profile": pick, "name": "Built-in " + desc, "kind": kind})
    # (one row per profile: the laptop's speakers and headphones share one)
    seen, rows = set(), []
    for a in out:
        key = (a["card"], a["profile"])
        if key not in seen:
            seen.add(key)
            rows.append(a)
    return rows


def switch(card, profile):
    if not any(a["card"] == card and a["profile"] == profile for a in alternatives(cards())):
        print(json.dumps({"ok": False, "error": "That output isn't available any more."}))
        return 1
    if subprocess.run(["pactl", "set-card-profile", card, profile], capture_output=True).returncode != 0:
        print(json.dumps({"ok": False, "error": "The sound card didn't switch."}))
        return 1
    # Its new output: the card's sink for that profile, once it exists
    suffix = "." + output_part(profile).removeprefix("output:")
    stem = card.removeprefix("alsa_card.")
    for _ in range(20):
        sinks = [s.strip().split("\t")[1] for s in pactl("list", "short", "sinks").splitlines() if "\t" in s]
        match = next((s for s in sinks if stem in s and s.endswith(suffix)), None)
        if match:
            subprocess.run(["pactl", "set-default-sink", match], capture_output=True)
            print(json.dumps({"ok": True, "sink": match}))
            return 0
        time.sleep(0.1)
    print(json.dumps({"ok": True, "sink": None}))
    return 0


def main(argv):
    if argv[:1] == ["list"]:
        print(json.dumps(alternatives(cards())))
        return 0
    if len(argv) == 3 and argv[0] == "switch":
        return switch(argv[1], argv[2])
    print(__doc__.strip(), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
