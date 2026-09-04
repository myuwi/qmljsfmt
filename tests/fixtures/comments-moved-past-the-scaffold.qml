import QtQuick

Item {
    width: (parent.width /* trailing */)
    onClicked: (prepare(), activate() /* trailing */)
    property var handler: function () { return 1 } /* trailing */
    onPressed: { activate() } /* trailing */
}
