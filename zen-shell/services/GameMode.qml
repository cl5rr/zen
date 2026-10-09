pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io
import Caelestia
import Caelestia.Config
import Caelestia.I18n
import qs.services

Singleton {
    id: root

    property alias enabled: props.enabled

    onEnabledChanged: {
        if (enabled) {
            props.dndBefore = Notifs.dnd;
            Notifs.dnd = true;
            if (GlobalConfig.utilities.toasts.gameModeChanged)
                Toaster.toast(Tr.tr("Game mode enabled"), Tr.tr("Notifications wait until you are done"), "gamepad");
        } else {
            Notifs.dnd = props.dndBefore;
            if (GlobalConfig.utilities.toasts.gameModeChanged)
                Toaster.toast(Tr.tr("Game mode disabled"), Tr.tr("Notifications are back"), "gamepad");
        }
    }

    PersistentProperties {
        id: props

        property bool enabled: false
        property bool dndBefore: false

        reloadableId: "gameMode"
    }

    IpcHandler {
        function isEnabled(): bool {
            return props.enabled;
        }

        function toggle(): void {
            props.enabled = !props.enabled;
        }

        function enable(): void {
            props.enabled = true;
        }

        function disable(): void {
            props.enabled = false;
        }

        target: "gameMode"
    }
}
