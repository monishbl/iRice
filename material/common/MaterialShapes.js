.pragma library
// Material 3 Expressive's shape library (androidx MaterialShapes), drawn by
// materialshape.frag: [kind, n, a, b, rotation°, scale]. kinds: 0 star
// (radius 1 - a·((1 - cos nθ)/2)^b), 1 rounded polygon (n sides, rounding a),
// 2 rounded box (half-height b, rounding a), 3 ellipse (half-height a).
// Close matches by eye to Google's shapes, not their exact curves.
var shapes = {
    "circle": [0, 1, 0, 1, 0, 1],
    "square": [2, 0, 0.3, 1, 0, 0.92],
    "triangle": [1, 3, 0.22, 0, 0, 0.98],
    "diamond": [1, 4, 0.14, 0, 45, 1],
    "pentagon": [1, 5, 0.16, 0, 0, 1.02],
    "gem": [1, 6, 0.14, 0, 0, 1],
    "pill": [2, 0, 1, 0.6, -45, 1],
    "oval": [3, 0, 0.64, 0, -45, 1],
    "cookie4": [0, 4, 0.16, 1, 45, 1],
    "cookie6": [0, 6, 0.12, 1, 0, 1],
    "cookie7": [0, 7, 0.1, 1, -90, 1],
    "cookie9": [0, 9, 0.08, 1, -90, 1],
    "cookie12": [0, 12, 0.065, 1, -90, 1],
    "sunny": [0, 8, 0.11, 0.55, 0, 1],
    "verySunny": [0, 8, 0.22, 0.6, 0, 1],
    "softBurst": [0, 10, 0.13, 0.75, 0, 1],
    "burst": [0, 12, 0.2, 0.45, 0, 1],
    "boom": [0, 15, 0.26, 0.4, 0, 1],
    "softBoom": [0, 16, 0.14, 0.9, 0, 1],
    "clover4": [0, 4, 0.34, 3, 45, 1],
    "clover8": [0, 8, 0.2, 2.6, 0, 1],
    "flower": [0, 8, 0.16, 1.6, 0, 1]
};

function get(name) {
    return shapes[name] || shapes.circle;
}

// The loading indicator's cycle (Google's order)
var loading = ["softBurst", "cookie9", "pentagon", "pill", "sunny", "cookie4", "oval"];
