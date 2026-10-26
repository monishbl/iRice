pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Static facts about the machine, read once.
Singleton {
    id: root

    property string osName: "Linux"
    property string osLogo: ""
    property string hostname: ""
    property string kernel: ""
    property string cpu: ""
    property string memory: ""
    property string gpu: ""

    function field(text, key) {
        const m = text.match(new RegExp("^" + key + "=\"?([^\"\\n]*)\"?", "m"));
        return m ? m[1] : "";
    }

    FileView {
        path: "/etc/os-release"
        onLoaded: {
            root.osName = root.field(text(), "PRETTY_NAME") || root.field(text(), "NAME") || "Linux";
            root.osLogo = root.field(text(), "LOGO");
        }
    }
    FileView {
        path: "/proc/cpuinfo"
        onLoaded: root.cpu = (text().match(/^model name\s*:\s*(.*)$/m)?.[1] ?? "").replace(/\(R\)|\(TM\)/g, "").replace(/\s+/g, " ").trim()
    }
    FileView {
        path: "/proc/meminfo"
        onLoaded: {
            const kb = parseInt(text().match(/^MemTotal:\s*(\d+)/m)?.[1] ?? "0");
            root.memory = Math.round(kb / 1024 / 1024) + " GB";
        }
    }
    FileView {
        path: "/proc/sys/kernel/hostname"
        onLoaded: root.hostname = text().trim()
    }
    FileView {
        path: "/proc/sys/kernel/osrelease"
        onLoaded: root.kernel = text().trim()
    }
    Process {
        running: true
        command: ["sh", "-c", "glxinfo -B 2>/dev/null | sed -n 's/^OpenGL renderer string: //p' | sed 's/ (.*//'"]
        stdout: StdioCollector {
            onStreamFinished: root.gpu = text.trim().replace(/^Mesa /, "")
        }
    }
}
