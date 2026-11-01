import QtQuick
import qs.common

// Control Center → Microphone (the Microphone slider's page): its level and
// the inputs (MicSection).
DetailPage {
    id: root
    title: "Microphone"
    MicSection {
        width: parent.width
    }
}
