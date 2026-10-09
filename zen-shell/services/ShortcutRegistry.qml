pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    property var shortcuts: []

    function add(shortcut: var): void {
        shortcuts = [...shortcuts, shortcut];
    }

    function remove(shortcut: var): void {
        shortcuts = shortcuts.filter(s => s !== shortcut);
    }

    function find(name: string): var {
        return shortcuts.find(s => s.name === name) ?? null;
    }

    IpcHandler {
        target: "shortcuts"

        function press(name: string): string {
            const s = root.find(name);
            if (!s)
                return `no shortcut called ${name}`;
            s.pressed();
            s.released();
            return "ok";
        }

        function list(): string {
            return root.shortcuts.map(s => `${s.name}\t${s.description}`).join("\n");
        }
    }
}
