-- iRice keybinds. Descriptions ("Group: Action") feed the cheatsheet and
-- Settings → Keyboard Shortcuts. MOD is SUPER normally (ALT in the sandbox).
local MOD = IRICE_MOD or "SUPER"
local function shell(name) return hl.dsp.global("irice:" .. name) end
local function run(cmd) return hl.dsp.exec_cmd(cmd) end

-- Shell
hl.bind(MOD .. " + Space", shell("spotlight"), { description = "Shell: Spotlight" })
-- Tap the modifier alone for the overview (a release bind: Hyprland skips it
-- when another key was pressed while the modifier was held)
hl.bind(MOD .. " + " .. (MOD == "SUPER" and "Super_L" or "Alt_L"), shell("overview"), { release = true, description = "Shell: Overview (tap)" })
hl.bind(MOD .. " + Tab", shell("missionControl"), { description = "Shell: Mission Control" })
hl.bind(MOD .. " + SHIFT + Space", shell("launchpad"), { description = "Shell: App Library" })
hl.bind(MOD .. " + A", shell("sidebar"), { description = "Shell: AI and tools sidebar" })
hl.bind(MOD .. " + C", shell("controlCenter"), { description = "Shell: Control Center" })
hl.bind(MOD .. " + N", shell("notificationCenter"), { description = "Shell: Notification Center" })
hl.bind(MOD .. " + Comma", shell("settings"), { description = "Shell: Settings" })
hl.bind(MOD .. " + Slash", shell("cheatsheet"), { description = "Shell: Keyboard shortcuts" })
hl.bind(MOD .. " + CTRL + T", shell("wallpaper"), { description = "Shell: Change wallpaper" })
hl.bind(MOD .. " + G", shell("overlay"), { description = "Shell: Overlay widgets" })
hl.bind(MOD .. " + K", shell("keyboard"), { description = "Shell: On-screen keyboard" })
hl.bind(MOD .. " + V", shell("clipboard"), { description = "Shell: Clipboard history" })
hl.bind(MOD .. " + CTRL + Q", shell("lock"), { description = "Shell: Lock screen" })
hl.bind("CTRL + ALT + Delete", shell("session"), { description = "Shell: Power options" })
hl.bind(MOD .. " + Escape", shell("reset"), { description = "Shell: Close everything (if something is stuck)" })
-- Runs even when the shell is frozen (Hyprland starts the script, not the shell)
local shellDir = os.getenv("IRICE_ROOT") and (os.getenv("IRICE_ROOT") .. "/shell") or (os.getenv("HOME") .. "/.config/quickshell/irice")
hl.bind(MOD .. " + CTRL + Escape", run("setsid -f '" .. shellDir .. "/assets/scripts/repair-shell.sh'"), { description = "Shell: Repair (duplicated or stuck shell; apps keep running)" })

-- Screenshots and screen tools
hl.bind(MOD .. " + SHIFT + 3", shell("screenshotScreen"), { description = "Screen: Screenshot of the screen" })
hl.bind(MOD .. " + SHIFT + 4", shell("screenshotRegion"), { description = "Screen: Screenshot of an area" })
hl.bind(MOD .. " + SHIFT + S", shell("screenshotScreen"), { description = "Screen: Screenshot of the screen (saved, no selection)" })
hl.bind(MOD .. " + CTRL + SHIFT + S", shell("screenshotRegion"), { description = "Screen: Screenshot of an area" })
hl.bind(MOD .. " + SHIFT + 5", shell("screenTools"), { description = "Screen: Screenshot and recording tools" })
hl.bind("Print", shell("screenshotRegion"), { description = "Screen: Screenshot of an area" })
hl.bind(MOD .. " + SHIFT + T", shell("ocr"), { description = "Screen: Copy text from screen" })
hl.bind(MOD .. " + SHIFT + Y", shell("screenTranslate"), { description = "Screen: Translate screen text" })
hl.bind(MOD .. " + SHIFT + A", shell("askScreen"), { description = "Screen: Ask AI about part of the screen" })
hl.bind(MOD .. " + SHIFT + C", run("hyprpicker -a"), { description = "Screen: Pick a colour" })
hl.bind(MOD .. " + SHIFT + R", shell("record"), { description = "Screen: Start or stop recording" })

