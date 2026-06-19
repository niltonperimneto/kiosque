// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n
import com.kiosque

Kirigami.Page {
    id: page
    title: i18n("Installed")
    property var installedModel: applicationWindow().installedModel
    property string searchQuery: ""
    property bool showUpdatesOnly: false

    readonly property bool isNarrow: width < Kirigami.Units.gridUnit * 38

    // Sorting state
    property string sortCriterion: "name"
    property bool sortAscending: true

    // Platform updates toggle
    property bool platformUpdatesExpanded: false

    Connections {
        target: page.installedModel
        ignoreUnknownSignals: true
        
        function onChecking_updatesChanged() {
            if (page.installedModel && !page.installedModel.checking_updates && page.installedModel.updatesAvailableCount() > 0) {
                page.showUpdatesOnly = true;
            }
        }
        
        function onUpdatingChanged() {
            if (page.installedModel && !page.installedModel.updating && page.installedModel.updatesAvailableCount() === 0) {
                page.showUpdatesOnly = false;
            }
        }
    }

    onSortCriterionChanged: {
        page.installedModel.sortModel(sortCriterion, sortAscending);
    }
    onSortAscendingChanged: {
        page.installedModel.sortModel(sortCriterion, sortAscending);
    }

    Component.onCompleted: {
        page.installedModel.sortModel(page.sortCriterion, page.sortAscending);
    }

    actions: [
        Kirigami.Action {
            text: i18n("Refresh")
            icon.name: "view-refresh"
            onTriggered: page.installedModel.refresh()
        },
        Kirigami.Action {
            icon.name: "window-close"
            text: i18n("Close")
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]

    Controls.BusyIndicator {
        anchors.centerIn: parent
        running: page.installedModel && page.installedModel.loading
        visible: running
        z: 100
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        visible: listView.count === 0 && (!page.installedModel || !page.installedModel.loading)
        text: i18n("No Flatpak Applications Installed")
        icon.name: "application-x-executable"
        explanation: i18n("Install applications from the Storefront tab")
    }

    ListView {
        id: listView
        anchors.fill: parent
        model: page.installedModel
        spacing: Kirigami.Units.mediumSpacing
        opacity: 0

        Component.onCompleted: {
            enterAnim.start();
        }

        NumberAnimation {
            id: enterAnim
            target: listView
            property: "opacity"
            from: 0
            to: 1.0
            duration: 500
            easing.type: Easing.OutCubic
        }

        // ── Sections to separate updates from up-to-date apps ────────────────
        section.property: "sectionGroup"
        section.criteria: ViewSection.FullString
        section.delegate: Component {
            id: sectionHeader
            Item {
                width: listView.width - listView.leftMargin - listView.rightMargin
                implicitHeight: Kirigami.Units.gridUnit * 2.5
                height: implicitHeight
                
                RowLayout {
                    anchors.fill: parent
                    anchors.topMargin: Kirigami.Units.largeSpacing
                    anchors.bottomMargin: Kirigami.Units.mediumSpacing
                    spacing: Kirigami.Units.mediumSpacing
                    
                    Kirigami.Heading {
                        level: 4
                        text: {
                            if (section === "Platform Updates") return i18n("Platform & SDK Updates");
                            if (section === "Updates Available") return i18n("Updates Available");
                            return i18n("Up to Date");
                        }
                        color: section === "Updates Available" || section === "Platform Updates" 
                               ? Kirigami.Theme.positiveTextColor 
                               : Kirigami.Theme.disabledTextColor
                    }
                    
                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.1)
                        Layout.alignment: Qt.AlignVCenter
                    }

                    Controls.ToolButton {
                        visible: section === "Platform Updates"
                        icon.name: page.platformUpdatesExpanded ? "go-up" : "go-down"
                        text: page.platformUpdatesExpanded ? i18n("Hide") : i18n("Show")
                        display: Controls.AbstractButton.TextBesideIcon
                        onClicked: page.platformUpdatesExpanded = !page.platformUpdatesExpanded
                    }
                }
            }
        }


        // Add padding around the list to match storefront spacing
        leftMargin: Kirigami.Units.largeSpacing
        rightMargin: Kirigami.Units.largeSpacing
        topMargin: Kirigami.Units.largeSpacing
        bottomMargin: Kirigami.Units.largeSpacing

        header: Item {
            width: listView.width - listView.leftMargin - listView.rightMargin
            height: headerLayout.implicitHeight + Kirigami.Units.largeSpacing

            GridLayout {
                id: headerLayout
                anchors.centerIn: parent
                width: parent.width
                columns: page.isNarrow ? 1 : 2
                columnSpacing: Kirigami.Units.mediumSpacing
                rowSpacing: Kirigami.Units.mediumSpacing

                Kirigami.SearchField {
                    id: searchField
                    Layout.fillWidth: true
                    placeholderText: i18n("Search installed applications…")
                    onTextChanged: page.searchQuery = text
                }

                RowLayout {
                    Layout.fillWidth: page.isNarrow
                    Layout.alignment: page.isNarrow ? Qt.AlignLeft : Qt.AlignRight
                    spacing: Kirigami.Units.mediumSpacing

                    Controls.Button {
                        id: updatesFilterButton
                        icon.name: "system-software-update"
                        text: page.installedModel ? i18n("Updates (%1)", page.installedModel.updatesAvailableCount()) : i18n("Updates")
                        checkable: true
                        checked: page.showUpdatesOnly
                        visible: page.installedModel && page.installedModel.updatesAvailableCount() > 0
                        onToggled: page.showUpdatesOnly = checked
                    }

                    Controls.Label {
                        text: i18n("Sort by:")
                        color: Kirigami.Theme.textColor
                        verticalAlignment: Text.AlignVCenter
                    }

                    Controls.ComboBox {
                        id: sortCombo
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                        model: [i18n("Name"), i18n("Size")]
                        currentIndex: 0
                        onCurrentIndexChanged: {
                            if (currentIndex === 0) {
                                page.sortCriterion = "name";
                            } else if (currentIndex === 1) {
                                page.sortCriterion = "size";
                            }
                        }
                    }

                    Controls.Button {
                        id: sortOrderButton
                        icon.name: page.sortAscending ? "view-sort-ascending" : "view-sort-descending"
                        text: page.sortAscending ? i18n("Ascending") : i18n("Descending")
                        display: Controls.AbstractButton.IconOnly
                        onClicked: page.sortAscending = !page.sortAscending
                        
                        Controls.ToolTip {
                            text: page.sortAscending ? i18n("Sort Ascending") : i18n("Sort Descending")
                            visible: parent.hovered
                        }
                    }
                }
            }
        }

        delegate: Kirigami.SwipeListItem {
            id: delegateItem

            property bool isMatch: (page.searchQuery.length === 0 || 
                                   model.name.toLowerCase().indexOf(page.searchQuery.toLowerCase()) !== -1 ||
                                   model.appId.toLowerCase().indexOf(page.searchQuery.toLowerCase()) !== -1) &&
                                   (!page.showUpdatesOnly || model.hasUpdate)

            property bool isVisibleInList: isMatch && (!model.isRuntime || page.platformUpdatesExpanded)

            width: ListView.view ? ListView.view.width - ListView.view.leftMargin - ListView.view.rightMargin : 0
            visible: isVisibleInList
            
            // Set standard premium height for list items
            implicitHeight: Kirigami.Units.gridUnit * 5.2
            height: isVisibleInList ? implicitHeight : 0
            clip: true

            background: Kirigami.ShadowedRectangle {
                radius: 8
                color: delegateItem.hovered ? Kirigami.Theme.alternateBackgroundColor : Kirigami.Theme.backgroundColor
                border.width: 1
                border.color: delegateItem.hovered ? Kirigami.Theme.highlightColor : Qt.rgba(0, 0, 0, 0.08)
                
                shadow.size: delegateItem.hovered ? 6 : 0
                shadow.color: Qt.rgba(0, 0, 0, 0.06)

                // ── Uninstall Progress Fill ─────────────────────────────────
                Rectangle {
                    visible: page.installedModel && page.installedModel.uninstalling_app_id === model.appId
                    height: parent.height
                    width: parent.width * (page.installedModel ? page.installedModel.uninstall_progress : 0.0)
                    color: Qt.rgba(Kirigami.Theme.negativeTextColor.r, Kirigami.Theme.negativeTextColor.g, Kirigami.Theme.negativeTextColor.b, 0.15)
                    radius: 8

                    Behavior on width { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }
                }

                Behavior on color { ColorAnimation { duration: 100 } }
                Behavior on border.color { ColorAnimation { duration: 100 } }
            }

            onClicked: {
                applicationWindow().pushAppDetail(model.appId);
            }

            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Layout.fillWidth: true
                Layout.fillHeight: true

                // ── App Icon ────────────────────────────────────────────────
                Kirigami.ShadowedRectangle {
                    Layout.alignment: Qt.AlignVCenter
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 3.8
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 3.8
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.width: 1
                    border.color: Qt.rgba(0, 0, 0, 0.08)

                    Kirigami.Icon {
                        id: appIcon
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing
                        source: model.appId || "application-x-executable"
                        fallback: "application-x-executable"
                    }
                }

                // ── Text Information & Badges ───────────────────────────────
                ColumnLayout {
                    spacing: 3
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter

                    Kirigami.Heading {
                        level: 3
                        text: model.name
                        font.bold: true
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }

                    Controls.Label {
                        text: model.appId
                        color: Kirigami.Theme.disabledTextColor
                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }

                    RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Layout.fillWidth: true

                        // ── Size Badge ─────────────────────────────────────────────
                        Kirigami.ShadowedRectangle {
                            visible: model.size !== ""
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.2
                            implicitWidth: sizeLabel.implicitWidth + Kirigami.Units.mediumSpacing * 2
                            radius: 4
                            color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.1)
                            border.width: 1
                            border.color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2)

                            Controls.Label {
                                id: sizeLabel
                                anchors.centerIn: parent
                                text: model.size
                                font.bold: true
                                font.pointSize: Kirigami.Theme.smallFont.pointSize - 1
                                color: Kirigami.Theme.highlightColor
                            }
                        }

                        // ── Version Badge ──────────────────────────────────────────
                        Kirigami.ShadowedRectangle {
                            visible: model.version !== ""
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.2
                            implicitWidth: versionLabel.implicitWidth + Kirigami.Units.mediumSpacing * 2
                            radius: 4
                            color: Kirigami.Theme.alternateBackgroundColor
                            border.width: 1
                            border.color: Qt.rgba(0, 0, 0, 0.15)

                            Controls.Label {
                                id: versionLabel
                                anchors.centerIn: parent
                                text: model.version
                                font.pointSize: Kirigami.Theme.smallFont.pointSize - 1
                                color: Kirigami.Theme.textColor
                            }
                        }


                        // Spacer to push badges to the left
                        Item {
                            Layout.fillWidth: true
                        }
                    }
                }

                // ── Update Checkbox ─────────────────────────────────────────
                Controls.CheckBox {
                    id: updateCheckbox
                    Layout.alignment: Qt.AlignVCenter
                    visible: model.hasUpdate
                    checked: model.isChecked
                    onToggled: page.installedModel.toggleUpdateChecked(index)
                }
            }

            actions: [
                Kirigami.Action {
                    text: i18n("Launch")
                    icon.name: "media-playback-start"
                    onTriggered: page.installedModel.launchApp(index)
                },
                Kirigami.Action {
                    text: i18n("Uninstall")
                    icon.name: "edit-delete"
                    onTriggered: page.installedModel.uninstallApp(index)
                }
            ]
        }
    }

    // ── Update Management Footer ────────────────────────────────────────────
    footer: Controls.ToolBar {
        id: footerBar
        implicitHeight: footerLayout.implicitHeight + Kirigami.Units.largeSpacing * 2
        visible: page.installedModel !== null
        
        background: Kirigami.ShadowedRectangle {
            color: Kirigami.Theme.backgroundColor
            border.width: 1
            border.color: Qt.rgba(0, 0, 0, 0.08)
            
            Rectangle {
                anchors.top: parent.top
                width: parent.width
                height: 1
                color: Qt.rgba(0, 0, 0, 0.12)
            }
        }

        ColumnLayout {
            id: footerLayout
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.mediumSpacing

            GridLayout {
                Layout.fillWidth: true
                columns: page.isNarrow ? 1 : 2
                rowSpacing: Kirigami.Units.mediumSpacing
                columnSpacing: Kirigami.Units.largeSpacing

                // ── Select All Checkbox ─────────────────────────────────────
                Controls.CheckBox {
                    id: selectAllCheckbox
                    text: page.installedModel ? i18n("Select All (%1)", page.installedModel.updatesAvailableCount()) : i18n("Select All")
                    visible: page.installedModel && page.installedModel.updatesAvailableCount() > 0 && !page.installedModel.checking_updates && !page.installedModel.updating
                    checked: page.installedModel && page.installedModel.updatesCheckedCount() === page.installedModel.updatesAvailableCount()
                    onToggled: page.installedModel.setAllUpdatesChecked(checked)
                    Layout.fillWidth: page.isNarrow
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.alignment: page.isNarrow ? Qt.AlignLeft : Qt.AlignRight
                    spacing: Kirigami.Units.mediumSpacing

                    // Spacer only when wide to push buttons to the right
                    Item {
                        visible: !page.isNarrow
                        Layout.fillWidth: true
                    }

                    // ── Buttons ─────────────────────────────────────────────────
                    Controls.Button {
                        text: i18n("Check for Updates")
                        icon.name: "view-refresh"
                        enabled: page.installedModel && !page.installedModel.checking_updates && !page.installedModel.updating
                        onClicked: page.installedModel.checkForUpdates()
                        Layout.fillWidth: page.isNarrow
                    }

                    Controls.Button {
                        text: {
                            if (!page.installedModel) {
                                return i18n("Update");
                            }
                            return page.installedModel.updatesAvailableCount() === 0 ? i18n("Updated") : i18n("Update");
                        }
                        icon.name: "system-software-update"
                        enabled: page.installedModel && page.installedModel.updatesCheckedCount() > 0 && !page.installedModel.checking_updates && !page.installedModel.updating
                        highlighted: enabled
                        onClicked: page.installedModel.updateSelectedApps()
                        Layout.fillWidth: page.isNarrow
                    }
                }
            }

            // ── Progress Bar ────────────────────────────────────────────────
            Controls.ProgressBar {
                Layout.fillWidth: true
                visible: page.installedModel && (page.installedModel.updating || page.installedModel.checking_updates)
                value: page.installedModel ? (page.installedModel.updating ? page.installedModel.update_progress : 0) : 0
                indeterminate: page.installedModel && (page.installedModel.checking_updates || (page.installedModel.updating && value <= 0.01))
            }

            // ── Status Visual Feedback Text ──────────────────────────────────
            Controls.Label {
                id: feedbackLabel
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.disabledTextColor
                elide: Text.ElideRight
                visible: page.installedModel && (page.installedModel.updating || page.installedModel.checking_updates)
                text: {
                    if (!page.installedModel) return "";
                    if (page.installedModel.updating) {
                        return page.installedModel.update_status_text || i18n("Updating applications…");
                    }
                    if (page.installedModel.checking_updates) {
                        return i18n("Checking for updates from Flatpak remotes…");
                    }
                    return "";
                }
            }
        }
    }
}

