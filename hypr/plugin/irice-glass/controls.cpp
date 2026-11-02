// Window controls drawn by Hyprland: a small pill at the top centre of every
// window. Hover it and it grows into close, minimize and zoom. Because the
// compositor draws it as part of the window, it moves with the window on the
// same frame (the shell-drawn pill trails behind while a window moves).
//
// hyprctl irice-glass controls on [dark|light]
// hyprctl irice-glass controls off
//
// Resting on the zoom button for half a second posts the IPC event
// `irice-controls>>tile,<address>,<x>,<y>` so the shell can show its tile grid.

#define WLR_USE_UNSTABLE
#include <hyprland/src/plugins/PluginAPI.hpp>
#include <hyprland/src/render/decorations/IHyprWindowDecoration.hpp>
#include <hyprland/src/render/pass/PassElement.hpp>
#include <hyprland/src/render/OpenGL.hpp>
#include <hyprland/src/render/Renderer.hpp>
#include <hyprland/src/desktop/view/Window.hpp>
#include <hyprland/src/desktop/state/FocusState.hpp>
#include <hyprland/src/desktop/state/WindowState.hpp>
#include <hyprland/src/desktop/state/ViewHitTester.hpp>
#include <hyprland/src/desktop/state/ViewState.hpp>
#include <hyprland/src/protocols/LayerShell.hpp>
#include <hyprland/src/desktop/Workspace.hpp>
#include <hyprland/src/devices/IPointer.hpp>
#include <hyprland/src/event/EventBus.hpp>
#include <hyprland/src/managers/EventManager.hpp>
#include <hyprland/src/managers/KeybindManager.hpp>
#include <hyprland/src/managers/SeatManager.hpp>
#include <hyprland/src/managers/eventLoop/EventLoopManager.hpp>
#include <hyprland/src/managers/eventLoop/EventLoopTimer.hpp>
#include <hyprland/src/managers/fullscreen/FullscreenController.hpp>
#include <hyprland/src/managers/input/InputManager.hpp>
#include <hyprland/src/output/Monitor.hpp>
#include <hyprland/src/state/MonitorState.hpp>
#include <algorithm>
#include <chrono>
#include <format>
#include <vector>

using namespace Render;
using namespace Render::GL;

// Logical px
constexpr float kTop = 6, kHandleW = 36, kHandleH = 5, kPillW = 68, kPillH = 20, kDot = 12, kGap = 8;
constexpr float kZoneW = 110, kZoneH = 34; // where hovering expands the pill
constexpr float kAnimMs = 150;

static HANDLE g_ctlHandle = nullptr;
static bool   g_enabled = false, g_dark = true;

class CIriceControls;
static std::vector<CIriceControls*> g_decos;

class CControlsPass : public IPassElement {
  public:
    CControlsPass(CIriceControls* d, float a) : m_deco(d), m_a(a) {}
    std::vector<UP<IPassElement>> draw() override;
    bool                          needsLiveBlur() override { return false; }
    bool                          needsPrecomputeBlur() override { return false; }
    std::optional<CBox>           boundingBox() override;
    const char*                   passName() override { return "CIriceControlsPass"; }
    ePassElementType              type() override { return EK_CUSTOM; }

  private:
    CIriceControls* m_deco;
    float           m_a;
};

class CIriceControls : public IHyprWindowDecoration {
  public:
    CIriceControls(PHLWINDOW w) : IHyprWindowDecoration(w), m_window(w) {
        g_decos.push_back(this);
        m_move   = Event::bus()->m_events.input.mouse.move.listen([this](Vector2D, Event::SCallbackInfo&) { onMove(); });
        m_button = Event::bus()->m_events.input.mouse.button.listen([this](IPointer::SButtonEvent e, Event::SCallbackInfo& info) { onButton(e, info); });
    }
    ~CIriceControls() override {
        std::erase(g_decos, this);
        shutdown();
    }
    // Drop input listeners and the timer (before the plugin's code goes away)
    void shutdown() {
        m_move.reset();
        m_button.reset();
        if (m_tileTimer) {
            m_tileTimer->cancel();
            g_pEventLoopManager->removeTimer(m_tileTimer);
            m_tileTimer.reset();
        }
    }

