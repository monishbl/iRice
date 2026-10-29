#include <hyprland/src/plugins/PluginAPI.hpp>
#include <hyprland/src/pointer/PointerManager.hpp>
#include <hyprland/src/managers/input/InputManager.hpp>
#include <hyprland/src/state/MonitorState.hpp>
#include <hyprland/src/output/Monitor.hpp>
#include <chrono>
#include <sstream>
#include <string>
#include "pointer-policy.hpp"

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

void pointerInit(HANDLE handle) {
    for (const auto& fn : HyprlandAPI::findFunctionsByName(handle, "move")) {
        if (fn.demangled.find("CPointerManager::move(") == std::string::npos) continue;
        hook = HyprlandAPI::createFunctionHook(handle, fn.address, reinterpret_cast<void*>(&movePointer));
        if (!hook || !hook->hook()) hook = nullptr;
        break;
    }
}
bool pointerReady() { return hook != nullptr; }
void pointerExit() {
    if (hook) hook->unhook();
    hook = nullptr; mode = 0; confinement.clear(); resistance.reset(); previousMonitor.clear();
}
std::string pointerCommand(const std::string& verb, std::istringstream& in) {
    if (!hook) return "error: pointer boundary hook unavailable; rebuild for this Hyprland version";
    if (verb == "pointer-refocus") {
        // simulateMouseMovement alone is insufficient after 500ms of idle:
        // Hyprland resets accumulated motion to zero, failing its > threshold
        // check. Reset only that movement timer, then use normal hit-testing so
        // exclusive layers, pointer grabs and no-follow rules remain respected.
        g_pInputManager->m_lastCursorMovement.reset();
        g_pInputManager->simulateMouseMovement();
    } else if (verb == "pointer-policy") {
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
