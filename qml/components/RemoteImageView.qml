import QtQuick
import com.kiosque
Image {
 id: root
 property url remoteSource
 // Image purpose is explicit: an unsized full-screen image is not an icon.
 property string imageKind: "icon:" + Math.max(128, sourceSize.width)
 RemoteImage {
  id: sanitized
  url: String(root.remoteSource).startsWith("qrc:") ? "" : String(root.remoteSource)
  kind: root.imageKind
 }
 source: String(remoteSource).startsWith("qrc:") ? remoteSource : sanitized.source
}
