#include <hyprland/src/plugins/PluginAPI.hpp>
#include <hyprland/src/pointer/PointerManager.hpp>
#include <hyprland/src/managers/input/InputManager.hpp>
#include <hyprland/src/state/MonitorState.hpp>
#include <hyprland/src/output/Monitor.hpp>
#include <chrono>
#include <sstream>
#include <string>
#include <hyprland/src/event/EventBus.hpp>
#include <hyprland/src/desktop/state/FocusState.hpp>
#include <hyprland/src/desktop/state/WindowState.hpp>
#include <hyprland/src/desktop/view/Window.hpp>
#include <hyprland/src/desktop/Workspace.hpp>
#include <hyprland/src/managers/eventLoop/EventLoopManager.hpp>
#include "pointer-policy.hpp"
#include "focus-policy.hpp"

static CFunctionHook* hook = nullptr;
static int mode = 0;
static double corner = 24;
static std::string confinement;
static double confineUntil = 0;
static double milliseconds() { return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count(); }
static std::string previousMonitor;
static PointerPolicy::Resistance resistance;
using Move = void (*)(void*, const Vector2D&);

static void movePointer(void* self, const Vector2D& delta) {
    if (!confinement.empty() && milliseconds() > confineUntil) confinement.clear();
    const auto original = reinterpret_cast<Move>(hook->m_original);
    if ((!mode && confinement.empty()) || g_pInputManager->isConstrained()) {
        original(self, delta);
        return;
    }
    const auto old = g_pInputManager->getMouseCoordsInternal();
    PointerPolicy::Point from{old.x, old.y}, to{old.x + delta.x, old.y + delta.y};
    for (const auto& monitor : State::monitorState()->monitors()) {
        const auto box = monitor->logicalBox();
        const PointerPolicy::Box bounds{box.x, box.y, box.w, box.h};
        if (!confinement.empty() && monitor->m_name == confinement) {
            // Only confine a pointer already on the selection's screen. Never
            // teleport a pointer from another screen due to a stale overlay.
            if (PointerPolicy::contains(bounds, from)) {
                to = PointerPolicy::clamp(bounds, to);
                original(self, Vector2D{to.x - old.x, to.y - old.y});
                return;
            }
        }
    }
    for (const auto& monitor : State::monitorState()->monitors()) {
        const auto box = monitor->logicalBox();
        const PointerPolicy::Box bounds{box.x, box.y, box.w, box.h};
        if (!PointerPolicy::contains(bounds, from)) continue;
        if (previousMonitor != monitor->m_name) { resistance.reset(); previousMonitor = monitor->m_name; }
        const auto now = milliseconds();
        to = PointerPolicy::limit(bounds, from, to, mode, corner, now, resistance);
        break;
    }
    original(self, Vector2D{to.x - old.x, to.y - old.y});
}

// ── Focus that follows the pointer, and windows brought forward ──────────
// (focus-policy.hpp says when; the shell turns it on for hover focus with
// `pointer-follow 1`.) Also when the user last clicked or typed, so the shell
// can tell a window asking for attention right after a click on a link (bring
// it forward) from one pinging on its own (just show it wants attention).
static bool               followFocus = false;
static double             lastInput   = -1e12;
static CHyprSignalListener focusListener, keyListener, buttonListener;

