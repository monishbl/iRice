// irice-glass: Liquid Glass for iRice, drawn by Hyprland itself.
//
// The shell tells the plugin where its glass shapes are (per monitor and
// layer namespace) with `hyprctl irice-glass ...`. When Hyprland draws one of
// those layers with blur, this plugin first paints the glass: it copies what's
// already on screen behind the shape and runs a refraction shader over it
// (lensing rim, optional thick "depth" lens, frost from Hyprland's blur,
// dispersion, tint, specular rim), then lets Hyprland draw the layer's own
// content on top. Nothing is drawn unless Hyprland redraws that area, so
// static glass costs nothing and glass over video updates live.
//
// hyprctl irice-glass set <monitor> <namespace> <n> [<16 floats per shape>...]
// hyprctl irice-glass clear [<monitor> <namespace>]
// hyprctl irice-glass ping            -> "irice-glass 2" (2: shapes may carry gloss)
// hyprctl irice-glass controls on|off [dark|light]   (see controls.cpp)
//
// Shape floats (layer-local logical px): x y w h radius mode refraction depth
// bevel frost chroma rim tintR tintG tintB tintA saturation opacity
// [gloss] (mode 0 liquid, 1 frosted, 2 solid; tint straight alpha; gloss 0..1
// scales the 3D lighting: 1 glass, ~0 frost (an even thin border), optional
// so shells that send 18 numbers still work). See kFloats.

#define WLR_USE_UNSTABLE
#include <hyprland/src/plugins/PluginAPI.hpp>
#include <hyprland/src/render/OpenGL.hpp>
#include <hyprland/src/render/Renderer.hpp>
#include <hyprland/src/render/gl/GLFramebuffer.hpp>
#include <hyprland/src/desktop/view/LayerSurface.hpp>
#include <hyprland/src/output/Monitor.hpp>
#include <hyprland/src/state/MonitorState.hpp>
#include <GLES3/gl32.h>
#include <map>
#include <mutex>
#include <sstream>
#include <string>
#include <vector>

using namespace Render;
using namespace Render::GL;

void        controlsInit(HANDLE);
void        controlsExit();
std::string controlsCommand(const std::string& state, const std::string& theme);

static HANDLE        g_handle = nullptr;
static CFunctionHook* g_hook  = nullptr;
static SP<SHyprCtlCommand> g_cmd;

constexpr int kFloats = 19; // the 19th (gloss) is optional
struct SShape {
    float v[kFloats];
};
// key: monitor + "\n" + namespace
static std::map<std::string, std::vector<SShape>> g_shapes;

// ── GL resources ────────────────────────────────────────────────────────────
static GLuint g_prog = 0, g_vao = 0, g_copyFB = 0, g_copyTex = 0;
static int    g_copyW = 0, g_copyH = 0;
static GLint  u_sharp, u_blurred, u_fbSize, u_rect, u_copyRect, u_radius, u_mode, u_refraction, u_depth, u_bevel, u_frost, u_chroma, u_rim, u_tint, u_saturation,
    u_opacity, u_solid, u_hasBlur, u_gloss;

static const char* kVert = R"(#version 300 es
precision highp float;
const vec2 pos[3] = vec2[3](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
void main() { gl_Position = vec4(pos[gl_VertexID], 0.0, 1.0); }
)";

// Coordinates are framebuffer pixels (gl_FragCoord), so both the copy of the
// screen and Hyprland's blurred background line up without a projection.
static const char* kFrag = R"(#version 300 es
precision highp float;
uniform sampler2D sharp;     // copy of the screen behind (full FB size)
uniform sampler2D blurred;   // Hyprland's blurred background (full FB size)
uniform vec2  fbSize;
uniform vec4  rect;          // glass rect: x, y, w, h (FB pixels, top-down rows)
uniform vec4  copyRect;      // area of `sharp` that holds valid pixels
uniform float radius, mode, refraction, depth, bevel, frost, chroma, rim, saturation, opacity, hasBlur, gloss;
uniform vec4  tint, solidColor;
out vec4 fragColor;

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

