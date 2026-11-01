import QtQuick
import Quickshell
import Quickshell.Io

Item {
    id: root

    // Resolve from this checkout, independently of the shell's working directory.
    readonly property string launcher: localPath("../scripts/omawser")
    readonly property string binary: localPath("../src-tauri/target/release/omawser")
    property string serviceState: "checking"
    property string message: "Checking the browser executable and launcher."
    property string lastAction: "none"
    property bool alive: true
    property bool pendingOpen: false
    property bool busy: false
    property bool stoppingProbe: false

    function localPath(relative: string): string {
        return decodeURIComponent(Qt.resolvedUrl(relative).toString().replace(/^file:\/\//, ""))
    }

    function check(openAfter: bool): string {
        if (!alive) return "unavailable"
        if (busy || probe.running || stoppingProbe) return "busy"
        busy = true
        pendingOpen = openAfter
        lastAction = openAfter ? "checking-open" : "checking-refresh"
        serviceState = "checking"
        message = "Checking the browser executable and launcher."
        watchdog.start()
        probe.running = true
        return "checking"
    }

    function complete(exitCode: int): void {
        if (stoppingProbe) {
            stoppingProbe = false
            return
        }
        if (!alive || !busy) return
        watchdog.stop()
        var launch = pendingOpen
        pendingOpen = false
        busy = false
        if (exitCode !== 0) {
            serviceState = "unbuilt"
            message = "Build the browser with cargo build --release --locked --manifest-path <plugin-dir>/src-tauri/Cargo.toml --target-dir <plugin-dir>/src-tauri/target; ensure scripts/omawser is executable, then refresh."
            lastAction = launch ? "open-rejected" : "checked"
            return
        }
        serviceState = "ready"
        message = "Browser executable and launcher are executable."
        lastAction = "checked"
        if (launch && alive) {
            // This acknowledges dispatch only; it cannot prove a window is visible.
            Quickshell.execDetached([launcher])
            lastAction = "launch-requested"
        }
    }

    IpcHandler {
        target: "komagata.omawser"
        function open(): string { return root.check(true) }
        function refresh(): string { return root.check(false) }
        function status(): string {
            return JSON.stringify({state: root.serviceState, message: root.message,
                busy: root.busy || root.stoppingProbe || probe.running, lastAction: root.lastAction})
        }
    }

    Process {
        id: probe
        // GNU test accepts separate argv, including spaces and shell metacharacters.
        command: ["test", "-x", root.binary, "-a", "-x", root.launcher]
        onExited: function(exitCode, exitStatus) { root.complete(exitCode) }
    }

    Timer {
        id: watchdog
        interval: 3000
        onTriggered: {
            if (!root.alive || !root.busy) return
            root.pendingOpen = false
            // Refuse new work until the terminated probe has delivered its exit.
            root.stoppingProbe = probe.running
            root.busy = false
            root.serviceState = "failed"
            root.message = "Executable check did not finish within 3 seconds; refresh to retry."
            root.lastAction = "check-failed"
            probe.running = false
        }
    }

    Component.onCompleted: check(false)
    Component.onDestruction: {
        alive = false
        pendingOpen = false
        busy = false
        watchdog.stop()
        probe.running = false
    }
}
