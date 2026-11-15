// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import "../style" as Styles
import org.kde.kirigami as Kirigami
import org.kde.ki18n

// Bazaar-style spotlight banner: the app's logo, name and summary on the left,
// its primary screenshot in a floating "window" bleeding off the bottom-right,
// all on a gradient of the app's appstream branding color. Degrades to a
// centered icon + text layout when there is no screenshot or the banner is
// narrow.
Item {
    id: root

    property string appName
    property string appSummary
    property string developer
    property url    iconUrl
    property url    screenshotUrl
    // Appstream branding primary colors as hex strings; "" when the app
    // publishes none.
    property string brandColorLight
    property string brandColorDark
    property bool   showAppOfTheDayBadge: false

    activeFocusOnTab: true
    Keys.onReturnPressed: root.clicked()
    Keys.onEnterPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()

    signal clicked()

    readonly property bool compact: width < Kirigami.Units.gridUnit * 30
    readonly property bool showScreenshot: String(screenshotUrl).length > 0
                                           && screenshot.status !== Image.Error
                                           && !compact

    // ── Brand color resolution ──────────────────────────────────────────
    // Pick the branding variant matching the current color scheme; if the app
    // only ships the other variant, shift it toward the scheme; with no
    // branding at all, tint the theme background with the highlight color.
    readonly property bool darkScheme:
        Kirigami.ColorUtils.brightnessForColor(Kirigami.Theme.backgroundColor) === Kirigami.ColorUtils.Dark
    readonly property color brandColor: {
        const pick = darkScheme ? brandColorDark : brandColorLight;
        const alt  = darkScheme ? brandColorLight : brandColorDark;
        if (pick.length > 0)
            return pick;
        if (alt.length > 0)
            return darkScheme ? Qt.darker(alt, 1.8) : Qt.lighter(alt, 1.6);
        return Qt.tint(Kirigami.Theme.backgroundColor,
                       Qt.alpha(Kirigami.Theme.highlightColor, 0.3));
    }
    // Text sits on the brand surface, so contrast must come from the brand
    // color itself — never from Kirigami.Theme.textColor.
    readonly property color fgColor:
        Kirigami.ColorUtils.brightnessForColor(brandColor) === Kirigami.ColorUtils.Dark
        ? "white" : Qt.rgba(0, 0, 0, 0.85)

    implicitWidth: Kirigami.Units.gridUnit * 36
    implicitHeight: Kirigami.Units.gridUnit * 14

    Accessible.role: Accessible.Button
    Accessible.name: root.appName
    Accessible.onPressAction: root.clicked()

    // ── Brand surface ───────────────────────────────────────────────────
    Rectangle {
        id: surface
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius * 2

        gradient: Gradient {
            GradientStop { position: 0.0; color: root.brandColor }
            GradientStop { position: 1.0; color: Qt.darker(root.brandColor, 1.25) }
        }

        // ── Screenshot "window", clipped so it bleeds off the bottom ────
        Item {
            id: screenshotPane
            visible: root.showScreenshot
            clip: true
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            // Right margin stays past the corner radius so the square clip
            // never cuts across the surface's rounded corner.
            anchors.rightMargin: surface.radius + Kirigami.Units.largeSpacing
            width: parent.width * 0.5

            TopCropImage {
                id: screenshot
                anchors.top: parent.top
                anchors.topMargin: Kirigami.Units.largeSpacing * 2
                anchors.left: parent.left
                anchors.right: parent.right
                // Extend past the banner's bottom edge; the pane clips it so
                // the window appears to continue below the fold.
                anchors.bottom: parent.bottom
                anchors.bottomMargin: -Kirigami.Units.gridUnit * 2

                source: root.screenshotUrl
                asynchronous: true
                sourceSizeWidth: Kirigami.Units.gridUnit * 40

                corners.topLeftRadius: Kirigami.Units.cornerRadius * 2
                corners.topRightRadius: Kirigami.Units.cornerRadius * 2
                shadow.size: Kirigami.Units.largeSpacing * 2
                shadow.yOffset: Kirigami.Units.smallSpacing
                shadow.color: Qt.alpha("black", 0.35)
            }
        }

        // ── Logo + text ─────────────────────────────────────────────────
        Item {
            id: contentPane
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            width: root.showScreenshot ? parent.width * 0.45 : parent.width

            Column {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.largeSpacing * 4
                spacing: Kirigami.Units.smallSpacing

                Row {
                    visible: root.showAppOfTheDayBadge
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: Kirigami.Units.smallSpacing

                    Kirigami.Icon {
                        source: "favorite-symbolic"
                        color: root.fgColor
                        width: Kirigami.Units.iconSizes.small
                        height: width
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    Kirigami.Heading {
                        font.family: Styles.Style.fontFamily
                        level: 5
                        text: i18nc("@info:label badge on the featured banner", "App of the Day")
                        color: root.fgColor
                        opacity: 0.8
                        font.capitalization: Font.AllUppercase
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }

                Image {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: root.compact ? Kirigami.Units.iconSizes.huge
                                        : Kirigami.Units.iconSizes.enormous
                    height: width
                    source: root.iconUrl
                    sourceSize: Qt.size(width, height)
                    fillMode: Image.PreserveAspectFit
                    mipmap: true
                    asynchronous: true
                }

                Kirigami.Heading {
                    font.family: Styles.Style.fontFamily
                    width: parent.width
                    level: 1
                    text: root.appName
                    color: root.fgColor
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }

                Kirigami.Heading {
                    font.family: Styles.Style.fontFamily
                    width: parent.width
                    level: 5
                    visible: root.developer.length > 0
                    text: root.developer
                    color: root.fgColor
                    opacity: 0.7
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    maximumLineCount: 1
                }

                Text {
                    width: parent.width
                    text: root.appSummary
                    color: root.fgColor
                    opacity: 0.9
                    font: Kirigami.Theme.defaultFont
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }
            }
        }

        // Subtle whole-surface hover feedback on an otherwise flat banner.
        Rectangle {
            anchors.fill: parent
            radius: surface.radius
            color: "white"
            opacity: hoverHandler.hovered ? 0.06 : 0
            Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
        }
    }

    HoverHandler {
        id: hoverHandler
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: root.clicked()
    }
}
