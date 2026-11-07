#!/usr/bin/env python3
# region.py video x y w h: frames where only this region changes (in 1920x1080 px)
import subprocess, sys
v, x, y, w, h = sys.argv[1], *map(int, sys.argv[2:6])
W, H = w // 4, h // 4
pts = [float(t.strip(",")) for t in subprocess.run(["ffprobe","-v","error","-select_streams","v:0","-show_entries","frame=pts_time","-of","csv=p=0",v],capture_output=True,text=True).stdout.split() if t]
raw = subprocess.run(["ffmpeg","-v","error","-i",v,"-fps_mode","passthrough","-vf",f"crop={w}:{h}:{x}:{y},scale={W}:{H}","-f","rawvideo","-pix_fmt","gray","-"],capture_output=True).stdout
sz = W*H; fr = [raw[i*sz:(i+1)*sz] for i in range(len(raw)//sz)]
ch = [(round(pts[i],3), round(sum(abs(a-b) for a,b in zip(fr[i],fr[i-1]))/sz,2)) for i in range(1, min(len(fr),len(pts)))]
moving = [c for c in ch if c[1] > 0.3]
print(f"{v.split('/')[-1]} region {x},{y} {w}x{h}: {len(moving)} changed frames", moving[:12])
