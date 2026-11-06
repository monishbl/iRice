import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.common
import qs.services

// Settings drawn in Rust: the "system" page group's side in this shell
// (RustBridge.qml; native/tessera-ui/src/surfaces/settings/pages/system).
// While Settings is open (`live`) it tells the pages what only this shell
// knows (`feed`, the pages' cx.feed["system"]): the fingerprint reader and
// the add-a-finger sheet's progress, the face camera, the firewall and the
// root helper (sharing-check.py status), Quick Share's trust, the energy
// mode and battery, Cool-down, live captions. It does what they ask (`act`:
// cx.ask("system", what, args)): everything that needs root or the power
// daemon goes through this shell's services, never from tessera-ui.
Scope {
    id: root

    property bool live: false
    readonly property string page: live ? Panels.settingsPage : ""

    // ── What each page needs fresh when it opens (as the QML pages' own
    // Component.onCompleted) ──
    onPageChanged: {
        if (page === "security") {
            Fingerprint.refresh();
            Face.refresh();
            checkSystem();
        } else if (page === "accessibility") {
            Dictation.refresh();
        }
    }
    // The apps' energy report runs while Battery & Power shows (Power.energyUsers)
    readonly property bool energy: page === "power"
    onEnergyChanged: Power.energyUsers = Math.max(0, Power.energyUsers + (energy ? 1 : -1))

    // Which firewall is on, and whether Tessera's root helper is there
    // (sharing-check.py status: read only)
    property var check: ({})
    readonly property string checkScript: decodeURIComponent(Qt.resolvedUrl("../../assets/scripts/sharing-check.py").toString().replace("file://", ""))
    function checkSystem() {
        checkProc.command = ["python3", "-I", checkScript, "status"];
        checkProc.running = true;
    }
    Process {
        id: checkProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.check = JSON.parse(text);
                } catch (e) {}
            }
        }
    }
    Connections {
        target: SharingPorts
        function onBusyChanged() {
            if (!SharingPorts.busy && root.page === "security")
                root.checkSystem();
        }
    }

    // ── The add-a-finger sheet: each touch taken or missed, the reader's
    // reason for a miss for a moment, the next section flashing red ──
    property int takes: 0
    property int misses: 0
    property string retry: ""
    property bool flash: false
    Timer { id: retryShown; interval: 2200; onTriggered: root.retry = "" }
    Timer { id: flashOff; interval: 510; onTriggered: root.flash = false }
    Connections {
        target: Fingerprint
        function onScanned(result) {
            if (result === "enroll-stage-passed" || result === "enroll-completed") {
                root.retry = "";
                root.takes++;
            } else if (result.startsWith("enroll-") && result !== "enroll-failed") {
                root.retry = Fingerprint.hints[result] ?? "Try that one again.";
                retryShown.restart();
                root.misses++;
                root.flash = true;
                flashOff.restart();
            }
        }
    }

    readonly property var feed: live ? ({
            fingerprint: {
                hasReader: Fingerprint.hasReader,
                ready: Fingerprint.ready,
                readerName: Fingerprint.reader?.name ?? "",
                readerUnsupported: Fingerprint.reader?.unsupported ?? false,
                missing: Fingerprint.missing,
                aur: (Fingerprint.info.aur ?? []).length > 0,
                gdm: !!Fingerprint.info.gdm,
                installing: Fingerprint.installing,
                installError: Fingerprint.installError,
                fixing: Fingerprint.fixing,
                fixError: Fingerprint.fixError,
                slowStart: Fingerprint.slowStart,
                enrolled: Fingerprint.enrolled,
                fingerOrder: Object.keys(Fingerprint.fingerNames),
                fingerNames: Fingerprint.fingerNames,
                enrolling: Fingerprint.enrolling,
                enrollStage: Fingerprint.enrollStage,
                enrollCount: Fingerprint.enrollCount,
                enrollStages: Fingerprint.enrollStages,
                enrollError: Fingerprint.enrollError,
                pam: Fingerprint.pam,
                pamBusy: Fingerprint.pamBusy,
                pamError: Fingerprint.pamError
            },
            sheet: { takes: root.takes, misses: root.misses, retry: root.retry, flash: root.flash },
            face: {
                hasCamera: Face.hasCamera,
                installed: Face.installed,
                setUp: Face.setUp,
                faces: Face.faces,
                busy: Face.busy,
                error: Face.error,
                installError: Face.installError,
                installing: Face.installing,
                camera: Face.camera?.name ?? ""
            },
            check: root.check,
            quickShare: { trusted: QuickShare.trusted, trustBusy: QuickShare.trustBusy, trustError: QuickShare.trustError },
            ports: { error: SharingPorts.error, failedWith: SharingPorts.failedWith, busy: SharingPorts.busy },
            updatesRunning: Updates.running,
            power: {
                ppd: Backends.ppd,
                powerShort: Backends.powerShort ?? "",
                powerReason: Backends.powerReason ?? "",
                hasBattery: Power.hasBattery,
                why: Power.why,
                mode: Power.mode,
                modeName: Power.modeName,
                hasPerformance: PowerProfiles.hasPerformanceProfile,
                guardError: Power.guardError,
                ownHypridle: Power.ownHypridle,
                userHypridle: Power.userHypridle,
                energyMinutes: Power.energyMinutes,
                energyApps: Power.energyApps.map(a => ({ key: a.key, seconds: a.seconds })),
                keepAwake: System.keepAwake,
                keepAwakeStatus: System.keepAwakeStatus,
                priority: { available: ShellPriority.available, on: ShellPriority.on, busy: ShellPriority.busy, error: ShellPriority.error }
            },
            cooldown: { active: CoolDown.active, why: CoolDown.why },
            captions: {
                hasBinary: Captions.hasBinary,
                hasModel: Captions.hasModel,
                ready: Captions.ready,
                error: Captions.error,
                downloading: Captions.downloading,
                installing: Dictation.installing,
                installError: Dictation.installError,
                downloadProgress: Dictation.downloadProgress,
                downloadError: Dictation.downloadError,
                lastDownload: Dictation.lastDownload,
                dictDownloading: Dictation.downloading,
                languages: Dictation.languages.map(l => ({ value: l.value, label: l.label }))
            }
        }) : null
    onFeedChanged: if (live) RustUi.tell("settings", { feed: { system: feed } })

    function act(what, args) {
        switch (what) {
        // Fingerprint (fprintd and PAM through the shell's helper)
        case "fpEnroll":
            if (typeof args[0] === "string" && Fingerprint.fingerNames[args[0]] !== undefined) {
                root.takes = 0;
                root.misses = 0;
                root.retry = "";
                Fingerprint.enroll(args[0]);
            }
            break;
        case "fpCancel":
            if (Fingerprint.enrolling)
                Fingerprint.cancelEnroll();
            break;
        case "fpRemove":
            if (Fingerprint.enrolled.includes(args[0]))
                Fingerprint.remove(args[0]);
            break;
        case "fpFix":
            Fingerprint.fix();
            break;
        case "fpInstall":
            Fingerprint.install();
            break;
        case "fpSetPam":
            if (["polkit", "sudo", "login"].includes(args[0]))
                Fingerprint.setPam(args[0], !!args[1]);
            break;
        // Face (Howdy)
        case "faceInstall":
            Face.install();
            break;
        case "faceSetUp":
            Face.setUpCamera();
            break;
        case "faceAdd":
            Face.addFace();
            break;
        case "faceClear":
            Face.clearFaces();
            break;
        // Sharing's trust and the firewall's ports
        case "quickShareTrust":
            QuickShare.setTrusted(!!args[0]);
            break;
        case "portsRetry":
            SharingPorts.retry();
            break;
        // Power
        case "powerMode":
            if (["power-saver", "balanced", "performance"].includes(args[0]))
                Power.setMode(args[0]);
            break;
        case "caffeineOff":
            System.setKeepAwake(false, 0);
            break;
        case "shellPriority":
            ShellPriority.set(!!args[0]);
            break;
        case "clearEnergy":
            Power.clearEnergy();
            break;
        case "quitApp":
            if (typeof args[0] === "string" && args[0] !== "")
                Apps.quit(args[0]);
            break;
        case "coolDownToggle":
            CoolDown.toggle();
            break;
        // Live captions
        case "captionsInstall":
            Captions.install();
            break;
        case "captionsDownload":
            Captions.download();
            break;
        case "captionsCancel":
            Dictation.cancelDownload();
            break;
        default:
            console.warn("settings (system): no act " + what);
        }
    }
}