vec3 sampleSharp(vec2 px) {
    px = clamp(px, copyRect.xy + 0.5, copyRect.xy + copyRect.zw - 0.5);
    return texture(sharp, px / fbSize).rgb;
}

void main() {
    vec2 px = gl_FragCoord.xy;
    vec2 hb = rect.zw * 0.5;
    vec2 c = rect.xy + hb;
    float r = min(radius, min(hb.x, hb.y));
    vec2 p = px - c;
    float d = sdRoundBox(p, hb, r);
    float cov = clamp(0.5 - d, 0.0, 1.0); // antialiased edge
    if (cov <= 0.0)
        discard;

    // Outward normal from the distance field
    vec2 e = vec2(0.75, 0.0);
    vec2 n = vec2(sdRoundBox(p + e.xy, hb, r) - sdRoundBox(p - e.xy, hb, r), sdRoundBox(p + e.yx, hb, r) - sdRoundBox(p - e.yx, hb, r));
    n = length(n) > 1e-4 ? normalize(n) : vec2(0.0);
    float inside = -d;

    vec3 col;
    float t = clamp(inside / max(bevel, 1.0), 0.0, 1.0);
    float rimBend = (1.0 - t) * (1.0 - t); // strongest right at the edge

    if (mode < 1.5) {
        // Lens: the rim samples from just outside the shape (light folds in);
        // depth adds a thick-lens magnification towards the centre
        vec2 sp = px + n * refraction * rimBend;
        sp += (c - px) * depth * 0.14 * (1.0 - rimBend * 0.6);
        vec3 sharpC;
        if (chroma > 0.0) {
            vec2 off = n * refraction * rimBend * chroma * 0.4;
            sharpC = vec3(sampleSharp(sp + off).r, sampleSharp(sp).g, sampleSharp(sp - off).b);
        } else {
            sharpC = sampleSharp(sp);
        }
        vec3 blurC = hasBlur > 0.5 ? texture(blurred, clamp(sp, vec2(0.5), fbSize - 0.5) / fbSize).rgb : sharpC;
        // Frost is even across the surface, like iOS: the lens bends the
        // frosted image at the edge rather than leaving a clear rim
        col = mix(sharpC, blurC, mode > 0.5 ? 1.0 : frost);
        float l = dot(col, vec3(0.2126, 0.7152, 0.0722));
        col = mix(vec3(l), col, saturation);
        col = mix(col, tint.rgb, tint.a);
    } else {
        col = solidColor.rgb;
    }

    // Light from the top left (rows count down from the top). `rim` scales
    // all of it, so the material presets fade it out toward solid.
    vec2  L = normalize(vec2(-0.55, -0.83));
    float facing = dot(n, L);
    float line = 1.0 - smoothstep(0.0, 1.2 + bevel * 0.05, inside);        // hairline edge
    float band = 1.0 - smoothstep(0.0, max(bevel * 0.22, 2.0), inside);    // thin highlight band at the edge
    float key  = pow(max(facing, 0.0), 10.0);                              // narrow specular where the light hits
    float back = pow(max(-facing, 0.0), 8.0) * 0.25;                       // light passing through, opposite edge
    // Edge: an even hairline when frosted, lit from one side when glassy
    float lineLit = mix(0.45, 0.2 + 0.6 * max(facing, 0.0), gloss);
    col += rim * line * lineLit;
    // The 3D part fades with gloss: specular, transmitted glint, inner glow
    col += rim * gloss * band * (key * 0.55 + back);
    col += rim * gloss * 0.08 * (1.0 - t) * max(facing, 0.0);
    // Sheen across the top, as if the surface catches the sky
    float yIn = clamp((px.y - (c.y - hb.y)) / max(rect.w, 1.0), 0.0, 1.0);
    col += rim * gloss * 0.04 * pow(1.0 - yIn, 4.0);
    // Inner shadow along the bottom edge gives it thickness
    col *= 1.0 - rim * gloss * 0.12 * band * max(n.y, 0.0) * (1.0 - key);
    // Thick lens: a soft bulge highlight above and left of centre
    if (depth > 0.0) {
        vec2 q = (p + hb * 0.35) / max(min(hb.x, hb.y), 1.0);
        col += rim * gloss * depth * 0.12 * exp(-dot(q, q) * 4.0);
    }

    float a = cov * opacity;
    fragColor = vec4(clamp(col, 0.0, 1.0) * a, a);
}
)";

