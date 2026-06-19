import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import org.kde.kirigami as Kirigami
import kiosque

Kirigami.ScrollablePage {
    title: "Test Page"
    
    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18n("Close")
            visible: applicationWindow().pageStack.depth > 1
            onTriggered: applicationWindow().pageStack.pop()
        }
    ]
    
    property AppListModel testModel: AppListModel {}
    
    Component.onCompleted: {
        testModel.loadCategory("Office");
    }

    ColumnLayout {
        anchors.fill: parent

        Repeater {
            model: testModel
            delegate: Rectangle {
                Layout.fillWidth: true
                height: 50
                color: "lightblue"
                border.color: "blue"

                required property string name
                required property string appId

                Text {
                    anchors.centerIn: parent
                    text: parent.name + " (" + parent.appId + ")"
                }
                
                Component.onCompleted: console.log("Test Delegate Created:", name, appId)
            }
        }
    }
}
