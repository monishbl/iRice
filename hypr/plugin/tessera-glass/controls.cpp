// SPDX-License-Identifier: GPL-3.0-or-later
// Window controls drawn by Hyprland: a small pill at the top centre of every
// window. Hover it and it grows into close, minimize and zoom. Because the
// compositor draws it as part of the window, it moves with the window on the
// same frame (the shell-drawn pill trails behind while a window moves).
//
// hyprctl tessera-glass controls on [dark|light]
// hyprctl tessera-glass controls off
// hyprctl tessera-glass controls-style glass | material <container rrggbb> <ink rrggbb>
//   glass: traffic lights (close, minimize, zoom from the left); material:
//   round icon buttons on a tonal pill, minimize, maximize and close (from
//   the left, close last), a state layer under the hovered one (close: red)
//
// Resting on the zoom button for half a second posts the IPC event
// `tessera-controls>>tile,<address>,<x>,<y>` so the shell can show its tile grid.

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
#include <cmath>
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
// The Material look (controls-style material …): its container and icon colours
static bool       g_material = false;
static CHyprColor g_mBg{0.17, 0.17, 0.19, 1}, g_mInk{0.8, 0.8, 0.84, 1};
// Sizes, logical px: Glass's dots, or Material's round buttons
static float pillW() { return g_material ? 92 : kPillW; }
static float pillH() { return g_material ? 26 : kPillH; }
static float dotD() { return g_material ? 22 : kDot; }
static float dotGap() { return g_material ? 4 : kGap; }
// Where button i (0 close, 1 minimize, 2 zoom) sits, left to right
static int slotOf(int i) { return g_material ? (i == 0 ? 2 : i - 1) : i; }

class CTesseraControls;
static std::vector<CTesseraControls*> g_decos;

class CControlsPass : public IPassElement {
  public:
    CControlsPass(CTesseraControls* d, float a) : m_deco(d), m_a(a) {}
    std::vector<UP<IPassElement>> draw() override;
    bool                          needsLiveBlur() override { return false; }
    bool                          needsPrecomputeBlur() override { return false; }
    std::optional<CBox>           boundingBox() override;
    const char*                   passName() override { return "CTesseraControlsPass"; }
    ePassElementType              type() override { return EK_CUSTOM; }

  private:
    CTesseraControls* m_deco;
    float           m_a;
};