static GLuint compile(GLenum type, const char* src) {
    GLuint s = glCreateShader(type);
    glShaderSource(s, 1, &src, nullptr);
    glCompileShader(s);
    GLint ok = 0;
    glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
    if (!ok) {
        char log[1024];
        glGetShaderInfoLog(s, sizeof log, nullptr, log);
        HyprlandAPI::addNotification(g_handle, std::string("[irice-glass] shader error: ") + log, CHyprColor{1, 0.3, 0.3, 1}, 8000);
        glDeleteShader(s);
        return 0;
    }
    return s;
}

static bool ensureProgram() {
    if (g_prog)
        return true;
    GLuint vs = compile(GL_VERTEX_SHADER, kVert), fs = compile(GL_FRAGMENT_SHADER, kFrag);
    if (!vs || !fs)
        return false;
    g_prog = glCreateProgram();
    glAttachShader(g_prog, vs);
    glAttachShader(g_prog, fs);
    glLinkProgram(g_prog);
    glDeleteShader(vs);
    glDeleteShader(fs);
    GLint ok = 0;
    glGetProgramiv(g_prog, GL_LINK_STATUS, &ok);
    if (!ok) {
        glDeleteProgram(g_prog);
        g_prog = 0;
        return false;
    }
#define U(name) u_##name = glGetUniformLocation(g_prog, #name)
    U(sharp);
    U(blurred);
    U(fbSize);
    U(rect);
    U(copyRect);
    U(radius);
    U(mode);
    U(refraction);
    U(depth);
    U(bevel);
    U(frost);
    U(chroma);
    U(rim);
    U(tint);
    U(saturation);
    U(opacity);
    u_solid = glGetUniformLocation(g_prog, "solidColor");
    U(hasBlur);
    U(gloss);
#undef U
    glGenVertexArrays(1, &g_vao);
    return true;
}

static void ensureCopyTarget(int w, int h) {
    if (g_copyTex && g_copyW == w && g_copyH == h)
        return;
    if (!g_copyTex) {
        glGenTextures(1, &g_copyTex);
        glGenFramebuffers(1, &g_copyFB);
    }
    glBindTexture(GL_TEXTURE_2D, g_copyTex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glBindFramebuffer(GL_FRAMEBUFFER, g_copyFB);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, g_copyTex, 0);
    g_copyW = w;
    g_copyH = h;
}

