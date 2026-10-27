pragma Singleton
import QtQuick
import Quickshell

// Which of iRice's native helpers (shell/native) this shell can use. Built
// with shell/native/build.sh and put on the import path by launch-shell.sh;
// without them everything falls back to QML.
Singleton {
    id: root

    // IRice.Native's GlassSync: glass kept in step from C++ (glass/GlassWindow.qml)
    property bool glassSync: false
    Component.onCompleted: {
        try {
            const probe = Qt.createQmlObject("import IRice.Native\nGlassSync {}", root, "native-probe");
            probe.destroy();
            glassSync = true;
        } catch (e) {
            glassSync = false;
        }
        console.info("Glass sync: " + (glassSync ? "native (IRice.Native)" : "QML (build shell/native for the faster native one)"));
    }
}
