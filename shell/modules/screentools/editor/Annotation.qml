import QtQuick
import QtQuick.Shapes
import qs.common

// One markup item, drawn as vectors in the image's native pixels (the editor
// scales the whole layer for display and grabs it 1:1 on save).
//   pen / highlighter: { points: [Qt.point], color, width }
//   arrow:             { x1, y1, x2, y2, color, width }
//   rect / ellipse:    { x, y, w, h, color, width }
//   text:              { x, y, text, color, size }
//   pixelate:          { x, y, w, h, block } — samples `source` downscaled
Item {
    id: a

    property var ann: null
    property Item source: null // the screenshot Image, for pixelate

    readonly property string type: ann?.type ?? ""

    // Bounding box of a stroke, padded by its width
    function bounds(points, w) {
        if (!points || points.length === 0)
            return Qt.rect(0, 0, 1, 1);
        let x0 = points[0].x, y0 = points[0].y, x1 = x0, y1 = y0;
        for (const p of points) {
            x0 = Math.min(x0, p.x);
            y0 = Math.min(y0, p.y);
            x1 = Math.max(x1, p.x);
            y1 = Math.max(y1, p.y);
        }
        const pad = w / 2 + 2;
        return Qt.rect(Math.floor(x0 - pad), Math.floor(y0 - pad), Math.ceil(x1 - x0 + 2 * pad), Math.ceil(y1 - y0 + 2 * pad));
    }

    Loader {
        active: a.type !== ""
        sourceComponent: ({ pen: pen, highlighter: highlighter, arrow: arrow, rect: box, ellipse: ellipse, text: label, pixelate: pixelate })[a.type] ?? null
    }

    Component {
        id: pen
        Shape {
            width: a.width
            height: a.height
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: a.ann.color
                strokeWidth: a.ann.width
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin
                PathPolyline { path: a.ann.points }
            }
        }
    }

    // Translucent: drawn opaque into its own small layer, then faded as a
    // whole, so the stroke doesn't darken where it crosses itself
    Component {
        id: highlighter
        Item {
            id: hl
            readonly property rect b: a.bounds(a.ann.points, a.ann.width)
            x: b.x
            y: b.y
            width: b.width
            height: b.height
            opacity: 0.4
            layer.enabled: true
            Shape {
                anchors.fill: parent
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeColor: a.ann.color
                    strokeWidth: a.ann.width
                    fillColor: "transparent"
                    capStyle: ShapePath.FlatCap
                    joinStyle: ShapePath.RoundJoin
                    PathPolyline { path: a.ann.points.map(p => Qt.point(p.x - hl.b.x, p.y - hl.b.y)) }
                }
            }
        }
    }

    Component {
        id: arrow
        Shape {
            id: arrowShape
            width: a.width
            height: a.height
            preferredRendererType: Shape.CurveRenderer
            readonly property real dx: a.ann.x2 - a.ann.x1
            readonly property real dy: a.ann.y2 - a.ann.y1
            readonly property real len: Math.max(0.001, Math.hypot(dx, dy))
            readonly property real ux: dx / len
            readonly property real uy: dy / len
            // Head: length grows with the stroke, never more than 60% of the arrow
            readonly property real head: Math.min(len * 0.6, a.ann.width * 3.2 + 10)
            readonly property real half: head * 0.55
            readonly property real baseX: a.ann.x2 - ux * head
            readonly property real baseY: a.ann.y2 - uy * head
            ShapePath {
                strokeColor: a.ann.color
                strokeWidth: a.ann.width
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                startX: a.ann.x1
                startY: a.ann.y1
                // Stop inside the head so the shaft doesn't poke through its tip
                PathLine { x: arrowShape.baseX + arrowShape.ux * arrowShape.head * 0.3; y: arrowShape.baseY + arrowShape.uy * arrowShape.head * 0.3 }
            }
            ShapePath {
                strokeColor: a.ann.color
                strokeWidth: Math.max(1, a.ann.width * 0.5)
                fillColor: a.ann.color
                joinStyle: ShapePath.RoundJoin
                startX: a.ann.x2
                startY: a.ann.y2
                PathLine { x: arrowShape.baseX + arrowShape.uy * arrowShape.half; y: arrowShape.baseY - arrowShape.ux * arrowShape.half }
                PathLine { x: arrowShape.baseX - arrowShape.uy * arrowShape.half; y: arrowShape.baseY + arrowShape.ux * arrowShape.half }
                PathLine { x: a.ann.x2; y: a.ann.y2 }
            }
        }
    }

    Component {
        id: box
        Rectangle {
            x: a.ann.x
            y: a.ann.y
            width: a.ann.w
            height: a.ann.h
            color: "transparent"
            radius: Math.min(a.ann.width, Math.min(width, height) / 2)
            antialiasing: true
            border.width: a.ann.width
            border.color: a.ann.color
        }
    }

    Component {
        id: ellipse
        Shape {
            width: a.width
            height: a.height
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: a.ann.color
                strokeWidth: a.ann.width
                fillColor: "transparent"
                PathAngleArc {
                    centerX: a.ann.x + a.ann.w / 2
                    centerY: a.ann.y + a.ann.h / 2
                    radiusX: Math.max(0.5, (a.ann.w - a.ann.width) / 2)
                    radiusY: Math.max(0.5, (a.ann.h - a.ann.width) / 2)
                    startAngle: 0
                    sweepAngle: 360
                }
            }
        }
    }

    Component {
        id: label
        Text {
            x: a.ann.x
            y: a.ann.y
            text: a.ann.text
            color: a.ann.color
            textFormat: Text.PlainText
            font { family: Theme.font.family; pixelSize: a.ann.size; weight: Font.DemiBold }
        }
    }

    // Redaction: the area rendered into a texture `block` times smaller (the
    // Image is mipmapped, so each block averages its pixels), then shown
    // unsmoothed. Only the screenshot is sampled, never markup on top.
    Component {
        id: pixelate
        ShaderEffectSource {
            x: a.ann.x
            y: a.ann.y
            width: a.ann.w
            height: a.ann.h
            sourceItem: a.source
            sourceRect: Qt.rect(a.ann.x, a.ann.y, a.ann.w, a.ann.h)
            textureSize: Qt.size(Math.max(1, Math.ceil(a.ann.w / a.ann.block)), Math.max(1, Math.ceil(a.ann.h / a.ann.block)))
            smooth: false
            hideSource: false
            live: true
        }
    }
}