static bool inBox(const CBox& b, const Vector2D& p) {
    return p.x >= b.x && p.y >= b.y && p.x < b.x + b.w && p.y < b.y + b.h;
}
static bool shows(const PHLWINDOW& w) {
    return w && w->m_isMapped && !w->isHidden() && w->m_workspace && w->m_workspace->isVisible();
}
static void onFocus(PHLWINDOW w, Desktop::eFocusReason reason) {
    if (!followFocus || !shows(w) || !FocusPolicy::broughtForward((int)reason))
        return;
    // After the focus change has settled (not from inside it)
    PHLWINDOWREF ref = w;
    g_pEventLoopManager->doLater([ref, reason] {
        const auto win = ref.lock();
        if (!followFocus || !shows(win) || g_pInputManager->isConstrained())
            return;
        const auto pos   = Pointer::mgr()->position();
        const auto box   = win->getWindowMainSurfaceBox();
        bool       other = false;
        for (auto& o : Desktop::windowState()->windows())
            if (o != win && shows(o) && inBox(o->getWindowMainSurfaceBox(), pos)) {
                other = true;
                break;
            }
        if (FocusPolicy::shouldWarp((int)reason, inBox(box, pos), other))
            Pointer::mgr()->warpTo(box.middle());
    });
}
static void focusInit() {
    focusListener  = Event::bus()->m_events.window.active.listen([](PHLWINDOW w, Desktop::eFocusReason r) { onFocus(w, r); });
    keyListener    = Event::bus()->m_events.input.keyboard.key.listen([](IKeyboard::SKeyEvent e, Event::SCallbackInfo&) {
        if (e.state == WL_KEYBOARD_KEY_STATE_PRESSED)
            lastInput = milliseconds();
    });
    buttonListener = Event::bus()->m_events.input.mouse.button.listen([](IPointer::SButtonEvent e, Event::SCallbackInfo&) {
        if (e.state == WL_POINTER_BUTTON_STATE_PRESSED)
            lastInput = milliseconds();
    });
}

void pointerInit(HANDLE handle) {
    focusInit();
    for (const auto& fn : HyprlandAPI::findFunctionsByName(handle, "move")) {
        if (fn.demangled.find("CPointerManager::move(") == std::string::npos) continue;
        hook = HyprlandAPI::createFunctionHook(handle, fn.address, reinterpret_cast<void*>(&movePointer));
        // A hook that couldn't be put in place is handed back, not kept
        if (hook && !hook->hook()) {
            HyprlandAPI::removeFunctionHook(handle, hook);
            hook = nullptr;
        }
        break;
    }
}
bool pointerReady() { return hook != nullptr; }
void pointerExit() {
    // The hook itself is removed by Hyprland when the plugin unloads (as in
    // main.cpp's exit); only our state is reset here
    hook = nullptr; mode = 0; confinement.clear(); resistance.reset(); previousMonitor.clear();
    focusListener.reset();
    keyListener.reset();
    buttonListener.reset();
    followFocus = false;
}
std::string pointerCommand(const std::string& verb, std::istringstream& in) {
    if (verb == "pointer-follow") { // 1: focus follows the pointer (hover), 0: click
        int on;
        if (!(in >> on) || (on != 0 && on != 1))
            return "error: pointer-follow expects 0 or 1";
        followFocus = on == 1;
        return "ok";
    }
    if (verb == "input-age") // ms since the last key or button press
        return std::to_string((long long)(milliseconds() - lastInput));
    if (verb == "pointer-refocus") {
        // simulateMouseMovement alone is insufficient after 500ms of idle:
        // Hyprland resets accumulated motion to zero, failing its > threshold
        // check. Reset only that movement timer, then use normal hit-testing so
        // exclusive layers, pointer grabs and no-follow rules remain respected.
        g_pInputManager->m_lastCursorMovement.reset();
        g_pInputManager->simulateMouseMovement();
        return "ok";
    }
    // Boundaries and confinement work through the move hook
    if (!hook) return "error: pointer boundary hook unavailable; rebuild for this Hyprland version";
    if (verb == "pointer-policy") {
        int requested; double pixels;
        if (!(in >> requested >> pixels) || requested < 0 || requested > 2 || !std::isfinite(pixels) || pixels < 4 || pixels > 100)
            return "error: pointer-policy expects mode 0..2 and corner size 4..100";
        mode = requested; corner = pixels; resistance.reset();
    } else if (verb == "pointer-confine") {
        std::string monitor;
        if (!(in >> monitor)) return "error: pointer-confine expects monitor or -";
        confinement = monitor == "-" ? "" : monitor;
        confineUntil = milliseconds() + 1500;
        resistance.reset();
    }
    return "ok";
}
