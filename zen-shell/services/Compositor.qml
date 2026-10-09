pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Caelestia.I18n

Singleton {
    id: root

    readonly property bool usingLua: false

    property var monitors: ({ values: [] })
    property var workspaces: ({ values: [] })
    property var toplevels: ({ values: [] })

    readonly property var focusedMonitor: monitors.values.find(m => m.focused) ?? monitors.values[0] ?? null
    readonly property var focusedWorkspace: focusedMonitor?.activeWorkspace ?? null
    readonly property var activeToplevel: toplevels.values.find(t => t.focused) ?? null
    readonly property int activeWsId: focusedWorkspace?.id ?? 1

    readonly property bool capsLock: false
    readonly property bool numLock: false
    readonly property string kbLayoutFull: Zen.kbLayoutFull || Tr.trCtx("Unknown", "keyboard layout")
    readonly property string defaultKbLayout: kbMap.get(Zen.keyboardLayouts[0] ?? "") ?? "??"
    readonly property string kbLayout: kbMap.get(kbLayoutFull) ?? "??"
    readonly property var kbMap: new Map()

    readonly property QtObject extras: QtObject {
        readonly property var options: ({})
        readonly property var devices: ({ keyboards: [] })

        function batchMessage(messages: var): void {}
        function applyOptions(options: var): void {}
        function message(message: string): void {}
        function refreshDevices(): void {}
        function refreshOptions(): void {}
    }
    readonly property var options: extras.options
    readonly property var devices: extras.devices

    property string lastSpecialWorkspace: ""

    signal configReloaded

    // zen
    readonly property var islands: Zen.islands
    readonly property var cameras: Zen.cameras

    function flyToWindow(id: int): void {
        Zen.flyToWindow(id);
    }
    function flyToIsland(id: int): void {
        Zen.flyToIsland(id);
    }
    function flyToPoint(x: real, y: real): void {
        Zen.flyToPoint(x, y);
    }

    // build
    function rebuild(): void {
        const monitorsByName = {};
        const monitorList = [];
        let i = 0;
        for (const screen of Quickshell.screens) {
            const mon = {
                id: i++,
                name: screen.name,
                x: screen.x,
                y: screen.y,
                width: screen.width,
                height: screen.height,
                scale: screen.devicePixelRatio ?? 1,
                focused: screen.name === Zen.focusedOutput,
                activeWorkspace: null,
                lastIpcObject: {
                    focused: screen.name === Zen.focusedOutput,
                    specialWorkspace: {
                        id: 0,
                        name: ""
                    }
                }
            };
            monitorsByName[screen.name] = mon;
            monitorList.push(mon);
        }

        const workspacesById = {};
        const workspaceList = [];
        for (const ws of Zen.workspaceList) {
            const mon = monitorsByName[ws.output] ?? null;
            const obj = {
                id: ws.idx,
                zenId: ws.id,
                name: ws.name ?? String(ws.idx),
                monitor: mon,
                focused: ws.is_focused,
                active: ws.is_active,
                toplevels: {
                    values: []
                },
                lastIpcObject: {
                    id: ws.idx,
                    name: ws.name ?? String(ws.idx),
                    monitor: ws.output ?? "",
                    windows: 0
                }
            };
            if (mon && ws.is_active)
                mon.activeWorkspace = obj;
            workspacesById[ws.id] = obj;
            workspaceList.push(obj);
        }

        const toplevelList = [];
        for (const w of Zen.windows) {
            const ws = workspacesById[w.workspace_id] ?? null;
            const size = w.layout?.window_size ?? [0, 0];
            const at = w.layout?.tile_pos_in_workspace_view ?? [0, 0];
            const obj = {
                address: w.id.toString(16),
                zenId: w.id,
                title: w.title ?? "",
                focused: w.is_focused,
                workspace: ws,
                monitor: ws?.monitor ?? null,
                wayland: waylandToplevel(w),
                lastIpcObject: {
                    address: "0x" + w.id.toString(16),
                    class: w.app_id ?? "",
                    initialClass: w.app_id ?? "",
                    title: w.title ?? "",
                    initialTitle: w.title ?? "",
                    floating: w.is_floating,
                    fullscreen: 0,
                    pinned: false,
                    xwayland: false,
                    mapped: true,
                    pid: w.pid ?? -1,
                    tags: [],
                    at: [at[0], at[1]],
                    size: [size[0], size[1]],
                    workspace: {
                        id: ws?.id ?? -1,
                        name: ws?.name ?? ""
                    }
                }
            };
            if (ws) {
                ws.toplevels.values.push(obj);
                ws.lastIpcObject.windows++;
            }
            toplevelList.push(obj);
        }

        monitors = {
            values: monitorList
        };
        workspaces = {
            values: workspaceList
        };
        toplevels = {
            values: toplevelList
        };
    }

    function waylandToplevel(w: var): var {
        const all = ToplevelManager.toplevels.values;
        return all.find(t => t.appId === w.app_id && t.title === w.title) ?? all.find(t => t.appId === w.app_id) ?? null;
    }

    // api
    function dispatch(request: string): void {
        let m;
        if ((m = request.match(/^workspace (\d+)$/)))
            Zen.focusWorkspace(parseInt(m[1]));
        else if ((m = request.match(/^workspace r([+-])1$/)))
            Zen.action([m[1] === "+" ? "focus-workspace-down" : "focus-workspace-up"]);
        else if ((m = request.match(/^togglefloating address:0x([0-9a-f]+)$/)))
            Zen.toggleFloating(parseInt(m[1], 16));
        else if ((m = request.match(/^killwindow address:0x([0-9a-f]+)$/)))
            Zen.closeWindow(parseInt(m[1], 16));
        else if ((m = request.match(/^movetoworkspace (\d+),address:0x([0-9a-f]+)$/)))
            Zen.action(["move-window-to-workspace", "--window-id", parseInt(m[2], 16), m[1]]);
        else if (request === "dpms off")
            Zen.action(["power-off-monitors"]);
        else if (request === "dpms on")
            Zen.action(["power-on-monitors"]);
        else if (request === "exit")
            Zen.action(["quit"]);
        else
            console.warn("compositor: no ZEN equivalent for", request);
    }

    function focusWorkspace(ws: var): void {
        const n = parseInt(ws);
        if (!isNaN(n))
            Zen.focusWorkspace(n);
    }

    function toggleSpecial(name: string): void {}

    function cycleSpecialWorkspace(direction: string): void {}

    function monitorNames(): list<string> {
        return monitors.values.map(e => e.name);
    }

    function monitorFor(screen: ShellScreen): var {
        return monitors.values.find(m => m.name === screen?.name) ?? null;
    }

    function trimWsName(name: string): string {
        return name;
    }

    function toplevelsForWs(ws: int, ignoredTags = []): var {
        return toplevels.values.filter(t => t.workspace && t.workspace.id === ws && !isToplevelIgnored(t, ignoredTags));
    }

    function isToplevelIgnored(toplevel: var, ignoredTags = []): bool {
        const ipc = toplevel?.lastIpcObject;
        if (!ipc?.class || !ipc.mapped)
            return true;
        return ipc.tags?.some(tag => ignoredTags.includes(tag.replace(/\*$/, ""))) ?? false;
    }

    function reloadDynamicConfs(): void {}

    Component.onCompleted: rebuild()

    Connections {
        target: Zen

        function onLayoutRevisionChanged(): void {
            root.rebuild();
        }

        function onConfigReloaded(): void {
            root.configReloaded();
        }
    }

    Connections {
        target: Quickshell

        function onScreensChanged(): void {
            root.rebuild();
        }
    }

    FileView {
        path: Quickshell.env("CAELESTIA_XKB_RULES_PATH") || "/usr/share/X11/xkb/rules/base.lst"
        onLoaded: {
            const layoutMatch = text().match(/! layout\n([\s\S]*?)\n\n/);
            if (layoutMatch) {
                for (const line of layoutMatch[1].split("\n")) {
                    if (!line.trim() || line.trim().startsWith("!"))
                        continue;
                    const match = line.match(/^\s*([a-z]{2,})\s+([a-zA-Z() ]+)$/);
                    if (match)
                        root.kbMap.set(match[2], match[1]);
                }
            }

            const variantMatch = text().match(/! variant\n([\s\S]*?)\n\n/);
            if (variantMatch) {
                for (const line of variantMatch[1].split("\n")) {
                    if (!line.trim() || line.trim().startsWith("!"))
                        continue;
                    const match = line.match(/^\s*([a-zA-Z0-9_-]+)\s+([a-z]{2,}): (.+)$/);
                    if (match)
                        root.kbMap.set(match[3], match[2]);
                }
            }
        }
    }

    IpcHandler {
        target: "compositor"

        function rebuild(): void {
            root.rebuild();
        }

        function focusedOutput(): string {
            return root.focusedMonitor?.name ?? "";
        }
    }
}
