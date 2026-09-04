import QtQuick

Item {
    property var config: ({a:1,b:2})
    property var member: ({a:1}).a
    property var guarded: ({a:1}) || fallback
    property var chosen: ({a:1}) ? 1 : 2
    onClicked: ({a:1}), activate()
}
