import QtQuick
import Quickshell
import Quickshell.Wayland

// A login screen's window and what every style does with it, once: the
// person and session chosen, logging in, and Sleep, Restart and Shut Down,
// which each ask to confirm (a second click, or Enter). Each style (the
// look's GreeterSurface, GreeterText, GreeterPlain) draws its own screen in
// it and hands over its password field. Authentication goes through greetd
// (GreeterAuth).
PanelWindow {
    id: win
    required property var auth
    property bool primary: false
    // The style's password field, and its username field ("Other user…")
    property Item passwordField: null
    property Item nameField: null
    // A password was refused (the field is already cleared): a style can shake
    signal refused
    // 24-hour time (Settings → Appearance), passed in by greeter-start.sh
    readonly property bool h24: Quickshell.env("IRICE_GREETER_CLOCK24") === "1"
    color: "black"
    WlrLayershell.namespace: "irice-greeter"
    WlrLayershell.layer: auth.mock ? WlrLayer.Overlay : WlrLayer.Top // (a preview covers the menu bar too)
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.keyboardFocus: primary ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
    anchors { top: true; bottom: true; left: true; right: true }

    readonly property var person: auth.users[auth.user] ?? null
    // The picture's name (AccountsService's), the typed one for "Other user…"
    readonly property string pictureName: auth.other ? "" : auth.userName
    readonly property string wallpaper: Quickshell.env("IRICE_GREETER_WALLPAPER") || "/usr/share/irice/wallpaper.jpg"
    property string powerChoice: ""

    function clearField() {
        if (passwordField)
            passwordField.text = "";
    }
    function nextUser(step) {
        if (auth.busy || auth.users.length < 2)
            return;
        auth.user = (auth.user + auth.users.length + step) % auth.users.length;
        clearField();
    }
    // From the username field to the password (Enter or Tab)
    function toPassword() {
        passwordField?.forceActiveFocus();
    }
    function nextSession(step) {
        if (!auth.busy && auth.sessions.length)
            auth.session = (auth.session + auth.sessions.length + (step ?? 1)) % auth.sessions.length;
    }
    function submit() {
        if (powerChoice) {
            const action = powerChoice;
            powerChoice = "";
            if (auth.mock)
                console.log("greeter (mock):", action);
            else
                Quickshell.execDetached(["systemctl", action]);
        } else if (!auth.busy) {
            auth.login(passwordField?.text ?? "");
        }
    }
    function choosePower(action) {
        if (auth.busy)
            return;
        if (powerChoice === action)
            submit();
        else
            powerChoice = action;
        passwordField?.forceActiveFocus();
    }
    // Escape: drops a power choice, else quits a preview, else clears the field
    function cancel() {
        if (powerChoice !== "")
            powerChoice = "";
        else if (auth.mock)
            auth.quitPreview();
        else
            clearField();
    }
    onPrimaryChanged: if (primary) passwordField?.forceActiveFocus()
    Connections {
        target: win.auth
        function onErrorChanged() {
            if (win.auth.error) {
                win.clearField();
                win.refused();
            }
        }
        function onUserChanged() {
            win.clearField();
            if (win.auth.other)
                win.nameField?.forceActiveFocus();
            else
                win.passwordField?.forceActiveFocus();
        }
    }
}
