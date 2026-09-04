import QtQuick

Item {
    width: /* explanation */
        parent.width+1
    onClicked: // prepare
        if (ready) { activate() }
}