class CTesseraControls : public IHyprWindowDecoration {
  public:
    CTesseraControls(PHLWINDOW w) : IHyprWindowDecoration(w), m_window(w) {
        g_decos.push_back(this);
        m_move   = Event::bus()->m_events.input.mouse.move.listen([this](Vector2D, Event::SCallbackInfo&) { onMove(); });
        m_button = Event::bus()->m_events.input.mouse.button.listen([this](IPointer::SButtonEvent e, Event::SCallbackInfo& info) { onButton(e, info); });
    }
    ~CTesseraControls() override {
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
    std::string      getDisplayName() override { return "TesseraControls"; }
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
        auto x0 = z.x + z.w / 2 - pillW() / 2 + (pillW() - 3 * dotD() - 2 * dotGap()) / 2;
        return {x0 + slotOf(i) * (dotD() + dotGap()), z.y + kTop + (pillH() - dotD()) / 2, dotD(), dotD()};
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
        const float pw = kHandleW + (pillW() - kHandleW) * e, ph = kHandleH + (pillH() - kHandleH) * e;
        CBox        pill{z.x + z.w / 2 - pw / 2, z.y + kTop + (pillH() - ph) / 2, pw, ph};
        pill.translate(-mon->m_position).scale(s).round();

        const CHyprColor handle = g_material ? CHyprColor{g_mInk.r, g_mInk.g, g_mInk.b, 0.5} : g_dark ? CHyprColor{1, 1, 1, 0.45} : CHyprColor{0, 0, 0, 0.35};
        const CHyprColor bg     = g_material ? g_mBg : g_dark ? CHyprColor{0.11, 0.11, 0.12, 0.82} : CHyprColor{0.98, 0.98, 0.98, 0.86};
        CHyprColor       c{handle.r + (bg.r - handle.r) * e, handle.g + (bg.g - handle.g) * e, handle.b + (bg.b - handle.b) * e, (handle.a + (bg.a - handle.a) * e) * a};
        g_pHyprOpenGL->renderRect(pill, c, {.round = (int)(pill.h / 2), .roundingPower = 2.f});

        if (e < 0.35f)
            return;
        if (g_material) {
            renderMaterial(mon, (e - 0.35f) / 0.65f * a);
            return;
        }
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

    // Material: each button a circle (a state layer while hovered; close
    // turns red), its icon drawn in the ink colour: a bar (minimize), a
    // square outline (maximize), a cross of two turned bars (close)
    void renderMaterial(PHLMONITOR mon, float da) {
        const float s   = mon->m_scale;
        auto        ink = [&](float al) { return CHyprColor{g_mInk.r, g_mInk.g, g_mInk.b, al * da}; };
        auto        rect = [&](double x, double y, double w, double h, const CHyprColor& c, double rot = 0) {
            CBox b{x, y, w, h};
            b.translate(-mon->m_position).scale(s);
            b.rot = rot;
            g_pHyprOpenGL->renderRect(b, c, {.round = (int)std::round(std::min(b.w, b.h) / 2), .roundingPower = 2.f});
        };
        for (int i = 0; i < 3; i++) {
            const CBox b  = button(i);
            const bool hot = m_hot == i;
            CBox       c  = b;
            c.translate(-mon->m_position).scale(s).round();
            if (hot)
                g_pHyprOpenGL->renderRect(c, i == 0 ? CHyprColor{0.86, 0.15, 0.15, da} : ink(0.14), {.round = (int)(c.w / 2), .roundingPower = 2.f});
            const CHyprColor col = hot && i == 0 ? CHyprColor{1, 1, 1, da} : ink(1);
            const double     cx = b.x + b.w / 2, cy = b.y + b.h / 2, t = 1.6;
            if (i == 1) {
                rect(cx - 5, cy - t / 2, 10, t, col);
            } else if (i == 2) {
                const double h = 9;
                rect(cx - h / 2, cy - h / 2, h, t, col);
                rect(cx - h / 2, cy + h / 2 - t, h, t, col);
                rect(cx - h / 2, cy - h / 2, t, h, col);
                rect(cx + h / 2 - t, cy - h / 2, t, h, col);
            } else {
                const double l = 12;
                rect(cx - l / 2, cy - t / 2, l, t, col, M_PI / 4);
                rect(cx - l / 2, cy - t / 2, l, t, col, -M_PI / 4);
            }
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
        g_pEventManager->postEvent(SHyprIPCEvent{"tessera-controls", std::format("tile,0x{:x},{},{}", (uintptr_t)w.get(), (int)(b.x + b.w / 2), (int)(b.y + b.h))});
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
    if (std::ranges::any_of(w->m_windowDecorations, [](const auto& d) { return d->getDisplayName() == "TesseraControls"; }))
        return;
    HyprlandAPI::addWindowDecoration(g_ctlHandle, w, makeUnique<CTesseraControls>(w));
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
        for (auto* d : std::vector<CTesseraControls*>(g_decos))
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
    g_pHyprRenderer->m_renderPass.removeAllOfType("CTesseraControlsPass");
    for (auto* d : g_decos)
        d->shutdown();
    for (auto& w : Desktop::windowState()->windows()) {
        auto& decos = w->m_windowDecorations;
        for (auto& d : decos)
            if (d && d->getDisplayName() == "TesseraControls")
                g_pDecorationPositioner->uncacheDecoration(d.get());
        std::erase_if(decos, [](const auto& d) { return d && d->getDisplayName() == "TesseraControls"; });
    }
    g_decos.clear();
}

// "controls-style glass" / "controls-style material <rrggbb> <rrggbb>"
static bool hexColor(const std::string& h, CHyprColor& out) {
    if (h.size() != 6 || h.find_first_not_of("0123456789abcdefABCDEF") != std::string::npos)
        return false;
    const auto v = std::stoul(h, nullptr, 16);
    out          = CHyprColor{((v >> 16) & 255) / 255.0, ((v >> 8) & 255) / 255.0, (v & 255) / 255.0, 1};
    return true;
}
std::string controlsStyleCommand(const std::string& style, const std::string& bg, const std::string& ink) {
    if (style == "glass") {
        g_material = false;
    } else if (style == "material") {
        CHyprColor b, i;
        if (!hexColor(bg, b) || !hexColor(ink, i))
            return "error: controls-style material <container rrggbb> <ink rrggbb>";
        g_material = true;
        g_mBg      = b;
        g_mInk     = i;
    } else {
        return "error: controls-style glass | material <container> <ink>";
    }
    for (auto* d : g_decos)
        d->damageEntire();
    return "ok";
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
