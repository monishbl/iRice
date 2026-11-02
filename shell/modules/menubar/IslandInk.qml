pragma Singleton
import QtQuick
import Quickshell
import qs.common

// The island's colours. Glass: white on the black island (the literals the
// island has always used, so Glass is unchanged). Material: the island is a
// tonal surface, surfaceContainerHigh, or inverseSurface while it shows an
// alert (`inverse`, like a snackbar), and its content takes the matching
// on-colours. `dim(a)` stands for Glass's white at `a`: its secondary text
// (0.45…0.9) becomes onSurfaceVariant, fainter tracks and state layers stay
// a fraction of the content colour.
Singleton {
    function surface(inverse) {
        return inverse ? Theme.m3.inverseSurface : Theme.m3.surfaceContainerHigh;
    }
    function ink(inverse) {
        if (!Theme.material)
            return "#F5F5F7";
        return inverse ? Theme.m3.inverseOnSurface : Theme.m3.onSurface;
    }
    function dim(a, inverse) {
        if (!Theme.material)
            return Qt.rgba(1, 1, 1, a);
        const c = inverse ? Theme.m3.inverseOnSurface : Theme.m3.onSurface;
        if (a >= 0.45 && a < 0.9)
            return inverse ? Qt.alpha(c, 0.78) : Theme.m3.onSurfaceVariant;
        return Qt.alpha(c, a);
    }
    // Key colour on the island: primary, inversePrimary on an alert
    function accent(inverse) {
        if (!Theme.material)
            return Theme.accent;
        return inverse ? Theme.m3.inversePrimary : Theme.m3.primary;
    }
    // Content on `accent`
    function onAccent(inverse) {
        if (!Theme.material)
            return Theme.color.onAccent;
        return inverse ? Theme.m3.inverseSurface : Theme.m3.onPrimary;
    }
    // A live activity's or alert's colour: Material keeps red for what's
    // urgent (recording, stop, low battery) as error, the rest primary
    function tint(c, inverse) {
        if (!Theme.material)
            return c;
        return isRed(c) ? Theme.m3.error : accent(inverse);
    }
    function onTint(c, inverse) {
        return isRed(c) ? Theme.m3.onError : onAccent(inverse);
    }
    function isRed(c) {
        if (c === undefined || c === null)
            return false;
        const k = Qt.alpha(c, 1); // (a colour, whether `c` is one or a name)
        return (k.r > 0.8 && k.g < 0.45 && k.b < 0.45) || (Theme.material && Qt.colorEqual(k, Theme.m3.error));
    }
    // A tonal button on the island (Material): secondaryContainer, or a
    // translucent layer of the content colour on an alert
    function tonal(inverse) {
        return inverse ? Qt.alpha(Theme.m3.inverseOnSurface, 0.16) : Theme.m3.secondaryContainer;
    }
    function onTonal(inverse) {
        return inverse ? Theme.m3.inverseOnSurface : Theme.m3.onSecondaryContainer;
    }
}
