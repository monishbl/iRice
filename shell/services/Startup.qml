pragma Singleton
import QtQuick
import Quickshell

// Startup work that can wait: what's installed for a settings page, a
// drive to mount again, a status for later. Run 15 s after the shell starts
// (the desktop first), one every 0.4 s rather than all at once. Asked for
// after that, it runs straight away.
Singleton {
    id: root

    property bool settled: false
    property var waiting: [] // (changed in place: nothing binds to it)
    function later(fn) {
        if (settled)
            fn();
        else
            waiting.push(fn);
    }
    Timer {
        interval: 15000
        running: true
        onTriggered: {
            root.settled = true;
            next.start();
        }
    }
    Timer {
        id: next
        interval: 400
        repeat: true
        onTriggered: {
            const fn = root.waiting.shift();
            if (fn)
                fn();
            else
                stop();
        }
    }
}
