.pragma library
// Settings' map, the same in both looks: the sidebar's pages under their
// groups (by what you're doing), the pages that live inside another as a
// part, and where an older page id goes now.
//
// A page is pages/<id>.qml; a subpage "page/sub" is pages/page-sub.qml. A
// merged page loads other page files as parts (PagePart), under a heading.

var sections = [
    { id: "appearance", name: "Appearance", icon: "circle-half", color: "#5E5CE6", group: "Look & Feel" },
    { id: "wallpaper", name: "Wallpaper", icon: "image", color: "#32ADE6", group: "Look & Feel" },
    { id: "desktop", name: "Desktop & Widgets", icon: "squares-four", color: "#FF9F0A", group: "Look & Feel" },
    { id: "menubar", name: "Menu Bar & Dock", icon: "list", color: "#8E8E93", group: "Look & Feel" },
    { id: "lock", name: "Lock & Login Screen", icon: "lock", color: "#636366", group: "Look & Feel" },
    { id: "notifications", name: "Notifications & Island", icon: "bell", color: "#FF453A", group: "Notifications" },
    { id: "focus", name: "Focus", icon: "moon", color: "#5E5CE6", group: "Notifications" },
    { id: "windowrules", name: "Windows & Spaces", icon: "app-window", color: "#FF375F", group: "Windows" },
    { id: "displays", name: "Displays", icon: "monitor", color: "#0A84FF", group: "Devices" },
    { id: "sound", name: "Sound", icon: "speaker-high", color: "#FF375F", group: "Devices" },
    { id: "network", name: "Wi-Fi & Bluetooth", icon: "wifi-high", color: "#0A84FF", group: "Devices" },
    { id: "mouse", name: "Mouse & Trackpad", icon: "mouse", color: "#636366", group: "Devices" },
    { id: "shortcuts", name: "Keyboard", icon: "keyboard", color: "#636366", group: "Devices" },
    { id: "phone", name: "Phone & Sharing", icon: "device-mobile", color: "#30D158", group: "Devices" },
    { id: "accounts", name: "Accounts", icon: "user-circle", color: "#4285F4", group: "Accounts & Apps" },
    { id: "calendar", name: "Calendar", icon: "calendar-blank", color: "#FF453A", group: "Accounts & Apps" },
    { id: "ai", name: "AI & Sidebar", icon: "sparkle", color: "#BF5AF2", group: "Accounts & Apps" },
    { id: "defaultapps", name: "Default Apps", icon: "browsers", color: "#0A84FF", group: "Accounts & Apps" },
    { id: "security", name: "Privacy & Security", icon: "shield-check", color: "#0A84FF", group: "System" },
    { id: "power", name: "Battery & Power", icon: "battery-high", color: "#30D158", group: "System" },
    { id: "accessibility", name: "Accessibility", icon: "person-arms-spread", color: "#0A84FF", group: "System" },
    { id: "update", name: "Software Update", icon: "arrow-circle-up", color: "#8E8E93", group: "System" },
    { id: "about", name: "About", icon: "info", color: "#30D158", group: "System" }
];

// Page files shown as a part of another page: file -> { page, part } (the
// part's heading). Search finds their rows under the page they're in.
var parts = {
    "dock": { page: "menubar", part: "Dock" },
    "controlcenter": { page: "menubar", part: "Control Center" },
    "appearance-windows": { page: "windowrules", part: "Windows, Stage Manager and Overview" },
    "bluetooth": { page: "network", part: "Bluetooth" },
    "trackpad": { page: "mouse", part: "Trackpad" },
    "dictation": { page: "shortcuts", part: "Dictation" },
    "sharing": { page: "phone", part: "Sharing" },
    "google": { page: "accounts", part: "Google" },
    "icloud": { page: "accounts", part: "iCloud" },
    "microsoft": { page: "accounts", part: "Microsoft 365" },
    "sidebar": { page: "ai", part: "Sidebar" },
    "biometrics": { page: "security", part: "" }
};

// Older page ids (other apps, scripts, IPC, a remembered page) -> where
// they are now, and what to show there
var aliases = {
    "dock": { page: "menubar", part: "Dock" },
    "controlcenter": { page: "menubar", part: "Control Center" },
    "menubar/island": { page: "notifications/island" },
    "appearance/windows": { page: "windowrules", part: "Windows, Stage Manager and Overview" },
    "appearance/cooldown": { page: "power/cooldown" },
    "bluetooth": { page: "network", part: "Bluetooth" },
    "trackpad": { page: "mouse", part: "Trackpad" },
    "trackpad/gestures": { page: "mouse/gestures" },
    "dictation": { page: "shortcuts", part: "Dictation" },
    "sharing": { page: "phone", part: "Sharing" },
    "google": { page: "accounts", part: "Google" },
    "icloud": { page: "accounts", part: "iCloud" },
    "microsoft": { page: "accounts", part: "Microsoft 365" },
    "sidebar": { page: "ai", part: "Sidebar" },
    "loginitems": { page: "about/loginitems" },
    "bugreport": { page: "about/bugreport" }
};

// The page a search hit in file `file` (no .qml) belongs to, and the part
// heading it's under (or "")
function pageOfFile(file) {
    var p = parts[file];
    if (p)
        return { page: p.page, part: p.part };
    return { page: file.replace("-", "/"), part: "" };
}
