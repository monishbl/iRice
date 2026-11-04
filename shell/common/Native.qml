pragma Singleton
import QtQuick
import Quickshell

// Which of Tessera's native helpers (shell/native) this shell can use. Built
// with shell/native/build.sh and put on the import path by launch-shell.sh;
// without them everything falls back to QML.
Singleton {
    id: root

    // Tessera.Native's GlassSync: glass kept in step from C++ (glass/GlassWindow.qml)
    property bool glassSync: false
    // Which module: Tessera.Native, or IRice.Native when it was built before
    // Tessera was called iRice (until shell/native/build.sh runs again)
    property string module: ""
    Component.onCompleted: {
        for (const m of ["Tessera.Native", "IRice.Native"]) {
            try {
                const probe = Qt.createQmlObject("import " + m + "\nGlassSync {}", root, "native-probe");
                probe.destroy();
                module = m;
                break;
            } catch (e) {}
        }
        glassSync = module !== "";
        console.info("Glass sync: " + (glassSync ? "native (" + module + ")" : "QML (build shell/native for the faster native one)"));
    }
}
