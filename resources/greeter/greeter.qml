import QtQuick
import QtQuick.Shapes
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Greetd
import "Script.js" as Script

ShellRoot {
    id: root

    readonly property bool preview: !Greetd.available
    readonly property string session: Quickshell.env("ZEN_SESSION") || "zen-session"
    readonly property var words: ["welcome", "hello", "szia", "bonjour", "hola", "ciao", "hallo", "hej"]

    property var users: []
    property int userIndex: 0
    readonly property var current: users.length > 0 ? users[userIndex] : null
    property int wordIndex: 0
    property bool awake: false
    property bool busy: false
    property bool leaving: false
    property string status: ""
    property string pending: ""
    property int failures: 0

    signal shake

    // fonts
    readonly property string sansFamily: "Rubik"

    // users
    FileView {
        path: "/etc/passwd"
        onLoaded: root.readUsers(text())
    }

    FileView {
        id: lastUser
        path: "/var/cache/zen-greeter/last-user"
        onLoaded: root.pick(text().trim())
    }

    function readUsers(text) {
        const found = [];
        for (const line of text.split("\n")) {
            const f = line.split(":");
            if (f.length < 7)
                continue;
            const uid = parseInt(f[2]);
            if (uid < 1000 || uid >= 60000 || /(nologin|false)$/.test(f[6]))
                continue;
            found.push({
                name: f[0],
                display: f[4].split(",")[0] || f[0]
            });
        }
        if (found.length === 0 && preview)
            found.push({
                name: Quickshell.env("USER") || "you",
                display: Quickshell.env("USER") || "you"
            });
        users = found;
        pick(lastUser.loaded ? lastUser.text().trim() : (Quickshell.env("USER") || ""));
    }

    function pick(name) {
        const i = users.findIndex(u => u.name === name);
        if (i >= 0)
            userIndex = i;
    }

    function cycleUser(step) {
        if (users.length < 2 || busy)
            return;
        userIndex = (userIndex + step + users.length) % users.length;
        status = "";
    }

    // login
    function wake() {
        if (!awake)
            awake = true;
    }

    function login(password) {
        if (busy || !current)
            return;
        busy = true;
        status = "";
        if (preview) {
            previewDone.start();
            return;
        }
        pending = password;
        Greetd.createSession(current.name);
    }

    function fail(message) {
        busy = false;
        pending = "";
        failures += 1;
        status = failures > 2 ? "Still not it. Caps Lock, maybe?" : (message || "That password did not work");
        if (Greetd.state !== 0)
            Greetd.cancelSession();
        shake();
    }

    Connections {
        target: Greetd

        function onAuthMessage(message, error, responseRequired, echoResponse) {
            if (responseRequired) {
                Greetd.respond(root.pending);
                root.pending = "";
            } else if (error) {
                root.status = message;
            }
        }

        function onAuthFailure(message) {
            root.fail("That password did not work");
        }

        function onError(error) {
            root.fail(error);
        }

        function onReadyToLaunch() {
            lastUser.setText(root.current.name + "\n");
            root.leaving = true;
            launch.start();
        }
    }

    Timer {
        id: launch
        interval: 700
        onTriggered: Greetd.launch([root.session])
    }

    Timer {
        id: previewDone
        interval: 500
        onTriggered: {
            root.leaving = true;
            previewQuit.start();
        }
    }

    Timer {
        id: previewQuit
        interval: 900
        onTriggered: Qt.quit()
    }

    Process {
        id: power
    }

    function powerAction(what) {
        power.command = ["systemctl", what];
        power.running = true;
    }

    component Blob: Shape {
        id: blob

        property color tint
        property real size: 600
        property real ox: 0
        property real oy: 0

        width: size
        height: size
        preferredRendererType: Shape.CurveRenderer
        transform: Translate {
            x: blob.ox
            y: blob.oy
        }

        ShapePath {
            strokeWidth: -1
            strokeColor: "transparent"
            fillGradient: RadialGradient {
                centerX: blob.size / 2
                centerY: blob.size / 2
                centerRadius: blob.size / 2
                focalX: blob.size / 2
                focalY: blob.size / 2

                GradientStop {
                    position: 0
                    color: blob.tint
                }
                GradientStop {
                    position: 1
                    color: Qt.rgba(blob.tint.r, blob.tint.g, blob.tint.b, 0)
                }
            }

            PathAngleArc {
                centerX: blob.size / 2
                centerY: blob.size / 2
                radiusX: blob.size / 2
                radiusY: blob.size / 2
                startAngle: 0
                sweepAngle: 360
            }
        }
    }

    component Drift: SequentialAnimation {
        id: drift

        required property Item target
        property real dx: 200
        property real dy: 120
        property int duration: 16000

        loops: Animation.Infinite
        running: true

        ParallelAnimation {
            NumberAnimation { target: drift.target; property: "ox"; from: 0; to: drift.dx; duration: drift.duration; easing.type: Easing.InOutSine }
            NumberAnimation { target: drift.target; property: "oy"; from: 0; to: drift.dy; duration: drift.duration; easing.type: Easing.InOutSine }
        }
        ParallelAnimation {
            NumberAnimation { target: drift.target; property: "ox"; from: drift.dx; to: 0; duration: drift.duration; easing.type: Easing.InOutSine }
            NumberAnimation { target: drift.target; property: "oy"; from: drift.dy; to: 0; duration: drift.duration; easing.type: Easing.InOutSine }
        }
    }

    component Pill: Rectangle {
        id: pill

        property string label
        signal clicked

        implicitWidth: pillText.implicitWidth + 36
        implicitHeight: 40
        radius: height / 2
        color: Qt.rgba(1, 1, 1, pillArea.containsMouse ? 0.22 : 0.12)
        border.color: Qt.rgba(1, 1, 1, 0.3)
        border.width: 1

        Behavior on color {
            ColorAnimation { duration: 150 }
        }

        Text {
            id: pillText
            anchors.centerIn: parent
            text: pill.label
            color: "white"
            font.family: root.sansFamily
            font.pixelSize: 15
            font.weight: 500
        }

        MouseArea {
            id: pillArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: pill.clicked()
        }
    }

    // screens
    Variants {
        model: Quickshell.screens

        Scope {
            id: scope

            required property var modelData
            readonly property bool primary: modelData === Quickshell.screens[0]

            PanelWindow {
                id: sky

                screen: scope.modelData
                color: "#0b1d6b"
                exclusionMode: ExclusionMode.Ignore
                WlrLayershell.layer: root.preview ? WlrLayer.Top : WlrLayer.Background
                WlrLayershell.namespace: "zen-greeter-sky"
                anchors {
                    top: true
                    bottom: true
                    left: true
                    right: true
                }

                Rectangle {
                    anchors.fill: parent
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0; color: "#0a1f86" }
                        GradientStop { position: 0.55; color: "#1747e6" }
                        GradientStop { position: 1; color: "#2f8dff" }
                    }
                }

                Blob {
                    id: b1
                    tint: "#63c6ff"
                    size: sky.height * 1.1
                    x: sky.width * 0.55
                    y: -sky.height * 0.25
                    opacity: 0.75
                }
                Drift { target: b1; dx: -sky.width * 0.12; dy: sky.height * 0.1; duration: 19000 }

                Blob {
                    id: b2
                    tint: "#0a14a8"
                    size: sky.height * 1.2
                    x: -sky.width * 0.2
                    y: sky.height * 0.25
                    opacity: 0.9
                }
                Drift { target: b2; dx: sky.width * 0.1; dy: -sky.height * 0.08; duration: 23000 }

                Blob {
                    id: b3
                    tint: "#7a5cff"
                    size: sky.height * 0.8
                    x: sky.width * 0.25
                    y: sky.height * 0.55
                    opacity: 0.45
                }
                Drift { target: b3; dx: sky.width * 0.16; dy: -sky.height * 0.12; duration: 27000 }

                Rectangle {
                    anchors.fill: parent
                    color: "black"
                    opacity: root.leaving ? 1 : 0

                    Behavior on opacity {
                        NumberAnimation { duration: 600; easing.type: Easing.InCubic }
                    }
                }
            }

            PanelWindow {
                id: glass

                screen: scope.modelData
                visible: scope.primary
                color: "transparent"
                exclusionMode: ExclusionMode.Ignore
                WlrLayershell.layer: root.preview ? WlrLayer.Overlay : WlrLayer.Top
                WlrLayershell.namespace: "zen-greeter-glass"
                WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
                anchors {
                    top: true
                    bottom: true
                    left: true
                    right: true
                }

                Item {
                    id: stage

                    anchors.fill: parent
                    focus: true
                    opacity: root.leaving ? 0 : 1

                    Behavior on opacity {
                        NumberAnimation { duration: 450 }
                    }

                    Keys.onPressed: event => {
                        if (!root.awake) {
                            root.wake();
                            if (event.text.length > 0 && event.key !== Qt.Key_Escape && event.key !== Qt.Key_Return && event.key !== Qt.Key_Space)
                                field.text = event.text;
                            field.forceActiveFocus();
                            event.accepted = true;
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        enabled: !root.awake
                        onClicked: {
                            root.wake();
                            field.forceActiveFocus();
                        }
                    }

                    // word
                    Item {
                        id: wordBox

                        readonly property real small: 0.42

                        width: parent.width
                        height: pen.height
                        y: root.awake ? glass.height * 0.13 - height / 2 : (glass.height - height) / 2 - glass.height * 0.03
                        scale: root.awake ? small : 1

                        Behavior on y {
                            NumberAnimation { duration: 800; easing.type: Easing.OutCubic }
                        }
                        Behavior on scale {
                            NumberAnimation { duration: 800; easing.type: Easing.OutCubic }
                        }

                        Canvas {
                            id: pen

                            property string word: root.words[root.wordIndex]
                            property real progress: 0
                            readonly property real unit: glass.height * 0.2 / 21
                            readonly property real thick: unit * 3.9
                            readonly property var shape: build(word)

                            x: (parent.width - width) / 2
                            width: shape.width * unit + thick * 4
                            height: (shape.maxY - shape.minY) * unit + thick * 4
                            renderStrategy: Canvas.Cooperative

                            onProgressChanged: requestPaint()
                            onShapeChanged: requestPaint()
                            onWordChanged: if (!root.awake) write.restart()

                            function build(word) {
                                const strokes = [];
                                let x = 0, total = 0, minY = 1e9, maxY = -1e9, last = null;
                                for (const ch of word) {
                                    const g = Script.glyphs[ch];
                                    if (!g) {
                                        x += 10;
                                        continue;
                                    }
                                    const ox = x - g[0];
                                    for (const s of g[2]) {
                                        let pts = [];
                                        for (let i = 0; i + 1 < s.length; i += 2) {
                                            pts.push([s[i] + ox, s[i + 1]]);
                                            minY = Math.min(minY, s[i + 1]);
                                            maxY = Math.max(maxY, s[i + 1]);
                                        }
                                        pts = smooth(smooth(smooth(pts)));
                                        const joined = last !== null && Math.abs(last[0] - pts[0][0]) < 0.01 && Math.abs(last[1] - pts[0][1]) < 0.01;
                                        let len = 0;
                                        for (let i = 1; i < pts.length; i++)
                                            len += Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
                                        strokes.push({ pts: pts, len: len, joined: joined });
                                        total += len;
                                        last = pts[pts.length - 1];
                                    }
                                    x += g[1] - g[0];
                                }
                                return { strokes: strokes, total: total, width: x, minY: minY, maxY: maxY };
                            }

                            function smooth(pts) {
                                if (pts.length < 3)
                                    return pts;
                                const out = [pts[0]];
                                for (let i = 0; i + 1 < pts.length; i++) {
                                    const a = pts[i], b = pts[i + 1];
                                    out.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25]);
                                    out.push([a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
                                }
                                out.push(pts[pts.length - 1]);
                                return out;
                            }

                            function trace(ctx, upto) {
                                const u = unit, ox = thick * 2, oy = thick * 2 - shape.minY * u;
                                let left = upto, tip = null;
                                ctx.beginPath();
                                for (const s of shape.strokes) {
                                    if (left <= 0)
                                        break;
                                    const p0 = s.pts[0];
                                    if (!s.joined)
                                        ctx.moveTo(ox + p0[0] * u, oy + p0[1] * u);
                                    for (let i = 1; i < s.pts.length; i++) {
                                        const a = s.pts[i - 1], b = s.pts[i];
                                        const seg = Math.hypot(b[0] - a[0], b[1] - a[1]);
                                        if (seg >= left) {
                                            const t = left / seg;
                                            tip = [ox + (a[0] + (b[0] - a[0]) * t) * u, oy + (a[1] + (b[1] - a[1]) * t) * u];
                                            ctx.lineTo(tip[0], tip[1]);
                                            left = 0;
                                            break;
                                        }
                                        ctx.lineTo(ox + b[0] * u, oy + b[1] * u);
                                        left -= seg;
                                    }
                                }
                                return tip;
                            }

                            function pass(ctx, upto, dx, dy, width, style, op) {
                                ctx.save();
                                ctx.translate(dx, dy);
                                ctx.globalCompositeOperation = op || "source-over";
                                ctx.lineCap = "round";
                                ctx.lineJoin = "round";
                                ctx.lineWidth = width;
                                ctx.strokeStyle = style;
                                const tip = trace(ctx, upto);
                                ctx.stroke();
                                ctx.restore();
                                return tip;
                            }

                            onPaint: {
                                const ctx = getContext("2d");
                                ctx.reset();
                                const upto = shape.total * progress;
                                if (upto <= 0)
                                    return;
                                const w = thick;
                                const sheen = ctx.createLinearGradient(0, thick * 2, 0, height - thick * 2);
                                sheen.addColorStop(0, "rgba(255,255,255,0.5)");
                                sheen.addColorStop(1, "rgba(200,225,255,0.2)");
                                const tip = pass(ctx, upto, 0, 0, w, "rgba(255,255,255,0.8)");
                                pass(ctx, upto, 0, 0, w - 3, "rgba(0,0,0,1)", "destination-out");
                                pass(ctx, upto, 0, 0, w - 3, sheen);
                                pass(ctx, upto, w * 0.06, w * 0.26, w * 0.16, "rgba(6,18,90,0.2)");
                                pass(ctx, upto, -w * 0.06, -w * 0.2, w * 0.22, "rgba(255,255,255,0.32)");
                                pass(ctx, upto, -w * 0.07, -w * 0.24, w * 0.08, "rgba(255,255,255,0.7)");
                                if (tip && progress < 1) {
                                    const glow = ctx.createRadialGradient(tip[0], tip[1], 0, tip[0], tip[1], w * 1.4);
                                    glow.addColorStop(0, "rgba(255,255,255,0.85)");
                                    glow.addColorStop(1, "rgba(255,255,255,0)");
                                    ctx.fillStyle = glow;
                                    ctx.fillRect(tip[0] - w * 1.5, tip[1] - w * 1.5, w * 3, w * 3);
                                }
                            }

                            SequentialAnimation {
                                id: write
                                running: true

                                PropertyAction { target: pen; property: "opacity"; value: 1 }
                                PropertyAction { target: pen; property: "progress"; value: 0 }
                                NumberAnimation {
                                    target: pen
                                    property: "progress"
                                    to: 1
                                    duration: Math.max(1500, Math.min(3000, pen.shape.total * 9))
                                    easing.type: Easing.InOutSine
                                }
                                PauseAnimation { duration: 1800 }
                                ScriptAction { script: if (root.awake) write.stop() }
                                NumberAnimation { target: pen; property: "opacity"; to: 0; duration: 700; easing.type: Easing.InQuad }
                                ScriptAction { script: root.wordIndex = (root.wordIndex + 1) % root.words.length }
                            }

                            Connections {
                                target: root
                                function onAwakeChanged() {
                                    if (root.awake && pen.opacity < 1) {
                                        write.stop();
                                        pen.opacity = 1;
                                        pen.progress = 1;
                                    }
                                }
                            }
                        }
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: glass.height * 0.08
                        text: "press any key"
                        color: Qt.rgba(1, 1, 1, 0.7)
                        font.family: root.sansFamily
                        font.pixelSize: 16
                        font.letterSpacing: 1.5
                        opacity: root.awake ? 0 : 1

                        Behavior on opacity {
                            NumberAnimation { duration: 300 }
                        }
                    }

                    // card
                    Rectangle {
                        id: card

                        property real nudge: 0

                        width: 400
                        height: column.implicitHeight + 56
                        x: (parent.width - width) / 2 + nudge
                        y: glass.height * 0.36 + (root.awake ? 0 : 40)
                        radius: 32
                        color: Qt.rgba(1, 1, 1, 0.1)
                        border.color: Qt.rgba(1, 1, 1, 0.32)
                        border.width: 1
                        opacity: root.awake ? 1 : 0
                        visible: opacity > 0

                        Behavior on opacity {
                            NumberAnimation { duration: 500; easing.type: Easing.OutCubic }
                        }
                        Behavior on y {
                            NumberAnimation { duration: 600; easing.type: Easing.OutCubic }
                        }

                        SequentialAnimation {
                            id: shakeAnim
                            NumberAnimation { target: card; property: "nudge"; to: -14; duration: 50 }
                            NumberAnimation { target: card; property: "nudge"; to: 12; duration: 70 }
                            NumberAnimation { target: card; property: "nudge"; to: -8; duration: 60 }
                            NumberAnimation { target: card; property: "nudge"; to: 4; duration: 50 }
                            NumberAnimation { target: card; property: "nudge"; to: 0; duration: 40 }
                        }

                        Connections {
                            target: root
                            function onShake() {
                                shakeAnim.restart();
                                field.text = "";
                                field.forceActiveFocus();
                            }
                        }

                        Column {
                            id: column

                            anchors.centerIn: parent
                            width: parent.width - 56
                            spacing: 18

                            Rectangle {
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: 96
                                height: 96
                                radius: 48
                                color: Qt.rgba(1, 1, 1, 0.2)
                                border.color: Qt.rgba(1, 1, 1, 0.45)
                                border.width: 1
                                clip: true

                                Text {
                                    anchors.centerIn: parent
                                    visible: face.status !== Image.Ready
                                    text: root.current ? root.current.display.charAt(0).toUpperCase() : "?"
                                    color: "white"
                                    font.family: root.sansFamily
                                    font.pixelSize: 42
                                    font.weight: 600
                                }

                                Image {
                                    id: face
                                    anchors.fill: parent
                                    fillMode: Image.PreserveAspectCrop
                                    source: root.current ? "file:///var/lib/AccountsService/icons/" + root.current.name : ""
                                    asynchronous: true
                                }
                            }

                            Row {
                                anchors.horizontalCenter: parent.horizontalCenter
                                spacing: 14

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: root.users.length > 1
                                    text: "‹"
                                    color: Qt.rgba(1, 1, 1, 0.7)
                                    font.pixelSize: 28

                                    MouseArea {
                                        anchors.fill: parent
                                        anchors.margins: -8
                                        onClicked: root.cycleUser(-1)
                                    }
                                }

                                Text {
                                    text: root.current ? root.current.display : "no users found"
                                    color: "white"
                                    font.family: root.sansFamily
                                    font.pixelSize: 24
                                    font.weight: 600
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: root.users.length > 1
                                    text: "›"
                                    color: Qt.rgba(1, 1, 1, 0.7)
                                    font.pixelSize: 28

                                    MouseArea {
                                        anchors.fill: parent
                                        anchors.margins: -8
                                        onClicked: root.cycleUser(1)
                                    }
                                }
                            }

                            Rectangle {
                                width: parent.width
                                height: 52
                                radius: 26
                                color: Qt.rgba(0, 0.05, 0.25, 0.28)
                                border.color: field.activeFocus ? Qt.rgba(1, 1, 1, 0.65) : Qt.rgba(1, 1, 1, 0.3)
                                border.width: 1

                                TextInput {
                                    id: field

                                    anchors.fill: parent
                                    anchors.leftMargin: 22
                                    anchors.rightMargin: 52
                                    verticalAlignment: TextInput.AlignVCenter
                                    echoMode: TextInput.Password
                                    passwordCharacter: "●"
                                    color: "white"
                                    selectionColor: Qt.rgba(1, 1, 1, 0.35)
                                    font.family: root.sansFamily
                                    font.pixelSize: 18
                                    font.letterSpacing: 3
                                    enabled: !root.busy
                                    clip: true

                                    Keys.onReturnPressed: root.login(text)
                                    Keys.onEnterPressed: root.login(text)
                                    Keys.onPressed: event => {
                                        if (event.key === Qt.Key_Escape) {
                                            text = "";
                                            event.accepted = true;
                                        } else if (event.key === Qt.Key_Tab) {
                                            root.cycleUser(event.modifiers & Qt.ShiftModifier ? -1 : 1);
                                            event.accepted = true;
                                        }
                                    }
                                }

                                Text {
                                    anchors.left: parent.left
                                    anchors.leftMargin: 22
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: field.text.length === 0
                                    text: root.preview ? "Preview: any password" : "Password"
                                    color: Qt.rgba(1, 1, 1, 0.55)
                                    font.family: root.sansFamily
                                    font.pixelSize: 17
                                }

                                Rectangle {
                                    anchors.right: parent.right
                                    anchors.rightMargin: 7
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 38
                                    height: 38
                                    radius: 19
                                    color: field.text.length > 0 ? Qt.rgba(1, 1, 1, 0.9) : Qt.rgba(1, 1, 1, 0.18)

                                    Behavior on color {
                                        ColorAnimation { duration: 150 }
                                    }

                                    Text {
                                        anchors.centerIn: parent
                                        text: root.busy ? "…" : "→"
                                        color: field.text.length > 0 ? "#1a3fd0" : "white"
                                        font.pixelSize: 20
                                        font.weight: 700
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.login(field.text)
                                    }
                                }
                            }

                            Text {
                                width: parent.width
                                horizontalAlignment: Text.AlignHCenter
                                wrapMode: Text.Wrap
                                text: root.status
                                visible: text.length > 0
                                color: "#ffd2dc"
                                font.family: root.sansFamily
                                font.pixelSize: 15
                            }
                        }
                    }

                    // power
                    Row {
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: 28
                        spacing: 10
                        opacity: root.awake ? 1 : 0

                        Behavior on opacity {
                            NumberAnimation { duration: 400 }
                        }

                        Pill {
                            label: "Restart"
                            onClicked: root.powerAction("reboot")
                        }
                        Pill {
                            label: "Shut down"
                            onClicked: root.powerAction("poweroff")
                        }
                    }

                    Text {
                        anchors.left: parent.left
                        anchors.bottom: parent.bottom
                        anchors.margins: 32
                        text: "ZEN"
                        color: Qt.rgba(1, 1, 1, 0.75)
                        font.family: root.sansFamily
                        font.pixelSize: 15
                        font.weight: 700
                        font.letterSpacing: 4
                        opacity: root.awake ? 1 : 0

                        Behavior on opacity {
                            NumberAnimation { duration: 400 }
                        }
                    }
                }
            }
        }
    }
}
