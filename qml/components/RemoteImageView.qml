import QtQuick
import com.kiosque
Image {
 id: root
 property url remoteSource
 RemoteImage {
  id: sanitized
  url: String(root.remoteSource).startsWith("qrc:") ? "" : String(root.remoteSource)
  kind: root.sourceSize.width > 512 || root.sourceSize.height > 128 ? "screenshot" : "icon:" + Math.max(128, root.sourceSize.width)
 }
 source: String(remoteSource).startsWith("qrc:") ? remoteSource : sanitized.source
}
