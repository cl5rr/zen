import QtQuick

QtObject {
    id: root

    property bool active
    property var windows: []

    signal cleared

    function clear(): void {
        if (active)
            cleared();
    }
}
