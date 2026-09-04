import QtQuick

Item {
    onCanceled: with (context) { reset(foo+1) }
}