// Saves and restores the GL state we touch, so Hyprland's own state caches
// (current program, caps, viewport) stay true.
struct SGLState {
    GLint prog, vao, activeTex, tex0, tex1, drawFB, readFB, viewport[4], scissorBox[4], blendSrcRGB, blendDstRGB, blendSrcA, blendDstA;
    GLboolean blend, scissor, stencil;
    void save() {
        glGetIntegerv(GL_CURRENT_PROGRAM, &prog);
        glGetIntegerv(GL_VERTEX_ARRAY_BINDING, &vao);
        glGetIntegerv(GL_ACTIVE_TEXTURE, &activeTex);
        glActiveTexture(GL_TEXTURE0);
        glGetIntegerv(GL_TEXTURE_BINDING_2D, &tex0);
        glActiveTexture(GL_TEXTURE1);
        glGetIntegerv(GL_TEXTURE_BINDING_2D, &tex1);
        glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &drawFB);
        glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &readFB);
        glGetIntegerv(GL_VIEWPORT, viewport);
        glGetIntegerv(GL_SCISSOR_BOX, scissorBox);
        glGetIntegerv(GL_BLEND_SRC_RGB, &blendSrcRGB);
        glGetIntegerv(GL_BLEND_DST_RGB, &blendDstRGB);
        glGetIntegerv(GL_BLEND_SRC_ALPHA, &blendSrcA);
        glGetIntegerv(GL_BLEND_DST_ALPHA, &blendDstA);
        blend   = glIsEnabled(GL_BLEND);
        scissor = glIsEnabled(GL_SCISSOR_TEST);
        stencil = glIsEnabled(GL_STENCIL_TEST);
    }
    void restore() {
        glUseProgram(prog);
        glBindVertexArray(vao);
        glActiveTexture(GL_TEXTURE1);
        glBindTexture(GL_TEXTURE_2D, tex1);
        glActiveTexture(GL_TEXTURE0);
        glBindTexture(GL_TEXTURE_2D, tex0);
        glActiveTexture(activeTex);
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, drawFB);
        glBindFramebuffer(GL_READ_FRAMEBUFFER, readFB);
        glViewport(viewport[0], viewport[1], viewport[2], viewport[3]);
        glScissor(scissorBox[0], scissorBox[1], scissorBox[2], scissorBox[3]);
        glBlendFuncSeparate(blendSrcRGB, blendDstRGB, blendSrcA, blendDstA);
        blend ? glEnable(GL_BLEND) : glDisable(GL_BLEND);
        scissor ? glEnable(GL_SCISSOR_TEST) : glDisable(GL_SCISSOR_TEST);
        stencil ? glEnable(GL_STENCIL_TEST) : glDisable(GL_STENCIL_TEST);
    }
};

static bool g_lastBlur = false;
static void drawGlass(const std::vector<SShape>& shapes, const CBox& box, const CHyprOpenGLImpl::STextureRenderData& data, PHLMONITOR mon) {
    auto& rd = g_pHyprRenderer->m_renderData;
    if (!rd.currentFB || !ensureProgram())
        return;
    auto* fb = GLFB(rd.currentFB);
    if (!fb)
        return;
    const int   fbW = (int)rd.currentFB->m_size.x, fbH = (int)rd.currentFB->m_size.y;
    const float scale = mon->m_scale;
    if (fbW <= 0 || fbH <= 0)
        return;

    SGLState st;
    st.save();
    ensureCopyTarget(fbW, fbH);

    GLuint blurTex = data.blurredBG ? data.blurredBG->m_texID : 0;
    g_lastBlur      = blurTex != 0;

    for (const auto& s : shapes) {
        const float* v = s.v;
        if (v[17] <= 0.001f || v[2] < 1 || v[3] < 1)
            continue;
        // Shape in FB pixels. Hyprland's framebuffers are stored top-down, so
        // FB rows (and gl_FragCoord.y) already count from the top.
        const float x = box.x + v[0] * scale, yTop = box.y + v[1] * scale, w = v[2] * scale, h = v[3] * scale;
        const float yGL = yTop;

        // Copy what's behind (plus room for the refraction to reach in from outside)
        const int margin = (int)std::ceil(v[6] * scale + 4 + v[7] * std::max(w, h) * 0.08f);
        int cx0 = std::max(0, (int)std::floor(x) - margin), cy0 = std::max(0, (int)std::floor(yGL) - margin);
        int cx1 = std::min(fbW, (int)std::ceil(x + w) + margin), cy1 = std::min(fbH, (int)std::ceil(yGL + h) + margin);
        if (cx1 <= cx0 || cy1 <= cy0)
            continue;
        glDisable(GL_SCISSOR_TEST);
        glBindFramebuffer(GL_READ_FRAMEBUFFER, fb->getFBID());
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, g_copyFB);
        glBlitFramebuffer(cx0, cy0, cx1, cy1, cx0, cy0, cx1, cy1, GL_COLOR_BUFFER_BIT, GL_NEAREST);

        // Draw the glass into the current framebuffer, only where damaged
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, fb->getFBID());
        glViewport(0, 0, fbW, fbH);
        glUseProgram(g_prog);
        glBindVertexArray(g_vao);
        glActiveTexture(GL_TEXTURE1);
        glBindTexture(GL_TEXTURE_2D, blurTex);
        glActiveTexture(GL_TEXTURE0);
        glBindTexture(GL_TEXTURE_2D, g_copyTex);
        glUniform1i(u_sharp, 0);
        glUniform1i(u_blurred, 1);
        glUniform2f(u_fbSize, fbW, fbH);
        glUniform4f(u_rect, x, yGL, w, h);
        glUniform4f(u_copyRect, cx0, cy0, cx1 - cx0, cy1 - cy0);
        glUniform1f(u_radius, v[4] * scale);
        glUniform1f(u_mode, v[5]);
        glUniform1f(u_refraction, v[6] * scale);
        glUniform1f(u_depth, v[7]);
        glUniform1f(u_bevel, v[8] * scale);
        glUniform1f(u_frost, v[9]);
        glUniform1f(u_chroma, v[10]);
        glUniform1f(u_rim, v[11]);
        glUniform4f(u_tint, v[12], v[13], v[14], v[15]);
        glUniform1f(u_saturation, v[16]);
        glUniform1f(u_opacity, v[17]);
        glUniform4f(u_solid, v[12], v[13], v[14], 1.0f);
        glUniform1f(u_hasBlur, blurTex ? 1.f : 0.f);
        glUniform1f(u_gloss, v[18]);
        glEnable(GL_BLEND);
        glBlendFuncSeparate(GL_ONE, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        glDisable(GL_STENCIL_TEST);
        glEnable(GL_SCISSOR_TEST);

        CRegion region(x - 1, yTop - 1, w + 2, h + 2);
        if (data.damage)
            region.intersect(*data.damage);
        for (auto& r : region.getRects()) {
            glScissor(r.x1, r.y1, r.x2 - r.x1, r.y2 - r.y1);
            glDrawArrays(GL_TRIANGLES, 0, 3);
        }
    }
    st.restore();
}