    SDecorationPositioningInfo getPositioningInfo() override {
        SDecorationPositioningInfo info;
        info.policy         = DECORATION_POSITION_ABSOLUTE;
        info.edges          = DECORATION_EDGE_TOP;
        info.desiredExtents = {{0, 0}, {0, 0}};
        info.reserved       = false;
        return info;
    }
    void             onPositioningReply(const SDecorationPositioningReply&) override {}
    eDecorationType  getDecorationType() override { return DECORATION_CUSTOM; }
    eDecorationLayer getDecorationLayer() override { return DECORATION_LAYER_OVERLAY; }
    uint64_t         getDecorationFlags() override { return DECORATION_ALLOWS_MOUSE_INPUT; }
    std::string      getDisplayName() override { return "iRiceControls"; }
    void             updateWindow(PHLWINDOW) override { damageEntire(); }
    void             damageEntire() override {
        if (auto z = zone(); z.w > 0)
            g_pHyprRenderer->damageBox(z.expand(4));
    }

    void draw(PHLMONITOR, float const& a) override {
        if (!visible())
            return;
        g_pHyprRenderer->m_renderPass.add(makeUnique<CControlsPass>(this, a));
    }

    // Global logical box of the hover zone, following the window as drawn
    CBox zone() {
        auto w = m_window.lock();
        if (!w || !w->m_isMapped)
            return {};
        CBox box = w->getWindowMainSurfaceBox();
        box.translate(w->m_floatingOffset);
        if (w->m_workspace && !w->m_pinned)
            box.translate(w->m_workspace->m_renderOffset->value());
        return {box.x + box.w / 2 - kZoneW / 2, box.y, kZoneW, kZoneH};
    }

    // Button i (0 close, 1 minimize, 2 zoom) in global logical px
    CBox button(int i) {
        auto z  = zone();
        auto x0 = z.x + z.w / 2 - kPillW / 2 + (kPillW - 3 * kDot - 2 * kGap) / 2;
        return {x0 + i * (kDot + kGap), z.y + kTop + (kPillH - kDot) / 2, kDot, kDot};
    }

    void renderPass(float a) {
        auto w   = m_window.lock();
        auto mon = g_pHyprRenderer->m_renderData.pMonitor.lock();
        if (!w || !mon)
            return;

        // Ease the hover expansion
        const auto now = std::chrono::steady_clock::now();
        const auto dt  = std::chrono::duration<float, std::milli>(now - m_lastFrame).count();
        m_lastFrame    = now;
        const float goal = m_hovered ? 1.f : 0.f;
        if (m_t != goal) {
            const float step = std::min(1.f, dt / kAnimMs);
            m_t              = goal > m_t ? std::min(goal, m_t + step) : std::max(goal, m_t - step);
            damageEntire(); // keep animating
        }
        const float e = 1 - (1 - m_t) * (1 - m_t) * (1 - m_t); // ease-out cubic

        const float s  = mon->m_scale;
        auto        z  = zone();
        const float pw = kHandleW + (kPillW - kHandleW) * e, ph = kHandleH + (kPillH - kHandleH) * e;
        CBox        pill{z.x + z.w / 2 - pw / 2, z.y + kTop + (kPillH - ph) / 2, pw, ph};
        pill.translate(-mon->m_position).scale(s).round();

        const CHyprColor handle = g_dark ? CHyprColor{1, 1, 1, 0.45} : CHyprColor{0, 0, 0, 0.35};
        const CHyprColor bg     = g_dark ? CHyprColor{0.11, 0.11, 0.12, 0.82} : CHyprColor{0.98, 0.98, 0.98, 0.86};
        CHyprColor       c{handle.r + (bg.r - handle.r) * e, handle.g + (bg.g - handle.g) * e, handle.b + (bg.b - handle.b) * e, (handle.a + (bg.a - handle.a) * e) * a};
        g_pHyprOpenGL->renderRect(pill, c, {.round = (int)(pill.h / 2), .roundingPower = 2.f});

        if (e < 0.35f)
            return;
        static const CHyprColor kColors[3] = {{1.0, 0.373, 0.341, 1}, {0.996, 0.737, 0.18, 1}, {0.157, 0.784, 0.251, 1}};
        const float             da         = (e - 0.35f) / 0.65f * a;
        for (int i = 0; i < 3; i++) {
            CBox b = button(i);
            b.translate(-mon->m_position).scale(s).round();
            auto col = kColors[i];
            if (m_hot == i)
                col = CHyprColor{col.r * 0.85, col.g * 0.85, col.b * 0.85, 1};
            col.a *= da;
            g_pHyprOpenGL->renderRect(b, col, {.round = (int)(b.w / 2), .roundingPower = 2.f});
        }
    }

