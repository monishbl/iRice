pragma Singleton
import QtQuick
import Quickshell

// Where the day is: minutes since midnight, and today's sunrise and sunset
// (set by the weather service; sensible defaults until then). Drives the
// Auto appearance and time-of-day wallpapers. Ticks once a minute.
Singleton {
    id: root

    property int sunrise: 390 // 6:30
    property int sunset: 1110 // 18:30
    readonly property int minute: clock.date.getHours() * 60 + clock.date.getMinutes()
    readonly property bool day: minute >= sunrise && minute < sunset

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }
}
