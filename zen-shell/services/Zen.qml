pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    property var windowMap: ({})
    property var workspaceMap: ({})
    property var islands: []
    property var cameras: ({})
    property var keyboardLayouts: []
    property int keyboardLayoutIdx: 0
    property bool overviewOpen: false
    property int focusedWindowId: -1
    property int layoutRevision: 0
    property bool connected: false

    readonly property var windows: {
        layoutRevision;
        return Object.values(windowMap);
    }
    readonly property var workspaceList: {
        layoutRevision;
        return Object.values(workspaceMap).sort((a, b) => a.idx - b.idx);
    }
    readonly property var focusedWindow: {
        layoutRevision;
        return windowMap[focusedWindowId] ?? null;
    }
    readonly property var focusedWorkspace: {
        layoutRevision;
        return workspaceList.find(w => w.is_focused) ?? null;
    }
    readonly property string focusedOutput: focusedWorkspace?.output ?? ""
    readonly property var activeIsland: islands.find(i => i.is_active) ?? null
    readonly property string kbLayoutFull: keyboardLayouts[keyboardLayoutIdx] ?? ""

    signal configReloaded

    // actions
    function action(args: var): void {
        Quickshell.execDetached(["zen", "msg", "action", ...args.map(a => String(a))]);
    }
    function focusWindow(id: int): void {
        action(["focus-window", "--id", id]);
    }
    function closeWindow(id: int): void {
        action(["close-window", "--id", id]);
    }
    function toggleFloating(id: int): void {
        action(["toggle-window-floating", "--id", id]);
    }
    function flyToWindow(id: int): void {
        action(["fly-to-window", "--id", id]);
    }
    function flyToIsland(id: int): void {
        action(["fly-to-island", "--id", id]);
    }
    function flyToPoint(x: real, y: real): void {
        action(["fly-to-point", "--", x, y]);
    }
    function focusWorkspace(idx: int): void {
        action(["focus-workspace", idx]);
    }

    function windowsOn(workspaceId: int): var {
        return windows.filter(w => w.workspace_id === workspaceId);
    }
    function camera(output: string): var {
        return cameras[output] ?? null;
    }

    // events
    function handle(event: var): void {
        const kind = Object.keys(event)[0];
        const e = event[kind];
        switch (kind) {
        case "WorkspacesChanged": {
            const map = {};
            for (const w of e.workspaces)
                map[w.id] = w;
            workspaceMap = map;
            break;
        }
        case "WorkspaceActivated": {
            const target = workspaceMap[e.id];
            if (!target)
                return;
            for (const w of Object.values(workspaceMap)) {
                if (w.output === target.output)
                    w.is_active = w.id === e.id;
                if (e.focused)
                    w.is_focused = w.id === e.id;
            }
            break;
        }
        case "WorkspaceActiveWindowChanged": {
            const w = workspaceMap[e.workspace_id];
            if (!w)
                return;
            w.active_window_id = e.active_window_id;
            break;
        }
        case "WindowsChanged": {
            const map = {};
            focusedWindowId = -1;
            for (const w of e.windows) {
                map[w.id] = w;
                if (w.is_focused)
                    focusedWindowId = w.id;
            }
            windowMap = map;
            break;
        }
        case "WindowOpenedOrChanged":
            windowMap[e.window.id] = e.window;
            if (e.window.is_focused)
                focusedWindowId = e.window.id;
            break;
        case "WindowClosed":
            delete windowMap[e.id];
            if (focusedWindowId === e.id)
                focusedWindowId = -1;
            break;
        case "WindowFocusChanged":
            focusedWindowId = e.id ?? -1;
            for (const w of Object.values(windowMap))
                w.is_focused = w.id === focusedWindowId;
            break;
        case "KeyboardLayoutsChanged":
            keyboardLayouts = e.keyboard_layouts.names;
            keyboardLayoutIdx = e.keyboard_layouts.current_idx;
            return;
        case "KeyboardLayoutSwitched":
            keyboardLayoutIdx = e.idx;
            return;
        case "IslandsChanged":
            islands = e.islands;
            return;
        case "CameraChanged": {
            const next = Object.assign({}, cameras);
            next[e.camera.output] = e.camera;
            cameras = next;
            return;
        }
        case "OverviewOpenedOrClosed":
            overviewOpen = e.is_open;
            return;
        case "ConfigLoaded":
            root.configReloaded();
            return;
        default:
            return;
        }
        layoutRevision++;
    }

    Process {
        id: stream

        command: ["zen", "msg", "--json", "event-stream"]
        running: true
        onStarted: root.connected = true
        onExited: {
            root.connected = false;
            restart.start();
        }
        stdout: SplitParser {
            onRead: line => {
                try {
                    root.handle(JSON.parse(line));
                } catch (err) {
                    console.warn("zen: could not read event", line, err);
                }
            }
        }
    }

    Timer {
        id: restart

        interval: 1000
        onTriggered: stream.running = true
    }
}