    bool visible() {
        auto w = m_window.lock();
        return g_enabled && w && w->m_isMapped && !Fullscreen::controller()->isFullscreen(w, Fullscreen::FSMODE_FULLSCREEN);
    }

  private:
    PHLWINDOWREF                          m_window;
    CHyprSignalListener                   m_move, m_button;
    SP<CEventLoopTimer>                   m_tileTimer;
    bool                                  m_hovered = false, m_pressedHere = false;
    int                                   m_hot = -1;
    float                                 m_t   = 0;
    std::chrono::steady_clock::time_point m_lastFrame = std::chrono::steady_clock::now();

    // Is this window the one under the pointer, with nothing on top of it?
    bool pointerOnUs() {
        auto w = m_window.lock();
        if (!w || !visible())
            return false;
        if (g_pSeatManager->m_seatGrab && !g_pSeatManager->m_seatGrab->accepts(w->wlSurface()->resource()))
            return false;
        const auto              mouse = g_pInputManager->getMouseCoordsInternal();
        Desktop::CViewHitTester hit{*Desktop::viewState()};
        if (hit.windowAt(mouse, Desktop::View::RESERVED_EXTENTS | Desktop::View::INPUT_EXTENTS | Desktop::View::ALLOW_FLOATING) != w)
            return false;
        auto mon = Desktop::focusState()->monitor();
        if (!mon)
            return false;
        PHLLS    ls;
        Vector2D local;
        for (auto layer : {ZWLR_LAYER_SHELL_V1_LAYER_TOP, ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY}) {
            hit.layerSurfaceAt(mouse, &mon->m_layerSurfaceLayers[layer], &local, &ls);
            if (ls)
                return false;
        }
        return true;
    }

    int buttonAt(const Vector2D& p) {
        for (int i = 0; i < 3; i++)
            if (button(i).expand(4).containsPoint(p))
                return i;
        return -1;
    }

    void onMove() {
        const auto p       = g_pInputManager->getMouseCoordsInternal();
        const bool hovered = zone().containsPoint(p) && pointerOnUs();
        const int  hot     = hovered ? buttonAt(p) : -1;
        if (hovered == m_hovered && hot == m_hot)
            return;
        if (hovered != m_hovered)
            m_lastFrame = std::chrono::steady_clock::now();
        if (hot != m_hot)
            hot == 2 ? armTile() : disarmTile();
        m_hovered = hovered;
        m_hot     = hot;
        damageEntire();
    }

    void onButton(IPointer::SButtonEvent e, Event::SCallbackInfo& info) {
        if (e.button != 272) // BTN_LEFT; right/middle clicks must not close a window
            return;
        if (e.state != WL_POINTER_BUTTON_STATE_PRESSED) {
            if (m_pressedHere)
                info.cancelled = true;
            m_pressedHere = false;
            return;
        }
        if (!m_hovered || !pointerOnUs())
            return;
        const int i = buttonAt(g_pInputManager->getMouseCoordsInternal());
        if (i < 0)
            return; // the pill itself: let the click through to the window
        info.cancelled = true;
        m_pressedHere  = true;
        disarmTile();
        act(i);
    }

