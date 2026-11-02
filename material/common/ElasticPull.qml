import QtQuick

// The Material look has no elastic pull (presses change shape instead): the
// same interface as the Glass one (common/ElasticPull.qml), doing nothing.
// (Not Glass's with a pull of 0: its rubber band divides by the pull, and
// 0 / 0 left a pressed panel's position NaN, so it never drew again.)
//   body.transform: [pull.translate, pull.stretch]
QtObject {
    id: root

    property Item target
    property bool active: false
    property point from: Qt.point(0, 0)
    property point pointer: Qt.point(0, 0)
    property real strength: 0
    readonly property point lean: Qt.point(0, 0)
    readonly property real lx: 0
    readonly property real ly: 0
    readonly property Translate translate: Translate {}
    readonly property Scale stretch: Scale {}
}
