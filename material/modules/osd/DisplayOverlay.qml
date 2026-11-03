import QtQuick
import QtQuick as QQ
import Quickshell
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.modules.settings
import qs.services

// Settings → Displays on every screen: "Identify" shows each screen's number
// and name in its corner, and after a change that could leave a screen dark,
// every screen asks "Keep these display settings?" with the countdown, so
// it can be answered from whichever screen still shows a picture.
// The Material look shows its own card and dialog (MaterialDisplayIdentify,
// MaterialDisplayConfirm).
Scope {
    Variants {
        model: Displays.identifying || Displays.confirming ? Quickshell.screens.filter(s => !/^(HEADLESS|IRICE-)/.test(s.name)) : []

        Scope {
            id: perScreen
            required property var modelData



            LazyLoader {
                active: Displays.identifying && Theme.material

                MaterialDisplayIdentify {
                    target: perScreen.modelData
                }
            }
            LazyLoader {
                active: Displays.confirming && Theme.material

                MaterialDisplayConfirm {
                    target: perScreen.modelData
                }
            }
        }
    }
}
