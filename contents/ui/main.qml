import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import QtQuick.Shapes
import org.kde.plasma.plasmoid
import org.kde.plasma.plasma5support as Plasma5Support

PlasmoidItem {
    id: root

    implicitWidth: 390
    implicitHeight: 560

    property var devices: []
    property string backendState: "Loading"
    property string errorText: ""
    property string currentCommand: ""
    property bool connected: false
    property bool busy: false
    property bool hasStatus: false
    readonly property string statusCommand: "tailscale status --json"
    readonly property color surfaceColor: "#303030"
    readonly property color insetColor: "#242424"
    readonly property color borderColor: "#4a4a4a"
    readonly property color primaryTextColor: "#f2f2f2"
    readonly property color secondaryTextColor: "#b8b8b8"
    readonly property color onlineColor: "#63c783"

    function runCommand(command) {
        if (busy) {
            return
        }
        errorText = ""
        currentCommand = command
        busy = true
        commandSource.connectSource(command)
    }

    function refreshStatus() {
        runCommand(statusCommand)
    }

    function toggleConnection() {
        runCommand("pkexec tailscale " + (connected ? "down" : "up"))
    }

    function parseStatus(rawStatus) {
        const status = JSON.parse(rawStatus)
        backendState = status.BackendState || "Unbekannt"
        connected = backendState === "Running"

        const result = []
        if (status.Self) {
            result.push({
                name: status.Self.HostName || status.Self.DNSName || "This device",
                address: (status.Self.TailscaleIPs || []).join(", "),
                online: true,
                self: true
            })
        }
        const peers = status.Peer || {}
        Object.keys(peers).forEach(function (key) {
            const peer = peers[key]
            result.push({
                name: peer.HostName || peer.DNSName || key,
                address: (peer.TailscaleIPs || []).join(", "),
                online: peer.Online === true,
                self: false
            })
        })
        result.sort(function (first, second) {
            if (first.online !== second.online) {
                return first.online ? -1 : 1
            }
            return first.name.localeCompare(second.name)
        })
        devices = result
        hasStatus = true
    }

    Plasma5Support.DataSource {
        id: commandSource
        engine: "executable"
        connectedSources: []

        onNewData: function (sourceName, data) {
            disconnectSource(sourceName)
            if (sourceName !== root.currentCommand) {
                return
            }

            const exitCode = Number(data["exit code"])
            if (exitCode !== 0) {
                root.errorText = (data.stderr || "Command failed.").trim()
                root.busy = false
                root.currentCommand = ""
                return
            }

            if (sourceName === root.statusCommand) {
                try {
                    root.parseStatus(data.stdout || "")
                    root.errorText = ""
                } catch (error) {
                    root.errorText = "Could not read Tailscale status data."
                }
                root.busy = false
                root.currentCommand = ""
                return
            }

            root.currentCommand = ""
            root.busy = false
            root.refreshStatus()
        }
    }

    Timer {
        interval: 5000
        running: true
        repeat: true
        onTriggered: root.refreshStatus()
    }

    Component.onCompleted: refreshStatus()

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: root.surfaceColor
        border.color: root.borderColor
        border.width: 1

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 3

                    Controls.Label {
                        text: "Tailscale"
                        font.pixelSize: 22
                        font.weight: Font.DemiBold
                        color: root.primaryTextColor
                    }

                    Controls.Label {
                        text: root.busy ? "Working…" : root.backendState
                        color: root.connected ? root.onlineColor : root.secondaryTextColor
                    }
                }

                Controls.Button {
                    text: root.connected ? "Turn off" : "Turn on"
                    enabled: root.hasStatus && !root.busy
                    palette.button: "#424242"
                    palette.buttonText: root.primaryTextColor
                    onClicked: root.toggleConnection()
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Controls.Label {
                    Layout.fillWidth: true
                    visible: root.connected
                    text: root.devices.length + " devices"
                    font.weight: Font.DemiBold
                    color: root.primaryTextColor
                }

            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: root.hasStatus && !root.connected
                spacing: 10

                Item {
                    Layout.fillHeight: true
                }

                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 56

                    Shape {
                        anchors.centerIn: parent
                        width: 56
                        height: 56

                        ShapePath {
                            strokeColor: "#929292"
                            strokeWidth: 2
                            fillColor: "transparent"
                            capStyle: ShapePath.RoundCap
                            joinStyle: ShapePath.RoundJoin
                            startX: 28
                            startY: 5
                            PathLine { x: 52; y: 49 }
                            PathLine { x: 4; y: 49 }
                            PathLine { x: 28; y: 5 }
                        }

                        ShapePath {
                            strokeColor: "#929292"
                            strokeWidth: 2.5
                            fillColor: "transparent"
                            capStyle: ShapePath.RoundCap
                            startX: 28
                            startY: 19
                            PathLine { x: 28; y: 34 }
                        }

                        ShapePath {
                            strokeColor: "#929292"
                            strokeWidth: 3
                            fillColor: "transparent"
                            capStyle: ShapePath.RoundCap
                            startX: 28
                            startY: 41
                            PathLine { x: 28; y: 41.1 }
                        }
                    }
                }

                Controls.Label {
                    Layout.fillWidth: true
                    text: "Tailscale is offline"
                    font.pixelSize: 18
                    color: root.primaryTextColor
                    horizontalAlignment: Text.AlignHCenter
                }

                Item {
                    Layout.fillHeight: true
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: !root.hasStatus || root.connected
                radius: 6
                color: root.insetColor

                ListView {
                    id: deviceList
                    anchors.fill: parent
                    anchors.margins: 4
                    clip: true
                    model: root.devices
                    spacing: 1

                    delegate: Item {
                        required property var modelData
                        width: deviceList.width
                        height: 58

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 10

                            Rectangle {
                                Layout.preferredWidth: 9
                                Layout.preferredHeight: 9
                                radius: 5
                                color: modelData.online ? root.onlineColor : "#858585"
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: modelData.name + (modelData.self ? " (this device)" : "")
                                    elide: Text.ElideRight
                                    color: root.primaryTextColor
                                }

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: modelData.address || (modelData.online ? "Online" : "Offline")
                                    elide: Text.ElideRight
                                    font.pixelSize: 11
                                    color: root.secondaryTextColor
                                }
                            }

                            Controls.Label {
                                text: modelData.online ? "Online" : "Offline"
                                font.pixelSize: 11
                                color: modelData.online ? root.onlineColor : root.secondaryTextColor
                            }
                        }
                    }

                    Controls.Label {
                        anchors.centerIn: parent
                        visible: root.hasStatus && root.devices.length === 0
                        text: "No devices found"
                        color: root.secondaryTextColor
                    }
                }
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: root.errorText.length > 0
                text: root.errorText
                wrapMode: Text.Wrap
                color: "#ff8a80"
            }
        }
    }
}