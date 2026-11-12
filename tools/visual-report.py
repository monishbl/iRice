#!/usr/bin/env python3
"""visual-report.py OUT SCENES: the page for visual-compare.sh's shots.

OUT/qml/<look>-<scene>.png and OUT/rust/<look>-<scene>.png in, OUT/diff/…png
(the changed pixels in red over a faded copy) and OUT/index.html out: every
screen with three views (a slider to drag between them, side by side, the
diff), sorted by how much differs, filterable by look and name.
"""
import json
import os
import re
import subprocess
import sys

out, scenes_file = sys.argv[1], sys.argv[2]
order = []
for line in open(scenes_file):
    line = line.strip()
    if line and not line.startswith("#"):
        order.append(line.split("|", 1)[0].strip())

os.makedirs(os.path.join(out, "diff"), exist_ok=True)
items = []
for side_file in sorted(os.listdir(os.path.join(out, "qml"))):
    m = re.match(r"(material|glass)-(.+)\.png$", side_file)
    if not m:
        continue
    look, scene = m.groups()
    q = os.path.join(out, "qml", side_file)
    r = os.path.join(out, "rust", side_file)
    if not os.path.exists(r):
        items.append({"look": look, "scene": scene, "qml": f"qml/{side_file}", "rust": "", "diff": "", "pct": None})
        continue
    d = os.path.join(out, "diff", side_file)
    # (the changed pixels: red over a faded copy of the QML shot)
    res = subprocess.run(["magick", "compare", "-metric", "AE", "-fuzz", "6%", "-highlight-color", "#ff3b30",
                          "-lowlight-color", "#00000000", q, r, d], capture_output=True, text=True)
    m2 = re.match(r"\s*([\d.e+]+)", res.stderr or "")
    try:
        w, h = map(int, subprocess.run(["magick", "identify", "-format", "%w %h", q], capture_output=True, text=True).stdout.split())
        pct = float(m2.group(1)) / (w * h) * 100 if m2 else None
    except ValueError:
        pct = None
    subprocess.run(["magick", q, "-fill", "white", "-colorize", "70%", d, "-composite", d], capture_output=True)
    items.append({"look": look, "scene": scene, "qml": f"qml/{side_file}", "rust": f"rust/{side_file}", "diff": f"diff/{side_file}", "pct": pct})

rank = {s: i for i, s in enumerate(order)}
items.sort(key=lambda it: (it["look"], rank.get(it["scene"], 999)))

page = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>QML vs Rust</title>
<style>
:root {
  --frame: #F4F4F5; --canvas: #FAFAFA; --surface: #FFFFFF; --muted: #F4F4F5; --pressed: #E4E4E7;
  --border: #E4E4E7; --border-strong: #D4D4D8;
  --primary: #18181B; --secondary: #52525B; --tertiary: #6B6B74;
  --accent: #4F46E5; --accent-tint: #EEF2FF; --focus: #6366F1;
  --danger: #B91C1C; --warning: #B45309; --success: #15803D;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --frame: #0A0A0B; --canvas: #111113; --surface: #1A1A1D; --muted: #2C2C31; --pressed: #333338;
    --border: #26262A; --border-strong: #36363C;
    --primary: #EDEDEF; --secondary: #A1A1AA; --tertiary: #94949D;
    --accent: #818CF8; --accent-tint: #1E1B4B; --focus: #6366F1;
    --danger: #F87171; --warning: #FBBF24; --success: #4ADE80;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--canvas); color: var(--primary); font: 14px/20px "Geist", system-ui, sans-serif; font-variant-numeric: tabular-nums; }
.app { display: grid; grid-template-columns: 256px 1fr; height: 100vh; }
aside { background: var(--frame); border-right: 1px solid var(--border); display: flex; flex-direction: column; min-height: 0; }
.side-top { padding: 16px; display: grid; gap: 12px; }
h1 { font-size: 16px; line-height: 24px; font-weight: 600; margin: 0; }
.sub { color: var(--secondary); font-size: 13px; line-height: 18px; margin-top: 4px; }
input[type=search] { width: 100%; height: 36px; padding: 0 12px; border-radius: 10px; border: 1px solid var(--border-strong); background: var(--surface); color: var(--primary); font: inherit; }
input[type=search]:focus { outline: none; border-color: var(--focus); box-shadow: 0 0 0 3px color-mix(in srgb, var(--focus) 40%, transparent); }
.seg { display: flex; gap: 4px; background: var(--muted); padding: 4px; border-radius: 10px; }
.seg button { flex: 1; height: 28px; border: 0; border-radius: 6px; background: transparent; color: var(--secondary); font: inherit; font-size: 13px; cursor: pointer; }
.seg button[aria-pressed=true] { background: var(--surface); color: var(--primary); font-weight: 500; }
.seg button:focus-visible, .list button:focus-visible, .views button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.list { overflow: auto; padding: 0 8px 16px; flex: 1; }
.list button { display: flex; justify-content: space-between; align-items: center; width: 100%; height: 36px; padding: 0 8px; border: 0; border-radius: 10px; background: transparent; color: var(--primary); font: inherit; font-size: 13px; text-align: left; cursor: pointer; }
.list button:hover { background: var(--pressed); }
.list button[aria-current=true] { background: var(--surface); box-shadow: 0 1px 2px rgba(24,24,27,.06); }
.list .pct { color: var(--tertiary); font-size: 12px; }
.list .pct.hi { color: var(--danger); }
.list .pct.mid { color: var(--warning); }
.empty { color: var(--secondary); padding: 16px; }
main { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.bar { height: 56px; display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 0 24px; border-bottom: 1px solid var(--border); }
.title { font-weight: 600; font-size: 16px; }
.meta { color: var(--secondary); font-size: 13px; }
.views { display: flex; gap: 4px; background: var(--muted); padding: 4px; border-radius: 10px; }
.views button { height: 28px; padding: 0 12px; border: 0; border-radius: 6px; background: transparent; color: var(--secondary); font: inherit; font-size: 13px; cursor: pointer; }
.views button[aria-pressed=true] { background: var(--surface); color: var(--primary); font-weight: 500; }
.stage { flex: 1; overflow: auto; padding: 24px; }
.labels { display: flex; justify-content: space-between; color: var(--secondary); font-size: 13px; margin-bottom: 8px; }
.frame { position: relative; border: 1px solid var(--border); border-radius: 14px; overflow: hidden; background: var(--surface); max-width: 1600px; margin: 0 auto; }
.frame img { display: block; width: 100%; height: auto; user-select: none; -webkit-user-drag: none; }
.slider .top { position: absolute; inset: 0; clip-path: inset(0 0 0 var(--cut, 50%)); }
.slider .handle { position: absolute; top: 0; bottom: 0; left: var(--cut, 50%); width: 2px; margin-left: -1px; background: var(--accent); cursor: ew-resize; }
.slider .handle::after { content: ""; position: absolute; top: 50%; left: 50%; width: 32px; height: 32px; margin: -16px 0 0 -16px; border-radius: 50%; background: var(--accent); box-shadow: 0 4px 16px rgba(24,24,27,.24); }
.slider input[type=range] { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: ew-resize; margin: 0; }
.sbs { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; max-width: 2400px; margin: 0 auto; }
.missing { padding: 48px; text-align: center; color: var(--secondary); }
.hint { color: var(--tertiary); font-size: 12px; text-align: center; margin-top: 12px; }
@media (max-width: 900px) {
  .app { grid-template-columns: 1fr; grid-template-rows: auto 1fr; height: auto; }
  aside { border-right: 0; border-bottom: 1px solid var(--border); max-height: 45vh; }
  .sbs { grid-template-columns: 1fr; }
  .bar { height: auto; flex-wrap: wrap; padding: 12px 16px; }
  .stage { padding: 16px; }
}
@media (prefers-reduced-motion: no-preference) { .list button, .seg button, .views button { transition: background 150ms ease-out; } }
</style>
</head>
<body>
<div class="app">
  <aside>
    <div class="side-top">
      <div>
        <h1>QML vs Rust</h1>
        <div class="sub">Left: QML before the rewrite (342a6ac). Right: Rust today. Same settings and wallpaper.</div>
      </div>
      <div class="seg" role="group" aria-label="Look">
        <button type="button" data-look="material" aria-pressed="true">Material</button>
        <button type="button" data-look="glass" aria-pressed="false">Glass</button>
      </div>
      <input type="search" id="q" placeholder="Filter screens" aria-label="Filter screens">
      <div class="seg" role="group" aria-label="Order">
        <button type="button" data-sort="flow" aria-pressed="true">In order</button>
        <button type="button" data-sort="diff" aria-pressed="false">Most different</button>
      </div>
    </div>
    <nav class="list" id="list" aria-label="Screens"></nav>
  </aside>
  <main>
    <div class="bar">
      <div><div class="title" id="title"></div><div class="meta" id="meta"></div></div>
      <div class="views" role="group" aria-label="View">
        <button type="button" data-view="slider" aria-pressed="true">Slider</button>
        <button type="button" data-view="sbs" aria-pressed="false">Side by side</button>
        <button type="button" data-view="diff" aria-pressed="false">Diff</button>
      </div>
    </div>
    <div class="stage" id="stage"></div>
  </main>
</div>
<script>
const ITEMS = __ITEMS__;
let look = "material", view = "slider", sort = "flow", cur = null, cut = 50;
try { view = localStorage.getItem("vc-view") || view; look = localStorage.getItem("vc-look") || look; } catch (e) {}
const $ = s => document.querySelector(s);
function shown() {
  const q = $("#q").value.trim().toLowerCase();
  let list = ITEMS.filter(it => it.look === look && it.scene.toLowerCase().includes(q));
  if (sort === "diff") list = [...list].sort((a, b) => (b.pct ?? -1) - (a.pct ?? -1));
  return list;
}
function pctText(p) { return p == null ? "no Rust shot" : p.toFixed(1) + "% differs"; }
function renderList() {
  const list = shown(), nav = $("#list");
  nav.innerHTML = "";
  if (!list.length) { nav.innerHTML = '<div class="empty">No screens match. Clear the filter to see them all.</div>'; return; }
  if (!cur || !list.includes(cur)) cur = list[0];
  for (const it of list) {
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("aria-current", it === cur ? "true" : "false");
    const cls = it.pct == null ? "hi" : it.pct > 10 ? "hi" : it.pct > 3 ? "mid" : "";
    b.innerHTML = `<span>${it.scene}</span><span class="pct ${cls}">${it.pct == null ? "missing" : it.pct.toFixed(1) + "%"}</span>`;
    b.onclick = () => { cur = it; renderList(); renderStage(); };
    nav.appendChild(b);
  }
}
function img(src, alt) { return `<img src="${src}" alt="${alt}" loading="lazy">`; }
function renderStage() {
  const st = $("#stage");
  if (!cur) { st.innerHTML = ""; $("#title").textContent = ""; $("#meta").textContent = ""; return; }
  $("#title").textContent = cur.scene;
  $("#meta").textContent = `${cur.look === "glass" ? "Glass" : "Material"} · ${pctText(cur.pct)}`;
  if (!cur.rust) { st.innerHTML = `<div class="labels"><span>QML</span></div><div class="frame">${img(cur.qml, "QML")}</div><div class="missing">The Rust side has no shot of this screen.</div>`; return; }
  if (view === "slider") {
    st.innerHTML = `<div class="labels"><span>QML</span><span>Rust</span></div>
      <div class="frame slider" style="--cut:${cut}%">${img(cur.qml, "QML")}<div class="top">${img(cur.rust, "Rust")}</div>
      <div class="handle" aria-hidden="true"></div>
      <input type="range" min="0" max="100" step="0.1" value="${cut}" aria-label="Divider: left QML, right Rust"></div>
      <div class="hint">Drag across the picture, or use ← → after clicking it. ↑ ↓ change screen.</div>`;
    const fr = st.querySelector(".frame"), r = st.querySelector("input");
    r.oninput = () => { cut = +r.value; fr.style.setProperty("--cut", cut + "%"); };
  } else if (view === "sbs") {
    st.innerHTML = `<div class="sbs"><div><div class="labels"><span>QML</span></div><div class="frame">${img(cur.qml, "QML")}</div></div>
      <div><div class="labels"><span>Rust</span></div><div class="frame">${img(cur.rust, "Rust")}</div></div></div>`;
  } else {
    st.innerHTML = `<div class="labels"><span>Changed pixels in red, over a faded QML shot</span></div><div class="frame">${img(cur.diff, "Difference")}</div>`;
  }
}
function press(group, attr, val) { document.querySelectorAll(`[${attr}]`).forEach(b => b.setAttribute("aria-pressed", b.getAttribute(attr) === val ? "true" : "false")); }
document.querySelectorAll("[data-look]").forEach(b => b.onclick = () => { look = b.dataset.look; press(0, "data-look", look); try { localStorage.setItem("vc-look", look); } catch (e) {} const s = cur && cur.scene; cur = ITEMS.find(it => it.look === look && it.scene === s) || null; renderList(); renderStage(); });
document.querySelectorAll("[data-view]").forEach(b => b.onclick = () => { view = b.dataset.view; press(0, "data-view", view); try { localStorage.setItem("vc-view", view); } catch (e) {} renderStage(); });
document.querySelectorAll("[data-sort]").forEach(b => b.onclick = () => { sort = b.dataset.sort; press(0, "data-sort", sort); renderList(); });
$("#q").oninput = () => { renderList(); renderStage(); };
document.addEventListener("keydown", e => {
  if (e.target.matches("input[type=search]")) return;
  const list = shown(), i = list.indexOf(cur);
  if (e.key === "ArrowDown" && i < list.length - 1) { cur = list[i + 1]; e.preventDefault(); }
  else if (e.key === "ArrowUp" && i > 0) { cur = list[i - 1]; e.preventDefault(); }
  else if (e.key === "1" || e.key === "2" || e.key === "3") { view = ["slider", "sbs", "diff"][+e.key - 1]; press(0, "data-view", view); }
  else return;
  renderList(); renderStage();
});
press(0, "data-look", look); press(0, "data-view", view);
renderList(); renderStage();
</script>
</body>
</html>
"""
open(os.path.join(out, "index.html"), "w").write(page.replace("__ITEMS__", json.dumps(items)))
print(f"{len(items)} screens: {os.path.join(out, 'index.html')}")
