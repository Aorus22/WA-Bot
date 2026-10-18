import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../../ui"

Rectangle {
    id: root
    color: Theme.glass(Theme.background)

    property var editSpec: ({})
    property string editValue: ""

    function openEdit(field) {
        editSpec = System.editSpec(field)
        editValue = String(editSpec.value || "")
        editDialog.open()
    }
    function saveEdit() {
        System.commitEdit(String(editSpec.field || ""), editValue)
        editDialog.close()
    }

    Component.onCompleted: System.refresh()

    component SettingsCard: Rectangle {
        id: card
        property string title
        property bool showTitle: true
        property real contentSpacing: 14
        default property alias content: body.data
        radius: Theme.rem * 0.625
        color: Theme.glass(Theme.card)
        border.width: 1
        border.color: Theme.alpha(Theme.border, 0.5)
        clip: true
        implicitHeight: body.implicitHeight + 32

        ColumnLayout {
            id: body
            anchors.fill: parent
            anchors.margins: 16
            spacing: card.contentSpacing
            AppText { visible: card.showTitle; Layout.fillWidth: true; text: card.title; size: 1; semibold: true }
        }
    }

    Flickable {
        id: scroll
        anchors.fill: parent
        clip: true
        contentWidth: width
        contentHeight: content.implicitHeight + 48
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ThinScrollBar {}

        ColumnLayout {
            id: content
            x: (scroll.width - width) / 2
            y: 40
            width: Math.min(672, scroll.width - 48)
            spacing: 16

            ColumnLayout {
                Layout.fillWidth: true
                Layout.bottomMargin: 16
                spacing: 10
                AppText { Layout.fillWidth: true; Layout.topMargin: 2; text: "Settings"; size: 1.5; semibold: true }
                AppText { Layout.fillWidth: true; text: "Customize your application appearance, credentials, and bot configurations"; size: 0.75; color: Theme.mutedForeground }
            }

            AppText { Layout.fillWidth: true; text: "APPEARANCE"; size: 0.6875; semibold: true; color: Theme.mutedForeground }

            SettingsCard {
                Layout.fillWidth: true
                title: "Appearance"
                showTitle: false
                contentSpacing: 0
                color: "transparent"

                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        RowLayout {
                            spacing: 12
                            Icon { name: "inbox"; size: 18; color: Theme.mutedForeground }
                            AppText { text: "Theme Mode"; size: 0.875; medium: true }
                        }
                        AppText { text: "Filter by dark or light appearance"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Item { Layout.fillWidth: true }
                    Button {
                        id: themeModeButton
                        text: System.themeModeFilter === "dark" ? "Dark" : System.themeModeFilter === "light" ? "Light" : "All themes"
                        Layout.preferredWidth: 110
                        Layout.preferredHeight: 32
                        padding: 0
                        leftPadding: 12
                        rightPadding: 10
                        Accessible.name: "Theme mode filter: " + text
                        onClicked: themeModeMenu.popupAt(themeModeButton, true, false)
                        background: Rectangle {
                            radius: Theme.rem * 0.375
                            color: themeModeButton.hovered ? Theme.alpha(Theme.muted, 0.65) : Theme.muted
                            border.width: 1
                            border.color: Theme.border
                        }
                        contentItem: RowLayout {
                            spacing: 8
                            AppText {
                                Layout.fillWidth: true
                                text: themeModeButton.text
                                size: 0.75
                                color: Theme.foreground
                                elide: Text.ElideRight
                            }
                            Icon { name: "chevron-down"; size: 14; color: Theme.mutedForeground }
                        }
                    }
                }

                AppMenu {
                    id: themeModeMenu
                    model: [
                        { label: "All themes", action: "all" },
                        { label: "Dark", action: "dark" },
                        { label: "Light", action: "light" }
                    ]
                    onTriggered: item => System.setThemeModeFilter(String(item.action || "all"))
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        RowLayout {
                            spacing: 12
                            Icon { name: "eye"; size: 18; color: Theme.mutedForeground }
                            AppText { text: "Window transparency"; size: 0.875; medium: true }
                        }
                        AppText { text: "Adjust how much of the desktop shows through"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Item { Layout.fillWidth: true }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        Layout.preferredWidth: 280
                        Layout.minimumWidth: 280
                        Layout.maximumWidth: 280
                        spacing: 12
                        AppText { Layout.preferredWidth: 42; horizontalAlignment: Text.AlignRight; text: String(System.windowTransparency) + "%"; size: 0.75; color: Theme.mutedForeground }
                        AppSlider {
                            Layout.fillWidth: true
                            Layout.preferredWidth: 220
                            from: 0; to: 100; stepSize: 1; value: System.windowTransparency
                            onMoved: Theme.setTransparency(Math.round(value), false)
                            onPressedChanged: if (!pressed) Theme.setTransparency(Math.round(value), true)
                        }
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        AppText { text: "Interface text size"; size: 0.875; medium: true }
                        AppText { text: "Scales menus, lists, headers and settings"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Item { Layout.fillWidth: true }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        Layout.preferredWidth: 280
                        Layout.minimumWidth: 280
                        Layout.maximumWidth: 280
                        spacing: 12
                        AppText { Layout.preferredWidth: 42; horizontalAlignment: Text.AlignRight; text: String(System.uiFontScale) + "%"; size: 0.75; color: Theme.mutedForeground }
                        AppSlider {
                            Layout.fillWidth: true
                            Layout.preferredWidth: 220
                            from: 75; to: 150; stepSize: 5; value: System.uiFontScale
                            onMoved: Theme.setUiScale(Math.round(value), false)
                            onPressedChanged: if (!pressed) Theme.setUiScale(Math.round(value), true)
                        }
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        AppText { text: "Chat text size"; size: 0.875; medium: true }
                        AppText { text: "Scales the text inside message bubbles only"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Item { Layout.fillWidth: true }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        Layout.preferredWidth: 280
                        Layout.minimumWidth: 280
                        Layout.maximumWidth: 280
                        spacing: 12
                        AppText { Layout.preferredWidth: 42; horizontalAlignment: Text.AlignRight; text: String(System.chatFontScale) + "%"; size: 0.75; color: Theme.mutedForeground }
                        AppSlider {
                            Layout.fillWidth: true
                            Layout.preferredWidth: 220
                            from: 75; to: 150; stepSize: 5; value: System.chatFontScale
                            onMoved: Theme.setChatScale(Math.round(value), false)
                            onPressedChanged: if (!pressed) Theme.setChatScale(Math.round(value), true)
                        }
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        RowLayout {
                            spacing: 12
                            Icon { name: "eye"; size: 18; color: Theme.mutedForeground }
                            AppText { text: "Backdrop blur"; size: 0.875; medium: true }
                        }
                        AppText { text: "Blur strength is controlled by COSMIC"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Item { Layout.fillWidth: true }
                    AppSwitch { checked: System.windowBlurEnabled; gpuiStyle: true; onToggled: System.setWindowBlurEnabled(value) }
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        RowLayout {
                            spacing: 12
                            Icon { name: "panel-left"; size: 18; color: Theme.mutedForeground }
                            AppText { text: "Chat Layout"; size: 0.875; medium: true }
                        }
                        AppText { text: "Auto: pages in the popover, sidebar in a window"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Item { Layout.fillWidth: true }
                    Row {
                        spacing: 4
                        Repeater {
                            model: [{ value: "auto", label: "Auto" }, { value: "pages", label: "Detail" }, { value: "sidebar", label: "Sidebar" }]
                            delegate: Btn {
                                required property var modelData
                                text: modelData.label
                                size: 0.75
                                variant: System.chatLayout === modelData.value ? "primary" : "secondary"
                                onClicked: System.setChatLayout(modelData.value)
                            }
                        }
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        RowLayout {
                            spacing: 12
                            Icon { name: "bell"; size: 18; color: Theme.mutedForeground }
                            AppText { text: "Notifications"; size: 0.875; medium: true }
                        }
                        AppText { text: "Show a desktop notification for new messages while the app is not focused"; size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                    }
                    Item { Layout.fillWidth: true }
                    AppSwitch { checked: System.notificationsEnabled; gpuiStyle: true; onToggled: System.setNotificationsEnabled(value) }
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        RowLayout {
                            spacing: 12
                            Icon { name: "file-text"; size: 18; color: Theme.mutedForeground }
                            AppText { text: "Format message text"; size: 0.875; medium: true }
                        }
                        AppText { text: "Show *bold*, _italic_, ~strikethrough~ and `monospace` like WhatsApp"; size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                    }
                    Item { Layout.fillWidth: true }
                    AppSwitch { checked: System.formatMessages; gpuiStyle: true; onToggled: System.setFormatMessages(value) }
                }

                Rectangle { Layout.fillWidth: true; Layout.leftMargin: -16; Layout.rightMargin: -16; Layout.preferredHeight: 13; color: "transparent"; Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border } }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 16
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        AppText { text: "Color Theme"; size: 0.875; medium: true }
                        AppText { text: String(System.presetList(System.themeModeFilter, "").length) + " themes available"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 12
                        Repeater {
                            model: System.presetList(System.themeModeFilter, "")
                            delegate: Item {
                                required property var modelData
                                width: 204
                                height: 72
                                Rectangle {
                                    anchors.fill: parent
                                    radius: 8
                                    color: modelData.bg
                                    border.width: System.activePreset === modelData.name ? 2 : 1
                                    border.color: System.activePreset === modelData.name ? Theme.primary : Theme.alpha(Theme.border, 0.8)
                                    Column {
                                        anchors.fill: parent
                                        anchors.margins: 8
                                        spacing: 6
                                        Row {
                                            spacing: 4
                                            Repeater {
                                                model: [modelData.primary, modelData.accent, modelData.destructive]
                                                delegate: Rectangle { required property var modelData; width: 12; height: 12; radius: 6; color: modelData }
                                            }
                                        }
                                        AppText { width: parent.width; text: String(modelData.label || modelData.name); size: 0.6875; medium: true; color: modelData.fg; elide: Text.ElideRight }
                                    }
                                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: System.applyPreset(modelData.name) }
                                }
                            }
                        }
                    }
                }
            }

            SettingsCard {
                Layout.fillWidth: true
                title: "Account and privacy"
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        AppText { text: "WhatsApp connection"; size: 0.8125; medium: true }
                        AppText { text: AppState.auth === "authenticated" ? "Connected · " + AppState.phone : "Not connected"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Btn { text: "Log out"; variant: "outline"; enabled: AppState.auth === "authenticated"; onClicked: logoutDialog.open() }
                }
                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: "Read receipts"; size: 0.8125 }
                    AppSwitch { checked: System.readReceipts; gpuiStyle: true; enabled: !System.savingReceipts; onToggled: System.toggleReadReceipts() }
                }
                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: "Contact name"; size: 0.8125 }
                    ComboBox {
                        model: [{ value: "profile", label: "Profile name" }, { value: "saved", label: "Saved name" }]
                        textRole: "label"
                        currentIndex: System.contactNameMode === "saved" ? 1 : 0
                        onActivated: System.setContactNameMode(model[currentIndex].value)
                    }
                }
            }

            SettingsCard {
                Layout.fillWidth: true
                title: "AI configuration"
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        AppText { text: "Gemini API key"; size: 0.8125; medium: true }
                        AppText { text: System.hasGeminiKey ? "Key saved" : "No key saved"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Btn { text: "Edit"; variant: "outline"; onClicked: root.openEdit("gemini_key") }
                }
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        AppText { text: "AI server URL"; size: 0.8125; medium: true }
                        AppText { text: System.aiServerUrl; size: 0.75; color: Theme.mutedForeground; elide: Text.ElideRight }
                    }
                    Btn { text: "Edit"; variant: "outline"; onClicked: root.openEdit("ai_url") }
                }
                Btn { text: "Save AI settings"; variant: "secondary"; loading: System.savingAi; enabled: !System.savingAi; onClicked: System.saveAi() }
            }

            SettingsCard {
                Layout.fillWidth: true
                title: "Call text-to-speech"
                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: "Provider"; size: 0.8125 }
                    ComboBox {
                        model: [{ value: "", label: "Disabled" }, { value: "edge", label: "Edge TTS" }, { value: "fish", label: "Fish Audio" }]
                        textRole: "label"
                        currentIndex: System.ttsProvider === "edge" ? 1 : System.ttsProvider === "fish" ? 2 : 0
                        onActivated: System.setTtsProvider(model[currentIndex].value)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        AppText { text: "Default voice"; size: 0.8125; medium: true }
                        AppText { text: System.ttsDefaultVoice || "Not set"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Btn { text: "Edit"; variant: "outline"; onClicked: root.openEdit("tts_voice") }
                }
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        AppText { text: "Fish Audio API key"; size: 0.8125; medium: true }
                        AppText { text: System.hasFishKey ? "Key saved" : "No key saved"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Btn { text: "Edit"; variant: "outline"; onClicked: root.openEdit("fish_key") }
                }
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        AppText { text: "Fish Audio model"; size: 0.8125; medium: true }
                        AppText { text: System.fishAudioModel || "Not set"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Btn { text: "Edit"; variant: "outline"; onClicked: root.openEdit("fish_model") }
                }
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        AppText { text: "Fish Audio voice ID"; size: 0.8125; medium: true }
                        AppText { text: System.fishAudioVoiceId || "Not set"; size: 0.75; color: Theme.mutedForeground }
                    }
                    Btn { text: "Edit"; variant: "outline"; onClicked: root.openEdit("fish_voice_id") }
                }
                Btn { text: "Save TTS settings"; variant: "secondary"; loading: System.savingTts; enabled: !System.savingTts; onClicked: System.saveTts() }
            }

            SettingsCard {
                Layout.fillWidth: true
                title: "Chat history sync"
                AppText { Layout.fillWidth: true; text: String(System.historyCard.finishedText || ""); size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: "Sync depth"; size: 0.8125; medium: true }
                    ComboBox {
                        model: System.historyCard.depths || []
                        textRole: "label"
                        currentIndex: {
                            const ds = System.historyCard.depths || []
                            return Math.max(0, ds.findIndex(d => d.selected))
                        }
                        enabled: !System.historyCard.active
                        onActivated: System.setHistoryDepth(model[currentIndex].id)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: String(System.historyCard.hint || ""); size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                            AppSwitch { checked: Boolean(System.historyCard.retryFinished); gpuiStyle: true; enabled: !System.historyCard.active; onToggled: System.toggleRetryFinished() }
                }
                AppText { Layout.fillWidth: true; visible: Boolean(System.historyCard.detail); text: String(System.historyCard.headline || ""); size: 0.8125; semibold: true }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 7
                    visible: Boolean(System.historyCard.showBar)
                    radius: 4
                    color: Theme.muted
                    Rectangle { width: parent.width * Number(System.historyCard.fraction || 0); height: parent.height; radius: 4; color: Theme.primary }
                }
                AppText { Layout.fillWidth: true; visible: String(System.historyCard.timing || "") !== ""; text: String(System.historyCard.timing || ""); size: 0.6875; color: Theme.mutedForeground }
                Repeater {
                    model: System.historyCard.now || []
                    delegate: AppText { required property var modelData; Layout.fillWidth: true; text: String(modelData); size: 0.6875; color: Theme.mutedForeground }
                }
                AppText { Layout.fillWidth: true; visible: String(System.historyCard.paused || "") !== ""; text: String(System.historyCard.paused || ""); size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                AppText { Layout.fillWidth: true; visible: String(System.historyCard.result || "") !== ""; text: String(System.historyCard.result || ""); size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                AppText { Layout.fillWidth: true; visible: String(System.historyCard.staged || "") !== ""; text: String(System.historyCard.staged || ""); size: 0.75; color: Theme.primary; wrapMode: Text.WordWrap }
                AppText { Layout.fillWidth: true; visible: String(System.historyCard.error || "") !== ""; text: String(System.historyCard.error || ""); size: 0.75; color: Theme.destructive; wrapMode: Text.WordWrap }
                RowLayout {
                    Layout.fillWidth: true
                    Item { Layout.fillWidth: true }
                    Btn { visible: System.historyCard.active; text: "Stop sync"; variant: "outline"; enabled: !System.historyCard.stopping; onClicked: System.cancelHistorySync() }
                    Btn { visible: !System.historyCard.active; text: System.historyCard.resumable ? "Resume sync" : "Start sync"; variant: "primary"; onClicked: System.startHistorySync(Boolean(System.historyCard.resumable)) }
                }
            }

            SettingsCard {
                Layout.fillWidth: true
                title: "History media download"
                AppText { Layout.fillWidth: true; text: String(System.mediaCard.chosenText || ""); size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                Flow {
                    Layout.fillWidth: true
                    spacing: 10
                    Repeater {
                        model: System.mediaCard.chips || []
                        delegate: Row {
                            required property var modelData
                            spacing: 5
                            AppSwitch { checked: Boolean(modelData.selected); gpuiStyle: true; enabled: !System.mediaCard.active; onToggled: System.toggleMediaKind(modelData.id) }
                            AppText { text: String(modelData.label || ""); size: 0.75; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: "Retry unavailable media"; size: 0.75 }
                    AppSwitch { checked: Boolean(System.mediaCard.retryUnavailable); gpuiStyle: true; enabled: !System.mediaCard.active; onToggled: System.toggleRetryUnavailable() }
                }
                RowLayout {
                    Layout.fillWidth: true
                    AppText { Layout.fillWidth: true; text: "Ask phone for missing media"; size: 0.75 }
                    AppSwitch { checked: Boolean(System.mediaCard.askPhone); gpuiStyle: true; enabled: !System.mediaCard.active; onToggled: System.toggleAskPhone() }
                }
                AppText { Layout.fillWidth: true; visible: String(System.mediaCard.usage || "") !== ""; text: String(System.mediaCard.usage || ""); size: 0.6875; color: Theme.mutedForeground }
                AppText { Layout.fillWidth: true; visible: String(System.mediaCard.headline || "") !== ""; text: String(System.mediaCard.headline || ""); size: 0.8125; semibold: true }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 7
                    visible: Boolean(System.mediaCard.showBar)
                    radius: 4
                    color: Theme.muted
                    Row {
                        anchors.fill: parent
                        Rectangle { width: parent.width * Number(System.mediaCard.done || 0); height: parent.height; color: Theme.primary; radius: 4 }
                        Rectangle { x: parent.width * Number(System.mediaCard.done || 0); width: parent.width * Number(System.mediaCard.gone || 0); height: parent.height; color: Theme.mutedForeground }
                        Rectangle { x: parent.width * (Number(System.mediaCard.done || 0) + Number(System.mediaCard.gone || 0)); width: parent.width * Number(System.mediaCard.failed || 0); height: parent.height; color: Theme.destructive; radius: 4 }
                    }
                }
                AppText { Layout.fillWidth: true; visible: String(System.mediaCard.timing || "") !== ""; text: String(System.mediaCard.timing || ""); size: 0.6875; color: Theme.mutedForeground }
                AppText { Layout.fillWidth: true; visible: String(System.mediaCard.paused || "") !== ""; text: String(System.mediaCard.paused || ""); size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WordWrap }
                AppText { Layout.fillWidth: true; visible: String(System.mediaCard.error || "") !== ""; text: String(System.mediaCard.error || ""); size: 0.75; color: Theme.destructive; wrapMode: Text.WordWrap }
                RowLayout {
                    Layout.fillWidth: true
                    Item { Layout.fillWidth: true }
                    Btn { visible: System.mediaCard.active; text: "Stop download"; variant: "outline"; enabled: !System.mediaCard.stopping; onClicked: System.cancelHistoryMedia() }
                    Btn { visible: !System.mediaCard.active; text: System.mediaCard.resumable ? "Resume download" : "Download media"; variant: "primary"; enabled: !System.mediaCard.nothingChosen || System.mediaCard.resumable; onClicked: System.startHistoryMedia(Boolean(System.mediaCard.resumable)) }
                }
            }

            SettingsCard {
                Layout.fillWidth: true
                title: "Connection"
                AppText { Layout.fillWidth: true; text: "Backend API · " + Backend.apiUrl; size: 0.75; color: Theme.mutedForeground; wrapMode: Text.WrapAnywhere }
                AppText { Layout.fillWidth: true; text: System.platformFramework + " · v" + System.appVersion; size: 0.6875; color: Theme.mutedForeground; wrapMode: Text.Wrap }
                AppText { Layout.fillWidth: true; text: Platform.trayAvailable ? "System tray available" : "No system tray host detected"; size: 0.6875; color: Theme.mutedForeground; wrapMode: Text.Wrap }
                RowLayout {
                    Layout.fillWidth: true
                    Item { Layout.fillWidth: true }
                    Btn { text: System.restartingBackend ? "Restarting…" : "Restart backend"; variant: "outline"; loading: System.restartingBackend; enabled: !System.restartingBackend; onClicked: System.restartBackend() }
                }
            }
        }
    }

    AppDialog {
        id: editDialog
        title: String(root.editSpec.title || "Edit setting")
        description: String(root.editSpec.description || "")
        cardWidth: 520
        AppTextField {
            id: editInput
            width: parent.width
            text: root.editValue
            echoMode: root.editSpec.secret ? TextInput.Password : TextInput.Normal
            placeholder: String(root.editSpec.title || "Value")
            onEdited: root.editValue = text
        }
        Flow {
            width: parent.width
            spacing: 6
            Repeater {
                model: root.editSpec.presets || []
                delegate: Btn {
                    required property var modelData
                    text: String(modelData[0] || "")
                    variant: "outline"
                    size: 0.75
                    onClicked: { root.editValue = String(modelData[1] || ""); editInput.text = root.editValue }
                }
            }
        }
        footer: [
            Btn { text: "Cancel"; variant: "outline"; onClicked: editDialog.close() },
            Btn { text: "Done"; variant: "primary"; onClicked: root.saveEdit() }
        ]
        onOpened: editInput.forceActiveFocus()
    }

    AppDialog {
        id: logoutDialog
        title: "Log out of WhatsApp?"
        description: "This disconnects this desktop session from WhatsApp."
        cardWidth: 400
        footer: [
            Btn { text: "Cancel"; variant: "outline"; onClicked: logoutDialog.close() },
            Btn { text: "Log out"; variant: "destructive"; onClicked: { System.logout(); logoutDialog.close() } }
        ]
    }
}
