import QtQuick

Item {
    width: parent.width+1
    onClicked: if (ready) activate()
    onPressed: { let x=1; activate(x) }
    function calculate(value: real): real { return value+1 }
}