-- Apps
hl.bind(MOD .. " + Return", run("xdg-terminal-exec || foot || kitty"), { description = "Apps: Terminal" })
hl.bind(MOD .. " + E", run("xdg-open ~"), { description = "Apps: Files" })

-- Windows
hl.bind(MOD .. " + Q", hl.dsp.window.close(), { description = "Window: Close" })
hl.bind(MOD .. " + F", hl.dsp.window.fullscreen({ mode = "fullscreen", action = "toggle" }), { description = "Window: Full screen" })
hl.bind(MOD .. " + D", hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" }), { description = "Window: Zoom" })
hl.bind(MOD .. " + ALT + Space", hl.dsp.window.float({ action = "toggle" }), { description = "Window: Float or tile" })
hl.bind(MOD .. " + P", hl.dsp.window.pin(), { description = "Window: Keep on all spaces" })
hl.bind(MOD .. " + mouse:272", hl.dsp.window.drag(), { mouse = true, description = "Window: Move" })
hl.bind(MOD .. " + mouse:273", hl.dsp.window.resize(), { mouse = true, description = "Window: Resize" })
hl.bind(MOD .. " + CTRL + Left", shell("snap-left"), { description = "Window: Tile left half" })
hl.bind(MOD .. " + CTRL + Right", shell("snap-right"), { description = "Window: Tile right half" })
hl.bind(MOD .. " + CTRL + Up", shell("snap-top"), { description = "Window: Tile top half" })
hl.bind(MOD .. " + CTRL + Down", shell("snap-bottom"), { description = "Window: Tile bottom half" })
hl.bind(MOD .. " + CTRL + F", shell("snap-fill"), { description = "Window: Fill screen" })
hl.bind(MOD .. " + CTRL + C", shell("snap-center"), { description = "Window: Center" })
hl.bind(MOD .. " + CTRL + R", shell("snap-restore"), { description = "Window: Return to previous size" })
local dirs = { Left = "l", Right = "r", Up = "u", Down = "d" }
for key, d in pairs(dirs) do
    hl.bind(MOD .. " + " .. key, hl.dsp.focus({ direction = d }), { description = "Window: Focus " .. key:lower() })
    hl.bind(MOD .. " + SHIFT + " .. key, hl.dsp.window.move({ direction = d }), { description = "Window: Move " .. key:lower() })
end

-- Spaces
for i = 1, 9 do
    hl.bind(MOD .. " + " .. i, hl.dsp.focus({ workspace = i }), { description = "Spaces: Go to space " .. i })
    hl.bind(MOD .. " + SHIFT + " .. i, hl.dsp.window.move({ workspace = i, follow = false }), { description = "Spaces: Move window to space " .. i })
end
hl.bind(MOD .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }), { description = "Spaces: Next space" })
hl.bind(MOD .. " + mouse_up", hl.dsp.focus({ workspace = "e-1" }), { description = "Spaces: Previous space" })

-- Media and hardware keys
hl.bind("XF86AudioRaiseVolume", shell("volumeUp"), { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume", shell("volumeDown"), { locked = true, repeating = true })
hl.bind("XF86AudioMute", shell("volumeMute"), { locked = true })
hl.bind("XF86AudioMicMute", shell("micMute"), { locked = true })
hl.bind("XF86MonBrightnessUp", shell("brightnessUp"), { locked = true, repeating = true })
hl.bind("XF86MonBrightnessDown", shell("brightnessDown"), { locked = true, repeating = true })
hl.bind("XF86AudioPlay", run("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioNext", run("playerctl next"), { locked = true })
hl.bind("XF86AudioPrev", run("playerctl previous"), { locked = true })
