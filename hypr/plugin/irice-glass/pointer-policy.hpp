#pragma once
#include <algorithm>
#include <cmath>

// Pure geometry/policy: logical coordinates, independent of monitor scale/rotation.
namespace PointerPolicy {
struct Point { double x, y; };
struct Box { double x, y, w, h; };
inline bool contains(Box b, Point p) { return p.x >= b.x && p.y >= b.y && p.x < b.x + b.w && p.y < b.y + b.h; }
inline Point clamp(Box b, Point p) { return {std::clamp(p.x, b.x, b.x + b.w - 0.01), std::clamp(p.y, b.y, b.y + b.h - 0.01)}; }
struct Resistance {
    int side = 0;
    double started = 0;
    double lastPush = 0;
    void reset() { side = 0; }
};
// Corners: a blocked strip at either end of the crossed edge, not the whole edge.
// Resistance: sustain an outward push for 180ms, or push again within 900ms.
inline Point limit(Box b, Point from, Point to, int mode, double corner, double now, Resistance& state) {
    if (contains(b, to)) return to;
    const double dx = to.x - from.x, dy = to.y - from.y;
    double t = 2;
    int side = 0;
    auto hit = [&](double at, int edge) { if (at >= 0 && at <= 1 && at < t) { t = at; side = edge; } };
    if (dx < 0) hit((b.x - from.x) / dx, 1);
    if (dx > 0) hit((b.x + b.w - from.x) / dx, 2);
    if (dy < 0) hit((b.y - from.y) / dy, 3);
    if (dy > 0) hit((b.y + b.h - from.y) / dy, 4);
    if (!side) return to;
    const Point cross{from.x + dx * t, from.y + dy * t};
    if (mode == 1) {
        const double reach = std::min(corner, std::min(b.w, b.h) / 3);
        const bool near = side <= 2 ? cross.y - b.y < reach || b.y + b.h - cross.y <= reach
                                   : cross.x - b.x < reach || b.x + b.w - cross.x <= reach;
        return near ? clamp(b, to) : to;
    }
    if (mode == 2) {
        if (state.side != side || now - state.lastPush > 900) {
            state.side = side; state.started = now;
        }
        state.lastPush = now;
        if (now - state.started >= 180) { state.reset(); return to; }
        return clamp(b, to);
    }
    return to;
}
}
