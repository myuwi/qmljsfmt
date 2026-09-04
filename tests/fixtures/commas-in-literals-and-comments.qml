import QtQuick

Item {
    property string quoted: "a, //"
    property string templated: `a, //`
    property int count: /* a, // b */ model.count+1
}