typedef void (*origRenderTextureWithBlur)(void*, SP<ITexture>, const CBox&, const CHyprOpenGLImpl::STextureRenderData&);

// Diagnostics for `hyprctl irice-glass stats`
static unsigned long g_calls = 0, g_drawn = 0;
static std::string   g_lastNs, g_lastBox;

static void hkRenderTextureWithBlur(void* thisptr, SP<ITexture> tex, const CBox& box, const CHyprOpenGLImpl::STextureRenderData& data) {
    auto ls  = data.currentLS.lock();
    g_calls++;
    if (ls) {
        g_lastNs  = ls->m_namespace;
        g_lastBox = std::to_string((int)box.x) + "," + std::to_string((int)box.y) + " " + std::to_string((int)box.w) + "x" + std::to_string((int)box.h);
    }
    auto mon = g_pHyprRenderer->m_renderData.pMonitor.lock();
    if (ls && mon && mon->m_transform == WL_OUTPUT_TRANSFORM_NORMAL) {
        auto it = g_shapes.find(mon->m_name + "\n" + ls->m_namespace);
        if (it != g_shapes.end() && !it->second.empty()) {
            drawGlass(it->second, box, data, mon);
            g_drawn++;
            // Hyprland's own blur fill would cover the glass: draw it fully transparent
            auto d2  = data;
            d2.blurA = 0.f;
            ((origRenderTextureWithBlur)g_hook->m_original)(thisptr, tex, box, d2);
            return;
        }
    }
    ((origRenderTextureWithBlur)g_hook->m_original)(thisptr, tex, box, data);
}

