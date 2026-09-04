import QtQuick

Item {
    onClicked: first.reallyLongProperty.reallyLongMethod(), second.reallyLongProperty.reallyLongMethod(), third.reallyLongProperty.reallyLongMethod()
}
