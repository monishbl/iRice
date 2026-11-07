#!/usr/bin/env python3
# anim.py video.mp4: when the picture changes (an animation), how long, how many
# frames, the longest gap between frames (a stutter), from the frames' own times
import subprocess, sys
v = sys.argv[1]
W, H = 192, 108
pts = [float(x.strip(",")) for x in subprocess.run(["ffprobe","-v","error","-select_streams","v:0","-show_entries","frame=pts_time","-of","csv=p=0",v],capture_output=True,text=True).stdout.split() if x]
raw = subprocess.run(["ffmpeg","-v","error","-i",v,"-fps_mode","passthrough","-vf",f"scale={W}:{H}","-f","rawvideo","-pix_fmt","gray","-"],capture_output=True).stdout
sz = W*H
fr = [raw[i*sz:(i+1)*sz] for i in range(len(raw)//sz)]
n = min(len(fr), len(pts))
d = [0.0] + [sum(abs(x-y) for x, y in zip(fr[i], fr[i-1]))/sz for i in range(1, n)]
moving = [i for i in range(1, n) if d[i] > 0.15]
if not moving:
    print(f"{v}: no change"); sys.exit()
a, b = moving[0], moving[-1]
gaps = [pts[i]-pts[i-1] for i in range(a+1, b+1)]
print(f"{v.split('/')[-1]}: starts {pts[a]:.3f}s, settles {pts[b]:.3f}s, lasts {1000*(pts[b]-pts[a]):.0f} ms, {b-a+1} frames, "
      f"avg {1000*sum(gaps)/len(gaps) if gaps else 0:.1f} ms/frame, longest gap {1000*max(gaps) if gaps else 0:.0f} ms")
