// SPDX-License-Identifier: GPL-3.0-or-later
// Trackpad long presses. Hyprland's own gestures are swipes and pinches;
// libinput also reports fingers resting still (a hold), which Hyprland only
// passes on to apps. Three or more fingers held still for kHoldMs run the
// long press set for that many fingers (gestures.lua's IRICE_HOLD: circle
// to search on three, by default). Fingers that move (a swipe) or lift
// sooner cancel it.
#include <hyprland/src/plugins/PluginAPI.hpp>
#include <hyprland/src/managers/input/InputManager.hpp>
#include <hyprland/src/managers/eventLoop/EventLoopManager.hpp>
#include <hyprland/src/managers/eventLoop/EventLoopTimer.hpp>
#include <hyprland/src/event/EventBus.hpp>
#include <chrono>
#include <format>
#include <vector>

static constexpr int                      kHoldMs = 500;
static std::vector<CHyprSignalListener>   g_holdListeners;
static std::vector<WP<IPointer>>          g_watched;
static CHyprSignalListener                g_holdRescan;
static SP<CEventLoopTimer>                g_holdTimer;
static uint32_t                           g_holdFingers = 0;

static void holdFire(SP<CEventLoopTimer>, void*) {
    const auto fingers = g_holdFingers;
    g_holdFingers      = 0;
    if (fingers >= 3)
        HyprlandAPI::invokeHyprctlCommand("eval", std::format("if IRICE_HOLD then IRICE_HOLD({}) end", fingers));
}

// Listens to every pointer's holds; again when the pointers change (a
// touchpad plugged in)
static void holdWatch() {
    const auto& now = g_pInputManager->m_pointers;
    bool        same = now.size() == g_watched.size();
    for (size_t i = 0; same && i < now.size(); i++)
        same = g_watched[i].lock() == now[i];
    if (same)
        return;
    g_holdListeners.clear();
    g_watched.clear();
    for (const auto& p : now) {
        g_watched.emplace_back(p);
        g_holdListeners.emplace_back(p->m_pointerEvents.holdBegin.listen([](IPointer::SHoldBeginEvent e) {
            g_holdFingers = e.fingers;
            if (e.fingers >= 3)
                g_holdTimer->updateTimeout(std::chrono::milliseconds(kHoldMs));
        }));
        g_holdListeners.emplace_back(p->m_pointerEvents.holdEnd.listen([](IPointer::SHoldEndEvent) {
            g_holdFingers = 0;
            g_holdTimer->updateTimeout(std::nullopt);
        }));
    }
}

void holdInit() {
    g_holdTimer = makeShared<CEventLoopTimer>(std::nullopt, holdFire, nullptr);
    g_pEventLoopManager->addTimer(g_holdTimer);
    holdWatch();
    g_holdRescan = Event::bus()->m_events.input.mouse.move.listen([](Vector2D, Event::SCallbackInfo&) { holdWatch(); });
}

void holdExit() {
    g_holdRescan.reset();
    g_holdListeners.clear();
    g_watched.clear();
    if (g_holdTimer) {
        g_pEventLoopManager->removeTimer(g_holdTimer);
        g_holdTimer.reset();
    }
    g_holdFingers = 0;
}