    void act(int i) {
        auto w = m_window.lock();
        if (!w)
            return;
        const auto addr = std::format("address:0x{:x}", (uintptr_t)w.get());
        if (i == 0)
            w->sendClose();
        else if (i == 1)
            g_pKeybindManager->m_dispatchers["movetoworkspacesilent"]("special:minimized," + addr);
        else {
            Desktop::focusState()->fullWindowFocus(w, Desktop::FOCUS_REASON_CLICK);
            g_pKeybindManager->m_dispatchers["fullscreen"]("1");
        }
    }

    void armTile() {
        if (!m_tileTimer) {
            m_tileTimer = makeShared<CEventLoopTimer>(std::nullopt, [this](SP<CEventLoopTimer>, void*) { postTile(); }, nullptr);
            g_pEventLoopManager->addTimer(m_tileTimer);
        }
        m_tileTimer->updateTimeout(std::chrono::milliseconds(500));
    }
    void disarmTile() {
        if (m_tileTimer)
            m_tileTimer->updateTimeout(std::nullopt);
    }
    void postTile() {
        auto w = m_window.lock();
        if (!w || m_hot != 2)
            return;
        auto b = button(2);
        g_pEventManager->postEvent(SHyprIPCEvent{"irice-controls", std::format("tile,0x{:x},{},{}", (uintptr_t)w.get(), (int)(b.x + b.w / 2), (int)(b.y + b.h))});
    }
};

std::vector<UP<IPassElement>> CControlsPass::draw() {
    m_deco->renderPass(m_a);
    return {};
}
std::optional<CBox> CControlsPass::boundingBox() {
    auto mon = g_pHyprRenderer->m_renderData.pMonitor.lock();
    if (!mon)
        return std::nullopt;
    return m_deco->zone().translate(-mon->m_position).expand(4);
}

static void addTo(PHLWINDOW w) {
    if (!w || w->m_X11DoesntWantBorders)
        return;
    if (std::ranges::any_of(w->m_windowDecorations, [](const auto& d) { return d->getDisplayName() == "iRiceControls"; }))
        return;
    HyprlandAPI::addWindowDecoration(g_ctlHandle, w, makeUnique<CIriceControls>(w));
}

static void setEnabled(bool on) {
    if (on == g_enabled)
        return;
    g_enabled = on;
    if (on) {
        // Every mapped window, also ones on other spaces or minimized
        for (auto& w : Desktop::windowState()->windows())
            if (w->m_isMapped)
                addTo(w);
    } else {
        for (auto* d : std::vector<CIriceControls*>(g_decos))
            HyprlandAPI::removeWindowDecoration(g_ctlHandle, d);
    }
    for (auto& m : State::monitorState()->monitors())
        g_pHyprRenderer->damageMonitor(m);
}

void controlsInit(HANDLE handle) {
    g_ctlHandle   = handle;
    static auto P = Event::bus()->m_events.window.open.listen([](PHLWINDOW w) {
        if (g_enabled)
            addTo(w);
    });
}

// On unload: remove our decorations now, while this code is still loaded
// (Hyprland may defer removal for hidden windows, and a deferred destructor
// or a listener left behind would call into the unloaded plugin)
void controlsExit() {
    g_enabled = false;
    g_pHyprRenderer->m_renderPass.removeAllOfType("CIriceControlsPass");
    for (auto* d : g_decos)
        d->shutdown();
    for (auto& w : Desktop::windowState()->windows()) {
        auto& decos = w->m_windowDecorations;
        for (auto& d : decos)
            if (d && d->getDisplayName() == "iRiceControls")
                g_pDecorationPositioner->uncacheDecoration(d.get());
        std::erase_if(decos, [](const auto& d) { return d && d->getDisplayName() == "iRiceControls"; });
    }
    g_decos.clear();
}

// "controls on [dark|light]" / "controls off"
std::string controlsCommand(const std::string& state, const std::string& theme) {
    if (state == "on") {
        if (!theme.empty())
            g_dark = theme != "light";
        setEnabled(true);
        for (auto* d : g_decos)
            d->damageEntire();
        return "ok";
    }
    if (state == "off") {
        setEnabled(false);
        return "ok";
    }
    return g_enabled ? "on" : "off";
}
