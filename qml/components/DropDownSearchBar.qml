// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import com.kiosque

// A page-wide search bar, hidden until it is needed. It slides down under the
// page's title bar when the user starts typing (see Main.qml), presses
// Ctrl+F or chooses Search, spans the whole width of the page, and slides
// away again on Escape or once it is left empty.
Item {
    id: bar

    property alias text: field.text
    property alias placeholderText: field.placeholderText
    // objectName of the field itself, for tests and accessibility tools.
    property string fieldObjectName: ""
    readonly property alias field: field
    // True while the bar is pulled down.
    property bool shown: false

    implicitWidth: parent ? parent.width : 0
    implicitHeight: shown ? field.implicitHeight + Kirigami.Units.largeSpacing * 2 : 0
    clip: true
    visible: implicitHeight > 0

    Behavior on implicitHeight {
        NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic }
    }

    Kirigami.Theme.colorSet: Kirigami.Theme.Header
    Kirigami.Theme.inherit: false

    /// Pull the bar down and put the cursor at the end, appending `typed`
    /// (the first characters the user typed, or "" for Ctrl+F and the
    /// Search action).
    function start(typed) {
        if (!shown) {
            field.text = "";
        }
        shown = true;
        if (typed && typed.length > 0) {
            field.insert(field.text.length, typed);
        }
        field.forceActiveFocus();
        field.cursorPosition = field.text.length;
    }

    /// Clear the query and slide the bar away.
    function close() {
        field.text = "";
        shown = false;
    }

    Rectangle {
        anchors.fill: parent
        color: Kirigami.Theme.backgroundColor
    }

    // Anchored to the bottom so it appears to drop down as the bar grows.
    Kirigami.SearchField {
        id: field
        objectName: bar.fieldObjectName
        anchors {
            left: parent.left
            right: parent.right
            bottom: parent.bottom
            leftMargin: Kirigami.Units.largeSpacing
            rightMargin: Kirigami.Units.largeSpacing
            bottomMargin: Kirigami.Units.largeSpacing
        }
        // Results follow every keystroke; models debounce their own requests.
        autoAccept: false
        Keys.onEscapePressed: bar.close()
        onActiveFocusChanged: {
            if (!activeFocus && text.length === 0) {
                bar.shown = false;
            }
        }
    }

    Kirigami.Separator {
        anchors {
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }
    }
}
