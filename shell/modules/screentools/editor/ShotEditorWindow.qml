import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.services

// Markup editor: a floating glass window sized to the screenshot. Tools on
// top (crop, pen, highlighter, arrow, rectangle, ellipse, text, redact) with
// undo/redo; colours and stroke widths, and Delete / Share / Copy / Save at
// the bottom. Markup is vector items over the image, flattened with
// grabToImage at the image's native resolution when it leaves the editor.
GlassWindow {
    id: win

    required property var session
    readonly property var s: session

    name: "shotEditor"
    screen: s.screen ?? Quickshell.screens[0]
    WlrLayershell.layer: WlrLayer.Overlay
    // The share sheet needs the keyboard while it's over the editor
    WlrLayershell.keyboardFocus: Panels.open === "share" ? WlrKeyboardFocus.None : WlrKeyboardFocus.Exclusive

    readonly property int m: 24 // shadow room
    readonly property int pad: 12
    readonly property int bar: 36
    readonly property string runtime: Quickshell.env("XDG_RUNTIME_DIR") || "/tmp"

    function fileUrl(p) {
        return "file://" + p.split("/").map(encodeURIComponent).join("/");
    }
    function baseName(p) {
        return p.split("/").pop();
    }

    // ---- Geometry -----------------------------------------------------------
    readonly property bool ready: base.status === Image.Ready && base.implicitWidth > 0
    readonly property int imgW: ready ? base.implicitWidth : 0
    readonly property int imgH: ready ? base.implicitHeight : 0
    readonly property rect full: Qt.rect(0, 0, imgW, imgH)
    // What the canvas shows: everything while cropping, else the crop
    readonly property rect shown: s.tool === "crop" || s.crop.width <= 0 ? full : s.crop
    // Window: the whole image fitted in 82% of the screen, at most 1:1
    readonly property real maxCanvasW: (screen?.width ?? 1920) * 0.82 - 2 * pad
    readonly property real maxCanvasH: (screen?.height ?? 1080) * 0.82 - 2 * bar - 4 * pad
    readonly property real fitFull: ready ? Math.min(maxCanvasW / imgW, maxCanvasH / imgH, 1) : 1
    readonly property int canvasW: ready ? Math.max(720, Math.round(imgW * fitFull)) : 720
    readonly property int canvasH: ready ? Math.max(240, Math.round(imgH * fitFull)) : 440
    readonly property real fit: shown.width > 0 ? Math.min(canvasW / shown.width, canvasH / shown.height, 2) : 1
    // Stroke unit: the same marks look the same on any screenshot size
    readonly property real unit: Math.max(1, Math.min(imgW, imgH) / 360)
    readonly property var penWidths: [unit, unit * 2, unit * 3.5]
    readonly property var textSizes: [unit * 6, unit * 9, unit * 13]
    readonly property var blockSizes: [unit * 3, unit * 5, unit * 8]
    readonly property color ink: s.palette[s.colorIndex] ?? s.palette[0]

    readonly property int cardW: canvasW + 2 * pad
    readonly property int cardH: canvasH + 2 * bar + 4 * pad
    implicitWidth: cardW + 2 * m
    implicitHeight: cardH + 2 * m

    // ---- State --------------------------------------------------------------
    property var draft: null // the mark being drawn
    property var editText: null // { x, y, color, size } while typing
    property rect pendingCrop: Qt.rect(0, 0, 0, 0)
    property string lastTool: "pen" // where Crop returns to
    property string busy: "" // "" | save | saveCopy | copy | share
    // Inline banner: null | { kind: "error" | "prompt", title, body }
    property var banner: null
    property string retry: "" // which action the error's Try again repeats
    property string toast: ""

    function showToast(t) {
        toast = t;
        toastTimer.restart();
    }
    Timer {
        id: toastTimer
        interval: 2000
        onTriggered: win.toast = ""
    }
    function fail(action, title, body) {
        busy = "";
        retry = action;
        banner = { kind: "error", title: title, body: body };
    }

    // ---- Tools ----------------------------------------------------------------
    function setTool(t) {
        if (!ready || t === s.tool)
            return;
        commitText();
        if (s.tool === "crop")
            applyCrop(false);
        if (t === "crop") {
            lastTool = s.tool;
            pendingCrop = s.crop.width > 0 ? s.crop : full;
        }
        s.tool = t;
    }
    function applyCrop(leave) {
        const r = pendingCrop;
        const whole = r.x <= 0 && r.y <= 0 && r.width >= imgW && r.height >= imgH;
        const next = whole || r.width < 2 || r.height < 2 ? Qt.rect(0, 0, 0, 0) : Qt.rect(Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height));
        if (next.x !== s.crop.x || next.y !== s.crop.y || next.width !== s.crop.width || next.height !== s.crop.height)
            s.setCrop(next);
        if (leave !== false)
            s.tool = lastTool;
    }
    Connections {
        target: win.s
        function onCropChanged() {
            if (win.s.tool === "crop")
                win.pendingCrop = win.s.crop.width > 0 ? win.s.crop : win.full;
        }
    }
    function cancelCrop() {
        pendingCrop = s.crop.width > 0 ? s.crop : full;
        s.tool = lastTool;
    }

    function commitText() {
        if (!editText)
            return;
        const t = textInput.text;
        if (t.trim() !== "")
            s.add({ type: "text", x: editText.x, y: editText.y, text: t, color: editText.color, size: editText.size });
        editText = null;
        textInput.text = "";
        body.forceActiveFocus();
    }
    function cancelText() {
        editText = null;
        textInput.text = "";
        body.forceActiveFocus();
    }
    // Before anything leaves the editor: typed text and a pending crop count
    function prepare() {
        commitText();
        if (s.tool === "crop")
            applyCrop();
        draft = null;
        banner = null;
    }

    // ---- Leaving ---------------------------------------------------------------
    property real progress: 0
    property bool leaving: false
    Component.onCompleted: {
        if (s.tool === "crop") // left over from a session that ended mid-crop
            s.tool = lastTool;
        enterAnim.start();
    }
    NumberAnimation { id: enterAnim; target: win; property: "progress"; to: 1; duration: 300; easing.type: Easing.OutCubic }
    property var after: null
    function leave(fn) {
        if (leaving)
            return;
        leaving = true;
        after = fn;
        enterAnim.stop();
        leaveAnim.restart();
    }
    SequentialAnimation {
        id: leaveAnim
        NumberAnimation { target: win; property: "progress"; to: 0; duration: 200; easing.type: Easing.InCubic }
        // Deferred: the callback unloads this window
        ScriptAction { script: { const f = win.after; Qt.callLater(() => { if (f) f(); }); } }
    }

    function requestClose() {
        commitText();
        if (s.tool === "crop")
            cancelCrop();
        if (s.dirty && ready) {
            banner = { kind: "prompt", title: "Save changes to this screenshot?", body: "Your markup will be lost if you don't save it." };
            return;
        }
        leave(() => s.closeEditor());
    }
    // A newer screenshot was clicked while this one has changes
    Connections {
        target: win.s
        function onPendingOpenChanged() {
            if (win.s.pendingOpen !== "")
                win.banner = { kind: "prompt", title: "Save changes before opening the new screenshot?", body: "Your markup on this one will be lost if you don't save it." };
        }
    }
    function keepEditing() {
        banner = null;
        s.pendingOpen = "";
    }

    // ---- Output ------------------------------------------------------------------
    // The image as shown (crop and markup), at native resolution
    function flattenTo(file, done) {
        const size = Qt.size(shown.width, shown.height);
        const ok = cropView.grabToImage(r => done(r.saveToFile(file)), size);
        if (!ok)
            done(false);
    }

    function save() {
        if (busy !== "" || !ready)
            return;
        prepare();
        if (!s.dirty) {
            leave(() => s.saved(s.path));
            return;
        }
        busy = "save";
        flattenTo(s.path, ok => {
            if (!ok)
                return fail("save", "Couldn't save “" + baseName(s.path) + "”", "Check that its folder still exists and the disk isn't full, then try again. Or copy it to the clipboard so the markup isn't lost.");
            busy = "";
            leave(() => s.saved(s.path));
        });
    }

    // Next to the original: "<name> (edited).png", "<name> (edited 2).png", ...
    function saveCopy() {
        if (busy !== "" || !ready)
            return;
        prepare();
        busy = "saveCopy";
        const stem = s.path.replace(/\.[^./]+$/, "");
        job(["sh", "-c", "n=\"$1 (edited).png\"; i=2; while [ -e \"$n\" ]; do n=\"$1 (edited $i).png\"; i=$((i+1)); done; printf '%s' \"$n\"", "sh", stem], (code, out) => {
            if (code !== 0 || out === "")
                return fail("saveCopy", "Couldn't save a copy", "Check that the screenshot's folder still exists, then try again.");
            flattenTo(out, ok => {
                if (!ok)
                    return fail("saveCopy", "Couldn't save a copy", "Check that the folder isn't read-only and the disk isn't full, then try again.");
                busy = "";
                leave(() => s.saved(out));
            });
        });
    }

    function copy() {
        if (busy !== "" || !ready)
            return;
        prepare();
        busy = "copy";
        const send = f => job(["sh", "-c", "wl-copy -t image/png < \"$1\"", "sh", f], code => {
            if (code !== 0)
                return fail("copy", "Couldn't copy the screenshot", "Make sure wl-clipboard is installed, then try again.");
            busy = "";
            showToast("Copied to the clipboard");
        });
        if (!s.dirty) {
            send(s.path);
            return;
        }
        const tmp = runtime + "/irice-markup.png";
        flattenTo(tmp, ok => ok ? send(tmp) : fail("copy", "Couldn't copy the screenshot", "There wasn't room to prepare the image. Free some memory, then try again."));
    }

    function share() {
        if (busy !== "" || !ready)
            return;
        prepare();
        if (!s.dirty) {
            Panels.share([s.path], win.screen);
            return;
        }
        // The edited image, under the screenshot's name, for the recipient
        busy = "share";
        const dir = runtime + "/irice-share";
        const file = dir + "/" + baseName(s.path);
        job(["mkdir", "-p", dir], code => {
            if (code !== 0)
                return fail("share", "Couldn't prepare the image to share", "Try again, or save it first and share the file.");
            flattenTo(file, ok => {
                if (!ok)
                    return fail("share", "Couldn't prepare the image to share", "Try again, or save it first and share the file.");
                busy = "";
                Panels.share([file], win.screen);
            });
        });
    }

    // Immediate; the thumbnail turns into an Undo toast
    function remove() {
        if (busy !== "")
            return;
        commitText();
        draft = null;
        leave(() => s.deleteShot());
    }

    function runAgain() {
        const r = retry;
        banner = null;
        retry = "";
        const fn = ({ save: save, saveCopy: saveCopy, copy: copy, share: share })[r];
        if (fn)
            fn();
    }

    // Short shell jobs: done(exitCode, stdout)
    function job(cmd, done) {
        const p = jobComponent.createObject(win, { command: cmd, done: done });
        p.running = true;
    }
    Component {
        id: jobComponent
        Process {
            id: p
            property var done
            property string out: ""
            property bool streamed: false
            property int code: -1
            // Reports once both the exit and the end of stdout arrived
            function finish() {
                if (!streamed || code < 0)
                    return;
                const f = done;
                done = null;
                if (f)
                    f(code, out);
                destroy();
            }
            stdout: StdioCollector {
                onStreamFinished: {
                    p.out = text;
                    p.streamed = true;
                    p.finish();
                }
            }
            onExited: c => {
                p.code = c;
                p.finish();
            }
        }
    }

    // ---- Keyboard ----------------------------------------------------------------
    readonly property bool typing: editText !== null
    Shortcut { sequences: [StandardKey.Undo]; enabled: !win.typing && win.busy === ""; onActivated: win.s.undo() }
    Shortcut { sequences: ["Ctrl+Shift+Z", StandardKey.Redo]; enabled: !win.typing && win.busy === ""; onActivated: win.s.redo() }
    Shortcut { sequences: [StandardKey.Copy]; enabled: !win.typing; onActivated: win.copy() }
    Shortcut { sequences: [StandardKey.Save]; enabled: !win.typing; onActivated: win.save() }
    Shortcut { sequences: ["Ctrl+Shift+S"]; enabled: !win.typing; onActivated: win.saveCopy() }
    Repeater {
        model: win.tools
        Item {
            id: toolKey
            required property var modelData
            Shortcut { sequence: toolKey.modelData.key; enabled: !win.typing && win.busy === ""; onActivated: win.setTool(toolKey.modelData.id) }
        }
    }

    readonly property var tools: [
        { id: "crop", icon: "crop", label: "Crop", key: "C" },
        { id: "pen", icon: "pencil-simple", label: "Pen", key: "P" },
        { id: "highlighter", icon: "highlighter", label: "Highlighter", key: "H" },
        { id: "arrow", icon: "arrow-up-right", label: "Arrow", key: "A" },
        { id: "rect", icon: "square", label: "Rectangle", key: "R" },
        { id: "ellipse", icon: "circle", label: "Ellipse", key: "O" },
        { id: "text", icon: "text-t", label: "Text", key: "T" },
        { id: "pixelate", icon: "checkerboard", label: "Redact", key: "B" }
    ]

    // ---- Layout --------------------------------------------------------------------
    Item {
        id: body
        anchors.fill: parent
        opacity: win.progress
        scale: 0.97 + 0.03 * win.progress
        onScaleChanged: glass.updatePosition()
        focus: true

        Glass {
            id: glass
            anchors.fill: parent
            shadowMargin: win.m
            radius: Theme.radius.module
            bevel: 18
        }

        // Esc and Enter, wherever focus is inside the editor
        Keys.onPressed: e => {
                if (e.key === Qt.Key_Escape) {
                    e.accepted = true;
                    if (win.banner)
                        win.banner.kind === "prompt" ? win.keepEditing() : (win.banner = null);
                    else if (win.draft)
                        win.draft = null;
                    else if (win.s.tool === "crop")
                        win.cancelCrop();
                    else
                        win.requestClose();
                } else if ((e.key === Qt.Key_Return || e.key === Qt.Key_Enter) && win.s.tool === "crop") {
                    e.accepted = true;
                    win.applyCrop();
                }
        }

        // -- Top bar: close, tools, undo/redo
        Item {
            id: topBar
            x: win.m + win.pad
            y: win.m + win.pad
            width: win.canvasW
            height: win.bar

            Row {
                spacing: 8
                EditorButton {
                    icon: "x"
                    tooltip: "Close editor"
                    shortcut: "Esc"
                    onClicked: win.requestClose()
                }
                Rectangle { width: 1; height: 20; anchors.verticalCenter: parent.verticalCenter; color: Theme.color.separator }
                Row {
                    spacing: 4
                    Repeater {
                        model: win.tools
                        EditorButton {
                            required property var modelData
                            icon: modelData.icon
                            tooltip: modelData.label
                            shortcut: modelData.key
                            selected: win.s.tool === modelData.id
                            enabled: win.ready && win.busy === ""
                            onClicked: win.setTool(modelData.id)
                        }
                    }
                }
            }
            Row {
                anchors.right: parent.right
                spacing: 4
                EditorButton {
                    icon: "arrow-counter-clockwise"
                    tooltip: "Undo"
                    shortcut: "Ctrl+Z"
                    enabled: win.s.undoStack.length > 0 && win.busy === ""
                    onClicked: { win.commitText(); win.s.undo(); }
                }
                EditorButton {
                    icon: "arrow-clockwise"
                    tooltip: "Redo"
                    shortcut: "Ctrl+Shift+Z"
                    enabled: win.s.redoStack.length > 0 && win.busy === ""
                    onClicked: win.s.redo()
                }
            }
        }

        // -- Canvas
        Rectangle {
            id: canvas
            x: win.m + win.pad
            y: topBar.y + win.bar + win.pad
            width: win.canvasW
            height: win.canvasH
            radius: Theme.radius.module - win.pad
            color: Theme.color.fill
            clip: true

            // The image at display size, centred
            Item {
                id: view
                width: win.shown.width * win.fit
                height: win.shown.height * win.fit
                anchors.centerIn: parent
                visible: win.ready

                // What gets flattened: the shown area at native size, scaled for display
                Item {
                    id: cropView
                    width: win.shown.width
                    height: win.shown.height
                    scale: win.fit
                    transformOrigin: Item.TopLeft
                    clip: true

                    Item {
                        id: content
                        x: -win.shown.x
                        y: -win.shown.y
                        width: win.imgW
                        height: win.imgH

                        Image {
                            id: base
                            source: win.s.path ? win.fileUrl(win.s.path) + "?v=" + win.s.serial : ""
                            asynchronous: true
                            cache: false
                            smooth: true
                            mipmap: true // pixelate averages through the mip levels
                        }
                        Repeater {
                            model: win.s.items
                            Annotation {
                                required property var modelData
                                width: win.imgW
                                height: win.imgH
                                ann: modelData
                                source: base
                            }
                        }
                        Annotation {
                            width: win.imgW
                            height: win.imgH
                            ann: win.draft
                            source: base
                            visible: win.draft !== null
                        }

                        // Typing a label
                        TextInput {
                            id: textInput
                            visible: win.editText !== null
                            enabled: visible
                            x: win.editText?.x ?? 0
                            y: win.editText?.y ?? 0
                            width: Math.max(contentWidth + 2, (win.editText?.size ?? 20) * 4)
                            color: win.editText?.color ?? "white"
                            selectionColor: Theme.accentAlpha(0.4)
                            selectByMouse: true
                            font { family: Theme.font.family; pixelSize: win.editText?.size ?? 20; weight: Font.DemiBold }
                            onAccepted: win.commitText()
                            Keys.onEscapePressed: win.cancelText()
                            Rectangle {
                                anchors.fill: parent
                                anchors.margins: -4 / win.fit
                                color: "transparent"
                                border.width: 1 / win.fit
                                border.color: Theme.accent
                                radius: 4 / win.fit
                            }
                        }
                    }
                }

                // Crop frame (display coordinates)
                Item {
                    id: cropFrame
                    anchors.fill: parent
                    visible: win.s.tool === "crop"
                    readonly property rect r: Qt.rect(win.pendingCrop.x * win.fit, win.pendingCrop.y * win.fit, win.pendingCrop.width * win.fit, win.pendingCrop.height * win.fit)
                    readonly property color dim: Qt.rgba(0, 0, 0, 0.5)
                    Rectangle { x: 0; y: 0; width: parent.width; height: cropFrame.r.y; color: cropFrame.dim }
                    Rectangle { x: 0; y: cropFrame.r.y + cropFrame.r.height; width: parent.width; height: parent.height - y; color: cropFrame.dim }
                    Rectangle { x: 0; y: cropFrame.r.y; width: cropFrame.r.x; height: cropFrame.r.height; color: cropFrame.dim }
                    Rectangle { x: cropFrame.r.x + cropFrame.r.width; y: cropFrame.r.y; width: parent.width - x; height: cropFrame.r.height; color: cropFrame.dim }
                    Rectangle {
                        x: cropFrame.r.x
                        y: cropFrame.r.y
                        width: cropFrame.r.width
                        height: cropFrame.r.height
                        color: "transparent"
                        border.width: 1
                        border.color: "white"
                        // Thirds while adjusting
                        Repeater {
                            model: input.pressed ? 4 : 0
                            Rectangle {
                                required property int index
                                readonly property bool vertical: index < 2
                                x: vertical ? parent.width * (index + 1) / 3 : 0
                                y: vertical ? 0 : parent.height * (index - 1) / 3
                                width: vertical ? 1 : parent.width
                                height: vertical ? parent.height : 1
                                color: Qt.rgba(1, 1, 1, 0.5)
                            }
                        }
                    }
                    // Corner handles
                    Repeater {
                        model: 4
                        Rectangle {
                            required property int index
                            readonly property bool atRight: index % 2 === 1
                            readonly property bool atBottom: index > 1
                            x: (atRight ? cropFrame.r.x + cropFrame.r.width : cropFrame.r.x) - 6
                            y: (atBottom ? cropFrame.r.y + cropFrame.r.height : cropFrame.r.y) - 6
                            width: 12
                            height: 12
                            radius: 3
                            color: "white"
                            border.width: 1
                            border.color: Qt.rgba(0, 0, 0, 0.3)
                        }
                    }
                }

                // Drawing input, mapped to native pixels
                MouseArea {
                    id: input
                    anchors.fill: parent
                    anchors.margins: win.s.tool === "crop" ? -8 : 0 // reach the handles on the edge
                    enabled: win.ready && win.busy === "" && !win.leaving && !(win.banner?.kind === "prompt")
                    hoverEnabled: win.s.tool === "crop"
                    acceptedButtons: Qt.LeftButton
                    cursorShape: win.s.tool === "text" ? Qt.IBeamCursor : win.s.tool === "crop" ? cropCursor : Qt.CrossCursor
                    property int cropCursor: Qt.CrossCursor
                    property point start // native
                    property string grab: "" // crop: new | move | tl | tr | bl | br
                    property rect startCrop

                    function native(m) {
                        const off = win.s.tool === "crop" ? -8 : 0; // margins shift the origin
                        const x = (m.x + off) / win.fit + win.shown.x;
                        const y = (m.y + off) / win.fit + win.shown.y;
                        return Qt.point(Math.max(0, Math.min(win.imgW, x)), Math.max(0, Math.min(win.imgH, y)));
                    }
                    function cropHit(p) {
                        const r = win.pendingCrop, t = 12 / win.fit;
                        const near = (a, b) => Math.abs(a - b) <= t;
                        const l = near(p.x, r.x), rt = near(p.x, r.x + r.width), tp = near(p.y, r.y), bt = near(p.y, r.y + r.height);
                        if (l && tp) return "tl";
                        if (rt && tp) return "tr";
                        if (l && bt) return "bl";
                        if (rt && bt) return "br";
                        if (p.x > r.x && p.x < r.x + r.width && p.y > r.y && p.y < r.y + r.height) return "move";
                        return "new";
                    }
                    // Shift: squares, circles, 45° arrows
                    function constrain(a, b, mods, angle) {
                        if (!(mods & Qt.ShiftModifier))
                            return b;
                        const dx = b.x - a.x, dy = b.y - a.y;
                        if (angle) {
                            const ang = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4);
                            const l = Math.hypot(dx, dy);
                            return Qt.point(a.x + Math.cos(ang) * l, a.y + Math.sin(ang) * l);
                        }
                        const d = Math.max(Math.abs(dx), Math.abs(dy));
                        return Qt.point(a.x + Math.sign(dx || 1) * d, a.y + Math.sign(dy || 1) * d);
                    }
                    function boxFrom(a, b) {
                        return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(b.x - a.x), h: Math.abs(b.y - a.y) };
                    }

                    onPressed: m => {
                        const p = native(m);
                        start = p;
                        const t = win.s.tool;
                        win.banner = win.banner?.kind === "error" ? win.banner : null;
                        if (t === "crop") {
                            grab = cropHit(p);
                            startCrop = win.pendingCrop;
                            if (grab === "new")
                                win.pendingCrop = Qt.rect(p.x, p.y, 0, 0);
                            return;
                        }
                        if (t === "text") {
                            win.commitText();
                            const size = win.textSizes[win.s.widthIndex];
                            win.editText = { x: p.x, y: p.y - size * 0.6, color: win.ink.toString(), size: size };
                            textInput.text = "";
                            Qt.callLater(() => textInput.forceActiveFocus());
                            return;
                        }
                        win.commitText();
                        const c = win.ink.toString();
                        if (t === "pen")
                            win.draft = { type: "pen", color: c, width: win.penWidths[win.s.widthIndex], points: [p] };
                        else if (t === "highlighter")
                            win.draft = { type: "highlighter", color: c, width: win.penWidths[win.s.widthIndex] * 4, points: [p] };
                        else
                            win.draft = null; // shapes appear once the pointer moves
                    }
                    onPositionChanged: m => {
                        const t = win.s.tool;
                        if (t === "crop" && !pressed) {
                            const h = cropHit(native(m));
                            cropCursor = h === "tl" || h === "br" ? Qt.SizeFDiagCursor : h === "tr" || h === "bl" ? Qt.SizeBDiagCursor : h === "move" ? Qt.SizeAllCursor : Qt.CrossCursor;
                            return;
                        }
                        if (!pressed)
                            return;
                        const p = native(m);
                        if (t === "crop") {
                            const r = startCrop, W = win.imgW, H = win.imgH;
                            let x0 = r.x, y0 = r.y, x1 = r.x + r.width, y1 = r.y + r.height;
                            if (grab === "new") {
                                const b = boxFrom(start, p);
                                win.pendingCrop = Qt.rect(b.x, b.y, b.w, b.h);
                                return;
                            }
                            if (grab === "move") {
                                const dx = Math.max(-x0, Math.min(W - x1, p.x - start.x));
                                const dy = Math.max(-y0, Math.min(H - y1, p.y - start.y));
                                win.pendingCrop = Qt.rect(x0 + dx, y0 + dy, r.width, r.height);
                                return;
                            }
                            if (grab[0] === "t") y0 = p.y; else y1 = p.y;
                            if (grab[1] === "l") x0 = p.x; else x1 = p.x;
                            win.pendingCrop = Qt.rect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
                            return;
                        }
                        if (t === "pen" || t === "highlighter") {
                            if (!win.draft)
                                return;
                            const pts = win.draft.points;
                            const last = pts[pts.length - 1];
                            if (Math.hypot(p.x - last.x, p.y - last.y) < 1.5 / win.fit)
                                return;
                            win.draft = Object.assign({}, win.draft, { points: pts.concat([p]) });
                            return;
                        }
                        const c = win.ink.toString();
                        if (t === "arrow") {
                            const e = constrain(start, p, m.modifiers, true);
                            win.draft = { type: "arrow", x1: start.x, y1: start.y, x2: e.x, y2: e.y, color: c, width: win.penWidths[win.s.widthIndex] };
                        } else if (t === "rect" || t === "ellipse") {
                            const b = boxFrom(start, constrain(start, p, m.modifiers, false));
                            win.draft = Object.assign({ type: t, color: c, width: win.penWidths[win.s.widthIndex] }, b);
                        } else if (t === "pixelate") {
                            const b = boxFrom(start, p);
                            win.draft = Object.assign({ type: "pixelate", block: Math.max(2, Math.round(win.blockSizes[win.s.widthIndex])) }, b);
                        }
                    }
                    onReleased: {
                        const t = win.s.tool;
                        if (t === "crop") {
                            if (grab === "new" && (win.pendingCrop.width < 4 / win.fit || win.pendingCrop.height < 4 / win.fit))
                                win.pendingCrop = startCrop; // a click, not a drag
                            grab = "";
                            return;
                        }
                        const d = win.draft;
                        win.draft = null;
                        if (!d)
                            return;
                        const tiny = 4 / win.fit;
                        if (d.type === "pen" || d.type === "highlighter") {
                            // A click leaves a dot
                            const pts = d.points.length === 1 ? [d.points[0], Qt.point(d.points[0].x + 0.01, d.points[0].y)] : d.points;
                            win.s.add(Object.assign({}, d, { points: pts }));
                        } else if (d.type === "arrow") {
                            if (Math.hypot(d.x2 - d.x1, d.y2 - d.y1) >= 2 * tiny)
                                win.s.add(d);
                        } else if (d.w >= tiny && d.h >= tiny) {
                            win.s.add(d);
                        }
                    }
                    onCanceled: {
                        win.draft = null;
                        grab = "";
                    }
                }
            }

            // Loading
            Column {
                anchors.centerIn: parent
                visible: base.status === Image.Loading || (base.status === Image.Null && win.s.path !== "")
                spacing: 8
                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: "circle-notch"
                    size: 24
                    color: Theme.color.tertiary
                    RotationAnimator on rotation { running: parent.visible; from: 0; to: 360; duration: 900; loops: Animation.Infinite }
                }
                Text {
                    text: "Opening screenshot…"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
            }

            // The file couldn't be read
            Column {
                anchors.centerIn: parent
                visible: base.status === Image.Error
                width: Math.min(360, parent.width - 48)
                spacing: 8
                Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "image"; size: 32; color: Theme.color.tertiary }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: "Couldn't open this screenshot"
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.title; weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    text: "It may have been moved or deleted. Take it again, or open it from the Screenshots folder."
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.small }
                }
                Item { width: 1; height: 8 }
                EditorButton {
                    anchors.horizontalCenter: parent.horizontalCenter
                    variant: "secondary"
                    label: "Close editor"
                    onClicked: win.leave(() => win.s.closeEditor())
                }
            }

            // Banner: a failed action, or unsaved changes
            Rectangle {
                id: bannerBox
                visible: win.banner !== null
                readonly property bool error: win.banner?.kind === "error"
                anchors { left: parent.left; right: parent.right; top: parent.top; margins: 12 }
                height: bannerRow.implicitHeight + 24
                radius: Theme.radius.menu
                color: error ? (Theme.dark ? "#3A1F1E" : "#FDECEA") : (Theme.dark ? "#2C2C31" : "#FFFFFF")
                border.width: 1
                border.color: error ? Qt.alpha(Theme.color.danger, 0.4) : Theme.color.separator
                MouseArea { anchors.fill: parent } // keep clicks off the canvas

                Row {
                    id: bannerRow
                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 24
                    spacing: 12
                    Icon {
                        name: bannerBox.error ? "warning-circle" : "floppy-disk"
                        size: 20
                        color: bannerBox.error ? Theme.color.danger : Theme.color.secondary
                    }
                    Column {
                        width: parent.width - 32 - bannerActions.width - 12
                        spacing: 2
                        anchors.verticalCenter: parent.verticalCenter
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: win.banner?.title ?? ""
                            color: Theme.color.label
                            font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.DemiBold }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: win.banner?.body ?? ""
                            color: Theme.color.secondary
                            font { family: Theme.font.family; pixelSize: Theme.font.small }
                        }
                    }
                    Row {
                        id: bannerActions
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 8
                        // Error
                        EditorButton {
                            visible: bannerBox.error && win.retry !== "copy"
                            variant: "secondary"
                            label: "Copy to clipboard"
                            onClicked: { win.banner = null; win.copy(); }
                        }
                        EditorButton {
                            visible: bannerBox.error
                            variant: "secondary"
                            label: "Try again"
                            onClicked: win.runAgain()
                        }
                        EditorButton {
                            visible: bannerBox.error
                            icon: "x"
                            tooltip: "Dismiss"
                            onClicked: win.banner = null
                        }
                        // Unsaved changes
                        EditorButton {
                            visible: !bannerBox.error
                            label: "Keep editing"
                            onClicked: win.keepEditing()
                        }
                        EditorButton {
                            visible: !bannerBox.error
                            variant: "secondary"
                            label: "Discard changes"
                            onClicked: { win.banner = null; win.leave(() => win.s.closeEditor()); }
                        }
                        EditorButton {
                            visible: !bannerBox.error
                            variant: "primary"
                            label: "Save"
                            busy: win.busy === "save"
                            onClicked: win.save()
                        }
                    }
                }
            }

            // Confirmation toast
            Rectangle {
                anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 16 }
                width: toastLabel.implicitWidth + 32
                height: 32
                radius: 16
                color: Theme.dark ? "#3A3A3F" : "#1C1C1E"
                opacity: win.toast !== "" ? 1 : 0
                visible: opacity > 0
                Behavior on opacity { NumberAnimation { duration: win.toast !== "" ? 200 : 140 } }
                Row {
                    id: toastLabel
                    anchors.centerIn: parent
                    spacing: 8
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: "check"; bold: true; size: 14; color: "#F2F2F5" }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: win.toast
                        color: "#F2F2F5"
                        font { family: Theme.font.family; pixelSize: Theme.font.body; weight: Font.Medium }
                    }
                }
            }
        }

        // -- Bottom bar: tool options on the left, actions on the right
        Item {
            id: bottomBar
            x: win.m + win.pad
            y: canvas.y + win.canvasH + win.pad
            width: win.canvasW
            height: win.bar

            // Crop: size readout, Reset, Apply
            Row {
                visible: win.s.tool === "crop"
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 112
                    text: Math.round(win.pendingCrop.width) + " × " + Math.round(win.pendingCrop.height)
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body; features: { "tnum": 1 } }
                }
                EditorButton {
                    label: "Reset"
                    tipBelow: false
                    tooltip: "Use the whole image"
                    enabled: win.pendingCrop.width < win.imgW || win.pendingCrop.height < win.imgH
                    onClicked: win.pendingCrop = win.full
                }
                EditorButton {
                    variant: "secondary"
                    icon: "check"
                    label: "Apply crop"
                    tipBelow: false
                    shortcut: "Enter"
                    tooltip: "Apply"
                    onClicked: win.applyCrop()
                }
            }

            // Colour and width
            Row {
                visible: win.s.tool !== "crop"
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12
                opacity: win.ready ? 1 : 0.5
                enabled: win.ready && win.busy === ""

                Row {
                    visible: win.s.tool !== "pixelate"
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2
                    Repeater {
                        model: win.s.palette
                        Item {
                            id: swatch
                            required property string modelData
                            required property int index
                            readonly property bool current: win.s.colorIndex === index
                            width: 26
                            height: 36
                            Rectangle {
                                anchors.centerIn: parent
                                width: 24
                                height: 24
                                radius: 12
                                color: "transparent"
                                border.width: 2
                                border.color: swatch.current ? Theme.color.label : swatchMouse.containsMouse ? Theme.color.separator : "transparent"
                            }
                            Rectangle {
                                anchors.centerIn: parent
                                width: 16
                                height: 16
                                radius: 8
                                color: swatch.modelData
                                scale: swatchMouse.pressed ? 0.9 : 1
                                Behavior on scale { ScaleAnimator { duration: Theme.motion.press } }
                            }
                            MouseArea {
                                id: swatchMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    win.s.colorIndex = swatch.index;
                                    // Recolour the label being typed
                                    if (win.editText)
                                        win.editText = Object.assign({}, win.editText, { color: swatch.modelData });
                                }
                            }
                        }
                    }
                }
                Text {
                    visible: win.s.tool === "pixelate"
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Block size"
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
                Rectangle {
                    visible: win.s.tool !== "pixelate"
                    anchors.verticalCenter: parent.verticalCenter
                    width: 1
                    height: 20
                    color: Theme.color.separator
                }
                // Three widths (for text: sizes; for redact: block sizes)
                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2
                    Repeater {
                        model: 3
                        EditorButton {
                            id: widthButton
                            required property int index
                            selected: win.s.widthIndex === index
                            tipBelow: false
                            tooltip: win.s.tool === "text" ? ["Small text", "Medium text", "Large text"][index] : win.s.tool === "pixelate" ? ["Fine blocks", "Medium blocks", "Coarse blocks"][index] : ["Thin", "Medium", "Thick"][index]
                            onClicked: {
                                win.s.widthIndex = index;
                                if (win.editText) {
                                    win.editText = Object.assign({}, win.editText, { size: win.textSizes[index] });
                                }
                            }
                            // Text: A in three sizes; redact: a block; else a dot
                            Text {
                                visible: win.s.tool === "text"
                                anchors.centerIn: parent
                                text: "A"
                                color: widthButton.fg
                                font { family: Theme.font.family; pixelSize: [11, 14, 18][widthButton.index]; weight: Font.DemiBold }
                            }
                            Rectangle {
                                visible: win.s.tool !== "text"
                                anchors.centerIn: parent
                                readonly property int d: [4, 7, 11][widthButton.index]
                                width: d
                                height: d
                                radius: win.s.tool === "pixelate" ? 1 : d / 2
                                color: win.s.tool === "pixelate" ? widthButton.fg : (widthButton.selected ? win.ink : widthButton.fg)
                            }
                        }
                    }
                }
            }

            // Actions
            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                EditorButton {
                    icon: "trash"
                    variant: "danger"
                    tipBelow: false
                    tooltip: "Delete screenshot"
                    enabled: win.busy === "" && win.s.path !== ""
                    onClicked: win.remove()
                }
                EditorButton {
                    icon: "export"
                    label: "Share"
                    variant: "secondary"
                    tipBelow: false
                    tooltip: "Send to a nearby device"
                    enabled: win.ready && (win.busy === "" || win.busy === "share")
                    busy: win.busy === "share"
                    onClicked: win.share()
                }
                EditorButton {
                    icon: "copy"
                    label: "Copy"
                    variant: "secondary"
                    tipBelow: false
                    shortcut: "Ctrl+C"
                    tooltip: "Copy image"
                    enabled: win.ready && (win.busy === "" || win.busy === "copy")
                    busy: win.busy === "copy"
                    onClicked: win.copy()
                }
                // Save, with Save as copy beside it
                Row {
                    spacing: 2
                    EditorButton {
                        variant: "primary"
                        label: win.s.dirty ? "Save" : "Done"
                        tipBelow: false
                        tooltip: win.s.dirty ? "Save over the original" : "Close the editor"
                        shortcut: win.s.dirty ? "Ctrl+S" : ""
                        enabled: win.ready && (win.busy === "" || win.busy === "save")
                        busy: win.busy === "save"
                        onClicked: win.save()
                    }
                    EditorButton {
                        variant: "secondary"
                        icon: "files"
                        tipBelow: false
                        tooltip: "Save as a copy"
                        shortcut: "Ctrl+Shift+S"
                        enabled: win.ready && (win.busy === "" || win.busy === "saveCopy")
                        busy: win.busy === "saveCopy"
                        onClicked: win.saveCopy()
                    }
                }
            }
        }
    }
}
