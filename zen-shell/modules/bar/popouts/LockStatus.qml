import QtQuick.Layouts
import Caelestia.Config
import Caelestia.I18n
import qs.components
import qs.services

ColumnLayout {
    spacing: Tokens.spacing.small

    StyledText {
        text: Compositor.capsLock ? Tr.tr("Caps lock enabled") : Tr.tr("Caps lock disabled")
    }

    StyledText {
        text: Compositor.numLock ? Tr.tr("Num lock enabled") : Tr.tr("Num lock disabled")
    }
}
