import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.SystemTray
import Quickshell.Services.UPower
import Quickshell.Wayland
import qs.common
import qs.glass
import qs.modules.resources
import qs.services

// Tahoe-style menu bar: transparent over the wallpaper, text that flips dark
// over light wallpapers. Left: system menu, active app (and window title),
// Window menu. Centre: Spaces. Right: extra buttons, tray, Now Playing,
// status items with their menus, sidebar, Spotlight, Control Center, clock.
// Scroll the left half for brightness, the right half for volume.
Variants {
    model: Quickshell.screens.filter(s => Config.options.menubar.screens.length === 0 || Config.options.menubar.screens.includes(s.name))

    GlassWindow {
        id: bar
        required property var modelData
        screen: modelData
        name: "menubar"
        visible: Wallpaper.started // after the wallpaper at login

        readonly property var cfg: Config.options.menubar
        readonly property bool autoHide: cfg.autoHide
        // Auto-hide also reveals the bar while the island has something to say
        readonly property bool revealed: !autoHide || reveal.hovered || (Panels.open !== "" && Panels.screen === screen) || (islandOn && Island.event !== null)

        WlrLayershell.layer: autoHide ? WlrLayer.Overlay : WlrLayer.Top
        anchors { top: true; left: true; right: true }
        // Taller than the bar so the dynamic island can open below it; only the
        // 30px bar is reserved, and input is masked to the bar and the island
        readonly property bool islandOn: Config.options.island.enabled
        implicitHeight: islandOn ? 340 : 30
        exclusionMode: autoHide ? ExclusionMode.Ignore : ExclusionMode.Normal
        exclusiveZone: autoHide ? 0 : 30
        // Auto-hide: only a 2px strip catches the pointer while hidden
        mask: Region {
            Region { item: bar.revealed ? fullArea : revealStrip }
            Region {
                x: island.shape.x
                y: island.shape.y + content.y
                width: bar.islandOn && bar.revealed ? island.shape.width : 0
                height: island.shape.height
            }
        }
        Item { id: fullArea; width: parent.width; height: 30 }
        Item { id: revealStrip; width: parent.width; height: 2 }
        HoverHandler { id: reveal }
        // Pointer for the island. On the window root, like `reveal`: a
        // HoverHandler on an item laid over the bar swallows the bar items'
        // hover (no chips, stale highlights)
        HoverHandler { id: tracker }
        Item {
            anchors.fill: parent
            z: 100
            // Touchscreen edge swipe: pull down from the bar, as on iPad.
            // Right third opens Control Center, elsewhere Notification Center.
            DragHandler {
                target: null
                acceptedDevices: PointerDevice.TouchScreen
                yAxis.minimum: 0
                xAxis.enabled: false
                property bool fired: false
                onActiveChanged: fired = false
                onTranslationChanged: {
                    if (fired || translation.y < 48)
                        return;
                    fired = true;
                    Panels.show(centroid.pressPosition.x > bar.width * 2 / 3 ? "controlCenter" : "notificationCenter", bar.screen);
                }
            }
        }

        readonly property bool opaque: cfg.background || autoHide || Theme.glass.tier === "solid"
        readonly property color fg: opaque ? Theme.color.label : Wallpaper.topIsLight ? Qt.rgba(0, 0, 0, 0.85) : Qt.rgba(1, 1, 1, 0.96)

        IdleInhibitor {
            window: bar
            enabled: System.keepAwake
        }
        Binding {
            target: Resources
            property: "menubarActive"
            value: bar.cfg.resources
        }

        Item {
            id: content
            width: parent.width
            height: 30
            y: bar.revealed ? 0 : -height
            opacity: bar.revealed ? 1 : 0
            Behavior on y { NumberAnimation { duration: Theme.motion.popover; easing.type: Easing.BezierSpline; easing.bezierCurve: Theme.motion.emphasized } }
            Behavior on opacity { NumberAnimation { duration: Theme.motion.popover } }

            Rectangle {
                anchors.fill: parent
                visible: bar.opaque
                color: Theme.glass.tier === "solid" ? Theme.glass.solid : Qt.alpha(Theme.glass.solid, bar.autoHide ? 0.92 : 0.6)
            }

            // Top rounded screen corners (bottom ones: ScreenCorners)
            Corner { visible: Config.options.appearance.screenCorners; x: 0; radius: Config.options.appearance.screenCornerRadius }
            Corner { visible: Config.options.appearance.screenCorners; x: bar.width - width; atRight: true; radius: Config.options.appearance.screenCornerRadius }

            // Scroll gestures: left half brightness, right half volume
            WheelHandler {
                enabled: bar.cfg.scrollGestures
                onWheel: e => {
                    const step = e.angleDelta.y > 0 ? 0.05 : -0.05;
                    if (point.position.x < bar.width / 2)
                        Brightness.set(Brightness.value + step);
                    else
                        Audio.setVolume(Audio.volume + step);
                }
            }

            // ── Left ────────────────────────────────────────────────────
            Row {
                id: leftItems
                anchors { left: parent.left; leftMargin: 8; verticalCenter: parent.verticalCenter }
                spacing: 0

                // The system menu: iRice's drop, your distro's logo, or a
                // brand mark (Settings → Appearance → Menu icon)
                MenuBarItem {
                    readonly property string choice: Config.options.appearance.menuIcon
                    readonly property string distroIcon: choice === "distro" && SystemInfo.osLogo !== "" ? Quickshell.iconPath(SystemInfo.osLogo, true) : ""
                    icon: distroIcon !== "" ? "" : ({ irice: "drop", linux: "linux-logo", windows: "windows-logo", apple: "apple-logo", android: "android-logo", distro: "linux-logo", command: "command", terminal: "terminal-window", cpu: "cpu", atom: "atom", planet: "planet", rocket: "rocket-launch", star: "star-four", lightning: "lightning", flame: "flame", leaf: "leaf", snowflake: "snowflake", moon: "moon-stars", cat: "cat", ghost: "ghost", alien: "alien", steam: "steam-logo", github: "github-logo", circle: "circle-wavy" })[choice] ?? "drop"
                    image: distroIcon
                    tintImage: true
                    fill: true
                    iconSize: 16
                    color: bar.fg
                    active: Panels.isOpen("systemMenu", bar.screen)
                    menu: "systemMenu"
                    onClicked: Panels.toggle("systemMenu", bar.screen, mapToItem(null, 0, 0).x)
                }
                MenuBarItem {
                    visible: bar.cfg.appMenu
                    text: Apps.activeName
                    bold: true
                    color: bar.fg
                    active: Panels.isOpen("appMenu", bar.screen)
                    menu: "appMenu"
                    onClicked: Panels.toggle("appMenu", bar.screen, mapToItem(null, 0, 0).x)
                }
                MenuBarItem {
                    visible: bar.cfg.windowMenu
                    text: "Window"
                    color: bar.fg
                    active: Panels.isOpen("windowMenu", bar.screen)
                    menu: "windowMenu"
                    onClicked: Panels.toggle("windowMenu", bar.screen, mapToItem(null, 0, 0).x)
                }
                Text {
                    visible: bar.cfg.windowTitle && text !== ""
                    anchors.verticalCenter: parent.verticalCenter
                    leftPadding: 6
                    width: Math.min(implicitWidth, bar.width / 2 - leftItems.x - 360)
                    elide: Text.ElideRight
                    text: Apps.active?.title ?? ""
                    color: Qt.alpha(bar.fg, 0.65)
                    font { family: Theme.font.family; pixelSize: Theme.font.body }
                }
            }

            // ── Centre: Spaces ──────────────────────────────────────────
            Workspaces {
                visible: bar.cfg.workspaces !== "off" && !Config.options.island.enabled // the island shows them instead
                anchors.centerIn: parent
                screen: bar.screen
                fg: bar.fg
            }

            // ── Centre: the dynamic island (it shows the spaces) ────────
            IslandView {
                id: island
                visible: bar.islandOn
                width: parent.width
                height: 340
                screen: bar.screen
                fg: bar.fg
                pointer: tracker.hovered ? Qt.point(tracker.point.position.x, tracker.point.position.y - content.y) : Qt.point(-1, -1)
            }

            // ── Right ───────────────────────────────────────────────────
            Row {
                anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                spacing: 0

                // Extra buttons (Settings → Menu Bar)
                Repeater {
                    model: bar.cfg.utilButtons.filter(b => b !== "colorPicker" || Tools.has("hyprpicker"))
                    MenuBarItem {
                        required property string modelData
                        readonly property var def: ({
                                screenshot: { icon: "camera", act: () => Panels.toggle("screenTools", bar.screen) },
                                colorPicker: { icon: "eyedropper", act: () => System.run(["hyprpicker", "-a"]) },
                                mic: { icon: Audio.micMuted ? "microphone-slash" : "microphone", act: () => Audio.toggleMic() },
                                keyboard: { icon: "keyboard", act: () => Panels.toggle("keyboard", bar.screen) },
                                darkMode: { icon: "circle-half", act: () => System.toggleDarkMode() },
                                powerProfile: { icon: PowerProfiles.profile === PowerProfile.PowerSaver ? "leaf" : PowerProfiles.profile === PowerProfile.Performance ? "lightning" : "scales", act: () => Power.setMode(PowerProfiles.profile === PowerProfile.PowerSaver ? "balanced" : PowerProfiles.profile === PowerProfile.Balanced && PowerProfiles.hasPerformanceProfile ? "performance" : "power-saver") },
                                record: { icon: "record", act: () => ScreenTools.recording ? ScreenTools.stopRecording() : ScreenTools.select("record") }
                            })[modelData]
                        visible: !!def
                        icon: def?.icon ?? ""
                        color: bar.fg
                        onClicked: def?.act()
                        HoverTip {
                            target: parent
                            hovered: parent.hovered && !parent.active
                            screen: bar.screen
                            text: ({
                                    screenshot: "Screenshot",
                                    colorPicker: "Pick a colour",
                                    mic: Audio.micMuted ? "Microphone off" : "Microphone on",
                                    keyboard: "On-screen keyboard",
                                    darkMode: Theme.dark ? "Dark appearance" : "Light appearance",
                                    powerProfile: "Energy mode: " + (PowerProfiles.profile === PowerProfile.PowerSaver ? "Low Power" : PowerProfiles.profile === PowerProfile.Performance ? "Performance" : "Balanced"),
                                    record: "Record the screen"
                                })[modelData] ?? ""
                        }
                    }
                }

                // Recording in progress: click to stop
                MenuBarItem {
                    visible: ScreenTools.recording
                    icon: "record"
                    fill: true
                    color: Theme.color.danger
                    property real now: Date.now()
                    readonly property int elapsed: Math.max(0, Math.floor((now - ScreenTools.recordingStarted) / 1000))
                    text: Math.floor(elapsed / 60) + ":" + String(elapsed % 60).padStart(2, "0")
                    onClicked: ScreenTools.stopRecording()
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: "Recording · " + parent.text + " · click to stop"
                    }
                    Timer { running: ScreenTools.recording; interval: 1000; repeat: true; triggeredOnStart: true; onTriggered: parent.now = Date.now() }
                }

                // Caffeine on: a cup; click for its options (or to turn it off)
                MenuBarItem {
                    visible: System.keepAwake
                    icon: "coffee"
                    fill: true
                    color: bar.fg
                    onClicked: {
                        Panels.show("controlCenter", bar.screen);
                        Panels.ccPage = "caffeine";
                    }
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: "Keeping awake · " + System.keepAwakeStatus.toLowerCase()
                    }
                }

                Repeater {
                    model: SystemTray.items
                    MenuBarItem {
                        required property var modelData
                        image: IconLookup.safe(modelData.icon, "application-x-executable")
                        color: bar.fg
                        active: Panels.isOpen("trayMenu", bar.screen) && Panels.trayMenu === modelData.menu
                        onClicked: m => {
                            if (m.button === Qt.LeftButton && !modelData.onlyMenu) {
                                modelData.activate();
                            } else if (modelData.hasMenu) {
                                Panels.trayMenu = modelData.menu;
                                Panels.toggle("trayMenu", bar.screen, mapToItem(null, 0, 0).x);
                            }
                        }
                        HoverTip {
                            target: parent
                            hovered: parent.hovered && !parent.active
                            screen: bar.screen
                            text: modelData.tooltipTitle || modelData.title || modelData.id
                        }
                    }
                }

                // Now Playing (the dynamic island shows music itself when it's on)
                MenuBarItem {
                    visible: !bar.islandOn && Media.player !== null && (Media.player.trackTitle ?? "") !== ""
                    icon: Media.player?.isPlaying ? "waveform" : "music-note"
                    fill: true
                    color: bar.fg
                    text: {
                        const t = Media.player?.trackTitle ?? "";
                        return t.length > 28 ? t.slice(0, 27) + "…" : t;
                    }
                    active: Panels.isOpen("nowPlaying", bar.screen)
                    menu: "nowPlaying"
                    onClicked: Panels.toggle("nowPlaying", bar.screen, mapToItem(null, 0, 0).x)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: (Media.player?.trackTitle ?? "") + ((Media.player?.trackArtist ?? "") !== "" ? " · " + Media.player.trackArtist : "")
                    }
                }

                // Resources
                MenuBarItem {
                    visible: bar.cfg.resources
                    icon: "cpu"
                    color: Resources.cpu > 0.9 || Resources.memory > 0.95 ? Theme.color.danger : bar.fg
                    text: Math.round(Resources.cpu * 100) + "% · " + Math.round(Resources.memory * 100) + "%"
                    active: Panels.isOpen("resourcesMenu", bar.screen)
                    menu: "resourcesMenu"
                    onClicked: Panels.toggle("resourcesMenu", bar.screen, mapToItem(null, 0, 0).x)
                    // Hovering it shows the last minute (not while its menu is open)
                    ResourceHover {
                        target: parent
                        key: "cpu"
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                    }
                }

                // Weather
                MenuBarItem {
                    visible: bar.cfg.weather && Weather.ready
                    icon: Weather.icon
                    fill: true
                    color: bar.fg
                    text: Weather.temp + "°"
                    active: Panels.isOpen("weatherMenu", bar.screen)
                    menu: "weatherMenu"
                    onClicked: Panels.toggle("weatherMenu", bar.screen, mapToItem(null, 0, 0).x)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: Weather.condition + " · H " + Weather.high + "° L " + Weather.low + "°" + (Weather.place !== "" ? " · " + Weather.place : "")
                    }
                }

                // Keyboard layout (only with two or more layouts)
                MenuBarItem {
                    visible: bar.cfg.keyboardLayout && KeyboardLayout.multiple
                    text: KeyboardLayout.short
                    color: bar.fg
                    onClicked: KeyboardLayout.next()
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: (KeyboardLayout.name || KeyboardLayout.short) + " · click to switch"
                    }
                }

                // Battery
                MenuBarItem {
                    id: batteryItem
                    readonly property var battery: UPower.displayDevice
                    readonly property real level: battery.percentage > 1 ? battery.percentage / 100 : battery.percentage
                    readonly property bool charging: battery.state === UPowerDeviceState.Charging || battery.state === UPowerDeviceState.FullyCharged
                    visible: bar.cfg.battery && battery.isLaptopBattery
                    readonly property bool plugged: battery.state === UPowerDeviceState.FullyCharged || battery.state === UPowerDeviceState.PendingCharge
                    // Settings → Menu Bar → Battery percentage (older configs saved true/false)
                    readonly property string percent: ({ "true": "outside", "false": "off" })[bar.cfg.batteryPercent] ?? bar.cfg.batteryPercent
                    color: bar.fg
                    // Beside the battery when asked, and always at 20% or less
                    // (unless it's already inside)
                    text: percent === "outside" || (percent !== "inside" && level <= 0.2 && !charging) ? Math.round(level * 100) + "%" : ""
                    glyph: Component {
                        BatteryGlyph {
                            level: batteryItem.level
                            charging: batteryItem.battery.state === UPowerDeviceState.Charging
                            plugged: batteryItem.plugged
                            lowPower: PowerProfiles.profile === PowerProfile.PowerSaver
                            showPercent: batteryItem.percent === "inside"
                            color: bar.fg
                        }
                    }
                    active: Panels.isOpen("batteryMenu", bar.screen)
                    menu: "batteryMenu"
                    onClicked: Panels.toggle("batteryMenu", bar.screen, mapToItem(null, 0, 0).x)
                    // Pointing at it: the level, what it's doing, the power mode
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: BatteryHealth.summary()
                    }
                }

                // Bluetooth
                MenuBarItem {
                    readonly property var adapter: Bluetooth.defaultAdapter
                    readonly property bool connected: Bluetooth.devices.values.some(d => d.connected)
                    visible: bar.cfg.bluetooth && adapter !== null
                    icon: !adapter?.enabled ? "bluetooth-slash" : connected ? "bluetooth-connected" : "bluetooth"
                    bold: true // the weight of the drawn battery and Wi-Fi
                    color: bar.fg
                    active: Panels.isOpen("bluetoothMenu", bar.screen)
                    menu: "bluetoothMenu"
                    onClicked: Panels.toggle("bluetoothMenu", bar.screen, mapToItem(null, 0, 0).x)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: {
                            if (!parent.adapter?.enabled)
                                return "Bluetooth off";
                            const on = Bluetooth.devices.values.filter(d => d.connected);
                            if (on.length === 0)
                                return "Bluetooth on · nothing connected";
                            return on.map(d => (d.name || d.deviceName) + (d.batteryAvailable ? " · " + Math.round(d.battery * 100) + "%" : "")).join(", ");
                        }
                    }
                }

                // Wi-Fi
                MenuBarItem {
                    id: wifiItem
                    readonly property var wifi: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
                    readonly property var network: wifi?.networks.values.find(n => n.connected) ?? null
                    readonly property real strength: network?.signalStrength ?? 0
                    visible: bar.cfg.wifi && wifi !== null
                    color: bar.fg
                    glyph: Component {
                        WifiGlyph {
                            on: Networking.wifiEnabled
                            connected: wifiItem.network !== null
                            strength: wifiItem.strength
                            color: bar.fg
                        }
                    }
                    active: Panels.isOpen("wifiMenu", bar.screen)
                    menu: "wifiMenu"
                    onClicked: Panels.toggle("wifiMenu", bar.screen, mapToItem(null, 0, 0).x)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: !Networking.wifiEnabled ? "Wi-Fi off" : !parent.network ? "Wi-Fi on · not connected" : parent.network.name + " · " + (parent.strength > 0.75 ? "Excellent" : parent.strength > 0.5 ? "Good" : parent.strength > 0.3 ? "Fair" : "Weak") + " signal"
                    }
                }

                // Sound (scroll to change volume)
                MenuBarItem {
                    visible: bar.cfg.sound
                    icon: Audio.icon
                    fill: true
                    color: bar.fg
                    active: Panels.isOpen("soundMenu", bar.screen)
                    menu: "soundMenu"
                    onClicked: Panels.toggle("soundMenu", bar.screen, mapToItem(null, 0, 0).x)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: (Audio.muted ? "Muted" : Math.round(Audio.volume * 100) + "%") + " · " + (Audio.sink?.description || Audio.sink?.nickname || "Sound")
                    }
                }

                // Focus: the active mode's icon; click for the Focus page
                MenuBarItem {
                    visible: Focus.on
                    icon: Focus.mode?.icon ?? "moon"
                    fill: true
                    color: bar.fg
                    onClicked: {
                        Panels.ccPage = "focus";
                        Panels.show("controlCenter", bar.screen);
                    }
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: (Focus.mode?.name ?? "Focus") + " · " + Focus.statusText
                    }
                }

                MenuBarItem {
                    visible: bar.cfg.ai
                    icon: "sparkle"
                    fill: Panels.isOpen("sidebar", bar.screen)
                    color: bar.fg
                    active: Panels.isOpen("sidebar", bar.screen)
                    onClicked: Panels.toggle("sidebar", bar.screen)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: "AI assistant"
                    }
                }
                MenuBarItem {
                    visible: bar.cfg.search
                    icon: "magnifying-glass"
                    bold: true
                    color: bar.fg
                    active: Panels.isOpen("spotlight", bar.screen)
                    onClicked: Panels.toggle("spotlight", bar.screen)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: "Search"
                    }
                }
                PrivacyDots {
                    active: Panels.isOpen("privacyMenu", bar.screen)
                    onClicked: Panels.toggle("privacyMenu", bar.screen, mapToItem(null, 0, 0).x)
                }
                MenuBarItem {
                    visible: bar.cfg.controlCenter
                    icon: "sliders-horizontal"
                    bold: true
                    color: bar.fg
                    active: Panels.isOpen("controlCenter", bar.screen)
                    menu: "controlCenter"
                    onClicked: Panels.toggle("controlCenter", bar.screen)
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: "Control Center"
                    }
                }
                MenuBarItem {
                    visible: bar.cfg.clock
                    color: bar.fg
                    active: Panels.isOpen("notificationCenter", bar.screen)
                    menu: "notificationCenter"
                    onClicked: Panels.toggle("notificationCenter", bar.screen)
                    indicator: Focus.on ? "off" : bar.cfg.notificationIndicator
                    unread: Notifications.unread
                    text: Qt.formatDateTime(clock.date, (bar.cfg.showDate ? "ddd d MMM  " : "") + (bar.cfg.showSeconds ? "h:mm:ss AP" : "h:mm AP"))
                    HoverTip {
                        target: parent
                        hovered: parent.hovered && !parent.active
                        screen: bar.screen
                        text: Qt.formatDate(clock.date, "dddd d MMMM yyyy") + (Notifications.unread > 0 ? " · " + Notifications.unread + (Notifications.unread === 1 ? " notification" : " notifications") : "")
                    }

                    SystemClock {
                        id: clock
                        precision: bar.cfg.showSeconds ? SystemClock.Seconds : SystemClock.Minutes
                    }
                }
            }
        }
    }
}
