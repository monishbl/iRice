import QtQuick
import Quickshell
import Quickshell.Io
import qs.common

// World clock: analog faces for the cities in Settings (Desktop). Small: the
// first city. Medium: four. Times come from `date` once a minute.
Item {
    id: root

    property string size: "m"
    readonly property var cities: Config.options.desktop.worldClocks.slice(0, size === "s" ? 1 : 4).map(c => {
        const [name, zone] = c.split("|");
        return { name: name, zone: zone || name };
    })
    property var times: ({}) // zone -> { h, m, offset (hours from here), day: -1|0|1 }

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
        onDateChanged: query.running = true
    }
    Process {
        id: query
        running: true
        command: ["sh", "-c", "date '+here %H %M %z %j'; for z in \"$@\"; do echo \"$z $(TZ=$z date '+%H %M %z %j')\"; done", "sh"].concat(root.cities.map(c => c.zone))
        stdout: StdioCollector {
            onStreamFinished: {
                const mins = z => (z[0] === "-" ? -1 : 1) * (Number(z.slice(1, 3)) * 60 + Number(z.slice(3, 5)));
                const lines = text.trim().split("\n").map(l => l.split(" "));
                const here = lines[0];
                const t = {};
                for (const l of lines.slice(1))
                    t[l[0]] = { h: Number(l[1]), m: Number(l[2]), offset: (mins(l[3]) - mins(here[3])) / 60, day: (() => { const d = Number(l[4]) - Number(here[4]); return d > 1 ? -1 : d < -1 ? 1 : Math.sign(d); })() // day-of-year wraps at New Year };
                root.times = t;
            }
        }
    }
    onCitiesChanged: query.running = true

    function describe(t) {
        if (!t)
            return "";
        const day = t.day > 0 ? "Tomorrow" : t.day < 0 ? "Yesterday" : "Today";
        const off = t.offset === 0 ? "" : (t.offset > 0 ? ", +" : ", ") + (Number.isInteger(t.offset) ? t.offset : t.offset.toFixed(1)) + "h";
        return day + off;
    }

    // Small: one big face
    Column {
        visible: root.size === "s"
        anchors.centerIn: parent
        spacing: 6
        AnalogClock {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 92
            height: 92
            hours: root.times[root.cities[0]?.zone]?.h ?? 0
            minutes: root.times[root.cities[0]?.zone]?.m ?? 0
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.cities[0]?.name ?? ""
            color: Theme.color.label
            font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
        }
    }

    // Medium: four faces in a row
    Row {
        visible: root.size !== "s"
        anchors.centerIn: parent
        width: parent.width
        Repeater {
            model: root.size !== "s" ? root.cities : []
            Column {
                id: city
                required property var modelData
                readonly property var t: root.times[modelData.zone]
                width: parent.width / 4
                spacing: 4
                AnalogClock {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(64, parent.width - 8)
                    height: width
                    hours: city.t?.h ?? 0
                    minutes: city.t?.m ?? 0
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: city.modelData.name
                    elide: Text.ElideRight
                    color: Theme.color.label
                    font { family: Theme.font.family; pixelSize: Theme.font.small; weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: root.describe(city.t)
                    elide: Text.ElideRight
                    color: Theme.color.secondary
                    font { family: Theme.font.family; pixelSize: Theme.font.caption }
                }
            }
        }
    }
}
