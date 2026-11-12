#!/usr/bin/env python3
# anim-timing.py video.mp4: when the picture changes (an animation), how long, how many
# frames, the longest gap between frames (a stutter), from the frames' own times
#
# anim-timing.py --marker [--ignore X,Y] open.mp4 [close.mp4 …]: recordings made by
# live-scenarios.sh's latency suite, which jumps the pointer (bottom right,
# 1840..1880,1040) just before the command. Per recording: ms from that jump
# to the first frame that moves elsewhere, how long the motion lasts (the
# first burst: changes less than 250 ms apart, so a caret's blink later is
# not counted), its frames and its longest gap. Prints one tab-separated line:
#   open_ms open_dur_ms open_frames open_gap_ms close_ms close_dur_ms close_gap_ms
import subprocess, sys

W, H = 384, 216


def frames(v):
    pts = [float(x.strip(",")) for x in subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "frame=pts_time", "-of", "csv=p=0", v], capture_output=True, text=True).stdout.split() if x]
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", v, "-fps_mode", "passthrough", "-vf", f"scale={W}:{H}", "-f", "rawvideo", "-pix_fmt", "gray", "-"], capture_output=True).stdout
    sz = W * H
    fr = [raw[i * sz:(i + 1) * sz] for i in range(len(raw) // sz)]
    n = min(len(fr), len(pts))
    return pts[:n], fr[:n]


def plain(v):
    pts, fr = frames(v)
    n = len(fr)
    sz = W * H
    d = [0.0] + [sum(abs(x - y) for x, y in zip(fr[i], fr[i - 1])) / sz for i in range(1, n)]
    moving = [i for i in range(1, n) if d[i] > 0.15]
    if not moving:
        print(f"{v}: no change")
        return
    a, b = moving[0], moving[-1]
    gaps = [pts[i] - pts[i - 1] for i in range(a + 1, b + 1)]
    print(f"{v.split('/')[-1]}: starts {pts[a]:.3f}s, settles {pts[b]:.3f}s, lasts {1000*(pts[b]-pts[a]):.0f} ms, {b-a+1} frames, "
          f"avg {1000*sum(gaps)/len(gaps) if gaps else 0:.1f} ms/frame, longest gap {1000*max(gaps) if gaps else 0:.0f} ms")


# the pointer's corner (bottom right, in the scaled frame): the marker
BX0, BY0 = int(W * 1780 / 1920), int(H * 1000 / 1080)


# a box around a click's target (the pointer drawn there is not the reaction): --ignore X,Y
IGNORE = None


def changed(a, b):
    """(marker corner changed, pixels changed elsewhere)"""
    corner = elsewhere = 0
    for i in range(0, W * H):
        if abs(a[i] - b[i]) > 6:
            x, y = i % W, i // W
            if x >= BX0 and y >= BY0:
                corner += 1
            elif IGNORE and abs(x - IGNORE[0]) <= 8 and abs(y - IGNORE[1]) <= 8:
                continue
            else:
                elsewhere += 1
    return corner > 0, elsewhere


def marked(v):
    """(ms to first motion, duration ms, frames, longest gap ms) or Nones"""
    pts, fr = frames(v)
    mark = None
    first = None
    last = None
    count = 0
    gap = 0.0
    for i in range(1, len(fr)):
        c, e = changed(fr[i], fr[i - 1])
        if mark is None:
            if c:
                mark = i
            continue
        if e >= 3:
            if first is None:
                first = i
            elif pts[i] - pts[last] > 0.25:
                break  # (a later, separate change: not this animation)
            else:
                gap = max(gap, pts[i] - pts[last])
            last = i
            count += 1
    if mark is None or first is None:
        return None, None, None, None
    return round(1000 * (pts[first] - pts[mark])), round(1000 * (pts[last] - pts[first])), count, round(1000 * gap)


if __name__ != "__main__":
    pass
elif sys.argv[1] == "--marker":
    args = sys.argv[2:]
    if args and args[0] == "--ignore":
        x, y = map(int, args[1].split(","))
        IGNORE = (x * W // 1920, y * H // 1080)
        args = args[2:]
    out = []
    for k, v in enumerate(args):
        r = marked(v)
        out += [r[0], r[1], r[2], r[3]] if k == 0 else [r[0], r[1], r[3]]
    print("\t".join("" if x is None else str(x) for x in out))
else:
    for v in sys.argv[1:]:
        plain(v)
