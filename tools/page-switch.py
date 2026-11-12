#!/usr/bin/env python3
"""page-switch.py OUT/NAME x y w h (after tools/settings-scenarios.sh clicks NAME): per click of a scen.sh `clicks` recording, when the
region (the page) first changes after the click's release, when it settles,
and the longest run of identical frames in between (a stall). The video's
clock is tied to the wall clock by the marker: the pointer jumping into the
region at the time in NAME.marker."""
import subprocess, sys
name, x, y, w, h = sys.argv[1], *map(int, sys.argv[2:6])
RELEASE = 0.95  # from a click's invocation to its button release (vptr: move, glide 200, sleep 400, click)
W, H = max(1, w // 4), max(1, h // 4)
raw = subprocess.run(["ffmpeg", "-v", "error", "-i", name + ".mp4", "-vf", f"crop={w}:{h}:{x}:{y},scale={W}:{H}", "-f", "rawvideo", "-pix_fmt", "gray", "-"], capture_output=True).stdout
sz = W * H
fr = [raw[i * sz:(i + 1) * sz] for i in range(len(raw) // sz)]
n = len(fr)
d = [0.0] + [sum(abs(a - b) for a, b in zip(fr[i], fr[i - 1])) / sz for i in range(1, n)]
cs = [float(l) for l in open(name + ".clicks").read().split()]
m = float(open(name + ".marker").read().split()[0])
vm = next(i for i in range(5, n) if d[i] > 0.05) / 60.0  # the marker: the pointer appears
lat, sett, stall = [], [], []
for k, c in enumerate(cs):
    t = vm + (c - m) + RELEASE
    i0 = max(1, int(t * 60) - 6)
    i1 = min(n, int((t + 1.0) * 60))
    moving = [i for i in range(i0, i1) if d[i] > 0.4]
    if not moving:
        print(f"click {k}: t={t:.3f}s no change"); continue
    a, b = moving[0], moving[-1]
    run = best = 0
    for i in range(a + 1, b + 1):
        run = run + 1 if d[i] < 0.02 else 0
        best = max(best, run)
    l = (a / 60 - t) * 1000
    lat.append(l); sett.append((b / 60 - t) * 1000); stall.append(best * 1000 / 60)
    print(f"click {k}: first change {l:+5.0f} ms, settles {(b/60-t)*1000:+5.0f} ms, {len(moving):2d} changed frames, longest stall {best*1000/60:4.0f} ms")
if lat:
    print(f"mean first change {sum(lat)/len(lat):.0f} ms (max {max(lat):.0f}), mean settle {sum(sett)/len(sett):.0f} ms, worst stall {max(stall):.0f} ms")
