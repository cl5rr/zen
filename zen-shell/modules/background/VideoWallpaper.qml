import QtQuick
import QtMultimedia
import qs.components

Item {
    id: vid

    property string path
    property Item owner
    readonly property int status: player.playbackState === MediaPlayer.PlayingState ? Image.Ready : Image.Loading

    anchors.fill: parent
    opacity: 0

    onStatusChanged: {
        if (status === Image.Ready)
            fadeIn.start();
    }

    MediaPlayer {
        id: player

        source: vid.path ? `file://${vid.path}` : ""
        loops: MediaPlayer.Infinite
        videoOutput: output
        audioOutput: AudioOutput {
            muted: true
        }
        Component.onCompleted: play()
    }

    VideoOutput {
        id: output

        anchors.fill: parent
        fillMode: VideoOutput.PreserveAspectCrop
    }

    Anim on opacity {
        id: fadeIn

        type: Anim.SlowEffects
        running: false
        from: 0
        to: 1
    }

    Timer {
        running: vid.owner && vid.owner.current !== vid && vid.owner.current?.status === Image.Ready
        interval: fadeIn.duration
        onTriggered: vid.destroy()
    }
}
