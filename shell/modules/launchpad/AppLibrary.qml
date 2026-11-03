import QtQuick
import Quickshell

// Every app (Super+Shift+Space, or the Apps button in the dock), in each
// look's own drawing, each made only in its look: Glass's App Library
// (AppLibraryGlass) and Material's app drawer (AppDrawer).
Scope {
    AppLibraryGlass {}
    AppDrawer {}
}
