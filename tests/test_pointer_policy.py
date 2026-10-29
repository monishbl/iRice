"""Pointer boundary and focus regression checks without a running compositor."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PointerPolicyTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which('g++'), 'requires C++ compiler')
    def test_geometry_corners_resistance_and_scaled_negative_origins(self):
        source = r'''
#include "pointer-policy.hpp"
#include <cassert>
using namespace PointerPolicy;
int main() {
    Box b{-1280, -100, 1280, 720}; Resistance r;
    auto p = limit(b, {-2, -95}, {20, -95}, 1, 24, 0, r);
    assert(p.x < 0 && p.y == -95); // shared top-right corner
    p = limit(b, {-2, 200}, {20, 200}, 1, 24, 0, r);
    assert(p.x == 20); // cross the middle freely
    p = limit(b, {-2, 200}, {20, 200}, 2, 24, 100, r);
    assert(p.x < 0); // first push stopped
    p = limit(b, {-0.01, 200}, {20, 200}, 2, 24, 200, r);
    assert(p.x < 0); // continuous jitter doesn't instantly release
    p = limit(b, {-0.01, 200}, {20, 200}, 2, 24, 300, r);
    assert(p.x == 20); // sustained / second push releases
    r.reset();
    limit(b, {-2, 200}, {20, 200}, 2, 24, 100, r);
    p = limit(b, {-2, 200}, {20, 200}, 2, 24, 1200, r);
    assert(p.x < 0); // old attempt expires
    p = limit(b, {-2, -95}, {20, -95}, 0, 24, 0, r);
    assert(p.x == 20); // off/capture-only has no general barrier
    p = limit(b, {-100, -2}, {10, -110}, 1, 24, 0, r);
    assert(contains(b, p)); // diagonal crossing at the top corner
    p = clamp({0, 0, 1536, 864}, {1800, -30});
    assert(p.x < 1536 && p.y == 0); // already logical scaled monitor geometry
}
'''
        with tempfile.TemporaryDirectory() as tmp:
            cpp = Path(tmp) / 'test.cpp'
            cpp.write_text(source)
            binary = str(Path(tmp) / 'test')
            subprocess.run(['g++', '-std=c++17', '-I', str(ROOT / 'hypr/plugin/irice-glass'), str(cpp), '-o', binary], check=True, capture_output=True)
            subprocess.run([binary], check=True, capture_output=True)

    @unittest.skipUnless(shutil.which('lua'), 'requires Lua')
    def test_stationary_refocus_and_new_window_warp_only_when_hyprland_focused_it(self):
        script = r'''
local focus = dofile(arg[1])
local calls = {}
local window = {mapped=true, hidden=false, accepts_input=true, active=true, visible=true, at={x=1920,y=0}, size={x=800,y=600}}
local cursor = {x=20,y=30}
hl = {
 get_cursor_pos = function() return cursor end,
 get_window = function(selector) assert(selector == 'address:0xab'); return window end,
 dispatch = function(command) table.insert(calls,command) end,
 dsp = {focus=function(v) return {focus=v.window} end, cursor={move=function(v) return {move=v} end}}
}
focus('')
assert(#calls == 1 and calls[1].move.x == 20 and calls[1].move.y == 30)
-- Hyprland focused it and shows it: the pointer moves onto it, nothing else
calls={}; focus('0xab')
assert(#calls == 1 and calls[1].move.x == 2320 and calls[1].move.y == 300)
cursor={x=2000,y=100}; calls={}; focus('0xab')
assert(#calls == 1 and calls[1].move.x == 2000 and calls[1].move.y == 100)
-- Not focused by Hyprland (a rule, a dialog behind), or on a space out of
-- sight: left alone
window.active=false; calls={}; focus('0xab'); assert(#calls == 0)
window.active=true; window.visible=false; calls={}; focus('0xab'); assert(#calls == 0)
window.visible=true; window.accepts_input=false; calls={}; focus('0xab'); assert(#calls == 0)
window.accepts_input=true; window.hidden=true; calls={}; focus('0xab'); assert(#calls == 0)
'''
        with tempfile.TemporaryDirectory() as tmp:
            test = Path(tmp) / 'test.lua'
            test.write_text(script)
            subprocess.run(['lua', str(test), str(ROOT / 'shell/assets/scripts/pointer-focus.lua')], check=True, capture_output=True)
