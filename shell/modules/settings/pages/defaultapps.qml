import QtQuick
import qs.common
import qs.modules.settings
import qs.services

// Default Apps: what the shortcuts and the shell open (a terminal, the
// browser, editors, the mixer, a task manager)
Column {
    width: parent?.width ?? 0
    spacing: 22

    Group {
        width: parent.width
        title: "Default apps"
        footer: "Used by keyboard shortcuts and the shell. Automatic picks the first one installed. The Files app is set in Desktop & Widgets."
        Row {
            label: "Terminal"
            detail: "Super+Return"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "gnome-terminal", "kgx", "ptyxis", "xfce4-terminal", "xterm"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.terminal
                onChosen: v => Config.options.defaultApps.terminal = v
            }
        }
        Row {
            label: "Browser"
            detail: "Super+W"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic (system default)" }].concat(["zen-browser", "firefox", "google-chrome-stable", "chromium", "brave"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.browser
                onChosen: v => Config.options.defaultApps.browser = v
            }
        }
        Row {
            label: "Code editor"
            detail: "Super+C"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["code", "codium", "zed", "cursor", "windsurf"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.code
                onChosen: v => Config.options.defaultApps.code = v
            }
        }
        Row {
            label: "Text editor"
            detail: "Super+X"
            Dropdown {
                width: 220
                options: [{ value: "", label: "Automatic" }].concat(["gnome-text-editor", "kate", "mousepad"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                value: Config.options.defaultApps.text
                onChosen: v => Config.options.defaultApps.text = v
            }
        }
    }

    // ── More options: the fine-tuning, folded away (MoreOptions) ───────
    MoreOptions {
        Group {
            width: parent.width
            title: "Default apps"
            Row {
                label: "Volume mixer"
                detail: "Ctrl+Super+V"
                Dropdown {
                    width: 220
                    options: [{ value: "", label: "Automatic" }].concat(["pavucontrol", "pavucontrol-qt"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                    value: Config.options.defaultApps.mixer
                    onChosen: v => Config.options.defaultApps.mixer = v
                }
            }
            Row {
                label: "Task manager"
                detail: "Ctrl+Shift+Escape"
                Dropdown {
                    width: 220
                    options: [{ value: "", label: "Automatic" }].concat(["gnome-system-monitor", "plasma-systemmonitor", "kitty -e btop", "kitty -e htop"].filter(c => Tools.has(c.split(" ").pop())).map(c => ({ value: c, label: c })))
                    value: Config.options.defaultApps.tasks
                    onChosen: v => Config.options.defaultApps.tasks = v
                }
            }
        }
    }
}
