import QtQuick
import qs.services

QtObject {
    id: root

    property string name
    property string description
    property string appid: "zen"

    signal pressed
    signal released

    Component.onCompleted: ShortcutRegistry.add(root)
    Component.onDestruction: ShortcutRegistry.remove(root)
}