// hyprctl irice-glass ...
static std::string onCommand(eHyprCtlOutputFormat, std::string request) {
    std::istringstream in(request);
    std::string        name, verb;
    in >> name >> verb;
    if (verb == "ping")
        return "irice-glass 2";
    if (verb == "controls") {
        std::string state, theme;
        in >> state >> theme;
        return controlsCommand(state, theme);
    }
    if (verb == "stats")
        return "calls " + std::to_string(g_calls) + " drawn " + std::to_string(g_drawn) + " last " + g_lastNs + " " + g_lastBox + " shapesets " + std::to_string(g_shapes.size()) + " blur " + (g_lastBlur ? "yes" : "no");
    if (verb == "clear") {
        std::string mon, ns;
        if (in >> mon >> ns)
            g_shapes.erase(mon + "\n" + ns);
        else
            g_shapes.clear();
        return "ok";
    }
    if (verb == "set") {
        std::string mon, ns;
        int         n = 0;
        if (!(in >> mon >> ns >> n) || n < 0 || n > 64)
            return "error: usage: set <monitor> <namespace> <n> <floats...>";
        std::vector<float> nums;
        for (float f; in >> f;)
            nums.push_back(f);
        // 19 numbers per shape, or 18 from older shells (gloss then defaults to 1)
        const int per = n > 0 && nums.size() == (size_t)n * kFloats ? kFloats : kFloats - 1;
        if (nums.size() != (size_t)n * per)
            return "error: expected " + std::to_string(n * kFloats) + " numbers";
        std::vector<SShape> shapes(n);
        for (int i = 0; i < n; i++) {
            for (int k = 0; k < per; k++)
                shapes[i].v[k] = nums[i * per + k];
            if (per < kFloats)
                shapes[i].v[kFloats - 1] = 1.f;
        }
        if (n == 0)
            g_shapes.erase(mon + "\n" + ns);
        else
            g_shapes[mon + "\n" + ns] = std::move(shapes);
        // Repaint so the change shows even if nothing else moved
        for (auto& m : State::monitorState()->monitors())
            if (m->m_name == mon)
                g_pHyprRenderer->damageMonitor(m);
        return "ok";
    }
    return "error: unknown verb (set, clear, ping, stats, controls)";
}

APICALL EXPORT std::string PLUGIN_API_VERSION() {
    return HYPRLAND_API_VERSION;
}

APICALL EXPORT PLUGIN_DESCRIPTION_INFO PLUGIN_INIT(HANDLE handle) {
    g_handle = handle;
    if (std::string(__hyprland_api_get_hash()) != std::string(__hyprland_api_get_client_hash())) {
        HyprlandAPI::addNotification(handle, "[irice-glass] Built for a different Hyprland version; rebuild it (iRice falls back to Frosted glass).", CHyprColor{1, 0.3, 0.3, 1},
                                     8000);
        throw std::runtime_error("[irice-glass] version mismatch");
    }

    void* target = nullptr;
    for (auto& f : HyprlandAPI::findFunctionsByName(handle, "renderTextureWithBlurInternal"))
        if (f.demangled.find("CHyprOpenGLImpl::renderTextureWithBlurInternal") != std::string::npos)
            target = f.address;
    if (!target) {
        HyprlandAPI::addNotification(handle, "[irice-glass] Couldn't find Hyprland's blur renderer; glass stays Frosted.", CHyprColor{1, 0.3, 0.3, 1}, 8000);
        throw std::runtime_error("[irice-glass] hook target missing");
    }
    g_hook = HyprlandAPI::createFunctionHook(handle, target, (void*)&hkRenderTextureWithBlur);
    if (!g_hook || !g_hook->hook())
        throw std::runtime_error("[irice-glass] hook failed");

    controlsInit(handle);
    g_cmd = HyprlandAPI::registerHyprCtlCommand(handle, SHyprCtlCommand{.name = "irice-glass", .exact = false, .fn = onCommand});
    return {"irice-glass", "Liquid Glass for the iRice shell", "iRice", "1.0"};
}

APICALL EXPORT void PLUGIN_EXIT() {
    g_shapes.clear();
    controlsExit();
    if (g_cmd)
        HyprlandAPI::unregisterHyprCtlCommand(g_handle, g_cmd);
    // GL objects die with the context; the hook is removed by Hyprland
}
