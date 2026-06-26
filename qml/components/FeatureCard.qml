// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import org.kde.kirigami as Kirigami
import QtQuick.Effects

// The single source of truth for the Shopfront's "flashy" surfaces.
//
// One decorative language — accent-tinted gradient, ambient glow, hover glow,
// lift and border — defined once and reused by every feature surface (the hero
// slide and the App-of-the-Day spotlight) so they stay perfectly consistent.
//
// The accent defaults to the system highlight colour, so the flair tracks the
// user's Plasma colour scheme (light / dark / custom) per the KDE HIG instead
// of hardcoding a hue.
//
// Place content as direct children; the decorative background sits behind them
// (z: -2) so consumer content always renders on top and can anchor to the card.
Kirigami.ShadowedRectangle {
    id: root

    // The single shared accent — defaults to the system highlight colour.
    property color accent: Kirigami.Theme.highlightColor

    // Hover state. Driven internally, but can be overridden by a parent (e.g.
    // when a whole carousel slide should react as one surface).
    property bool hovered: hoverHandler.hovered

    // When false the card is purely decorative (no cursor, tap or hover lift).
    property bool interactive: true

    // A subtle lift on hover. Disable for surfaces that must not clip (e.g. a
    // full-bleed carousel slide inside a clipping SwipeView).
    property bool hoverScale: true

    // Gate the perpetual ambient pulse so the motion stays tasteful and can be
    // switched off for motion-sensitive setups (one toggle for all surfaces).
    property bool animateGlow: true

    signal clicked()

    // View colour set → surface distinct from the page background.
    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.inherit: false

    radius: Kirigami.Units.cornerRadius
    color: "transparent"

    // ── One shared shadow ramp ──────────────────────────────────────────
    shadow.size: root.hovered ? Kirigami.Units.gridUnit : Kirigami.Units.gridUnit * 0.65
    shadow.color: root.hovered
        ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.22)
        : Qt.rgba(0, 0, 0, 0.12)
    shadow.yOffset: root.hovered ? 5 : 3

    border.width: 1
    border.color: root.hovered
        ? root.accent
        : Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.5)

    Behavior on shadow.size { NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic } }
    Behavior on shadow.yOffset { NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic } }
    Behavior on border.color { ColorAnimation { duration: Kirigami.Units.longDuration } }

    scale: (root.interactive && root.hoverScale && root.hovered) ? 1.02 : 1.0
    Behavior on scale { NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic } }

    // ── Interaction ─────────────────────────────────────────────────────
    HoverHandler {
        id: hoverHandler
        enabled: root.interactive
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        enabled: root.interactive
        onTapped: root.clicked()
    }

    // ── Decorative background (gradient + glow), masked to rounded corners ─
    Rectangle {
        id: innerBg
        anchors.fill: parent
        radius: root.radius
        z: -2

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0.0
                color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.14))
            }
            GradientStop {
                position: 0.5
                color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.28))
            }
            GradientStop {
                position: 1.0
                color: Qt.tint(Kirigami.Theme.backgroundColor, Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.45))
            }
        }

        // Decorative pulsing/ambient glow sphere in the top-right.
        Item {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: -width / 4
            width: parent.width * 0.4
            height: width
            z: -1

            opacity: root.hovered ? 1.0 : 0.6
            Behavior on opacity { NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic } }

            Rectangle {
                anchors.fill: parent
                radius: width / 2

                gradient: Gradient {
                    GradientStop {
                        position: 0.0
                        color: Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.2)
                    }
                    GradientStop {
                        position: 1.0
                        color: "transparent"
                    }
                }

                SequentialAnimation on opacity {
                    running: root.animateGlow
                    loops: Animation.Infinite
                    NumberAnimation { from: 0.5; to: 0.9; duration: 4000; easing.type: Easing.InOutSine }
                    NumberAnimation { from: 0.9; to: 0.5; duration: 4000; easing.type: Easing.InOutSine }
                }
            }
        }

        // Hover highlight overlay.
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: root.hovered
                ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.08)
                : "transparent"
            Behavior on color { ColorAnimation { duration: Kirigami.Units.longDuration } }
        }

        // Mask item for MultiEffect to enforce rounded corners.
        Rectangle {
            id: maskRect
            anchors.fill: parent
            radius: parent.radius
            color: "black"
            visible: false
        }

        layer.enabled: true
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: maskRect
        }
    }
}
