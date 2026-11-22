// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

import QtQuick
import com.kiosque
import org.kde.kirigami as Kirigami

// A screenshot frame with rounded corners that crops from the TOP of the
// image instead of the center, so application title bars stay visible.
// Kirigami.ShadowedImage cannot do this (its inner Image has no alignment
// control), so this mirrors its hidden-Image + ShaderEffectSource +
// ShadowedTexture technique with a top-anchored crop.
Item {
    id: root

    property alias source: img.remoteSource
    property alias status: img.status
    property alias asynchronous: img.asynchronous
    // Width-only request: the image backend scales preserving aspect ratio
    // when a single axis is given. Never set a height here.
    property alias sourceSizeWidth: img.sourceSize.width
    property alias corners: texture.corners
    property alias shadow: texture.shadow
    property alias color: texture.color

    RemoteImageView {
        id: img
        anchors.fill: parent
        visible: false
        fillMode: Image.PreserveAspectCrop
        verticalAlignment: Image.AlignTop
        horizontalAlignment: Image.AlignHCenter
        asynchronous: true
    }

    ShaderEffectSource {
        id: effectSource
        anchors.fill: parent
        sourceItem: img
        hideSource: true
        visible: false
    }

    Kirigami.ShadowedTexture {
        id: texture
        anchors.fill: parent
        color: "transparent"
        source: img.status === Image.Ready ? effectSource : null
    }
}
