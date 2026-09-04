import QtQuick

Item {
    onClicked:
        if (ready) {
            _: activate()
        }
}
