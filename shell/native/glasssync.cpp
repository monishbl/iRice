#include "glasssync.h"

#include <QQuickWindow>
#include <QVariantList>

#include <algorithm>
#include <cerrno>
#include <chrono>
#include <cctype>
#include <cmath>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include <sys/socket.h>
#include <sys/time.h>
#include <sys/un.h>
#include <unistd.h>

// ── GlassSender ─────────────────────────────────────────────────────────────
// One worker thread for the whole shell. Each window's newest request waits
// under its key; the worker sends whatever is waiting as one Hyprland batch,
// one request at a time, so an animation costs one request per frame however
// many windows move. Blocking socket calls happen only on this thread.
namespace {

class GlassSender {
public:
    static GlassSender& get() {
        static GlassSender s;
        return s;
    }

    void submit(const std::string& key, std::string cmd) {
        {
            std::lock_guard lock(m_mutex);
            auto it = std::find_if(m_pending.begin(), m_pending.end(), [&](auto& p) { return p.first == key; });
            if (it != m_pending.end())
                it->second = std::move(cmd);
            else
                m_pending.emplace_back(key, std::move(cmd));
        }
        m_cv.notify_one();
    }

private:
    GlassSender() {
        const char* rt  = std::getenv("XDG_RUNTIME_DIR");
        const char* sig = std::getenv("HYPRLAND_INSTANCE_SIGNATURE");
        if (sig)
            m_path = std::string(rt && *rt ? rt : "/tmp") + "/hypr/" + sig + "/.socket.sock";
        m_thread = std::thread([this] { run(); });
    }
    ~GlassSender() {
        {
            std::lock_guard lock(m_mutex);
            m_stop = true;
        }
        m_cv.notify_one();
        if (m_thread.joinable())
            m_thread.join();
    }

    void run() {
        for (;;) {
            std::vector<std::pair<std::string, std::string>> batch;
            {
                std::unique_lock lock(m_mutex);
                m_cv.wait(lock, [this] { return m_stop || !m_pending.empty(); });
                if (m_stop)
                    return; // stopping, nothing left
                batch.swap(m_pending);
            }
            std::string cmd;
            if (batch.size() == 1) {
                cmd = batch[0].second;
            } else {
                cmd = "[[BATCH]]";
                for (size_t i = 0; i < batch.size(); i++)
                    cmd += (i ? ";" : "") + batch[i].second;
            }
            if (!request(cmd, batch.size())) {
                std::unique_lock lock(m_mutex);
                if (m_stop) return;
                for (auto& item : batch) {
                    const auto newer = std::find_if(m_pending.begin(), m_pending.end(), [&](auto& p) { return p.first == item.first; });
                    if (newer == m_pending.end()) m_pending.push_back(std::move(item));
                }
                m_cv.wait_for(lock, std::chrono::milliseconds(500), [this] { return m_stop; });
            }
        }
    }

    // Connect, send, read the answer to the end (3 s at most each way)
    bool request(std::string cmd, size_t expected) {
        if (m_path.empty()) return true; // no compositor in previews
        sockaddr_un addr{};
        addr.sun_family = AF_UNIX;
        if (m_path.size() >= sizeof(addr.sun_path)) return false;
        std::memcpy(addr.sun_path, m_path.c_str(), m_path.size() + 1);
        const int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
        if (fd < 0) return false;
        timeval tv{3, 0};
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));
        bool ok = false;
        if (connect(fd, reinterpret_cast<sockaddr*>(&addr), sizeof(addr)) == 0) {
            if (cmd.size() % 1023 == 0) cmd += ' ';
            size_t sent = 0;
            while (sent < cmd.size()) {
                const ssize_t n = send(fd, cmd.data() + sent, cmd.size() - sent, MSG_NOSIGNAL);
                if (n < 0 && errno == EINTR) continue;
                if (n <= 0) break;
                sent += size_t(n);
            }
            std::string reply;
            char buf[1024];
            if (sent == cmd.size()) {
                for (;;) {
                    const ssize_t n = read(fd, buf, sizeof(buf));
                    if (n < 0 && errno == EINTR) continue;
                    if (n == 0) { ok = !reply.empty(); break; }
                    if (n < 0 || reply.size() > 65536) break;
                    reply.append(buf, size_t(n));
                }
                // Each command returns "ok"; whitespace separates batch replies.
                size_t acknowledgements = 0;
                for (size_t pos = 0; ok && pos < reply.size();) {
                    if (std::isspace(static_cast<unsigned char>(reply[pos]))) { ++pos; continue; }
                    if (reply.compare(pos, 2, "ok") != 0) { ok = false; break; }
                    pos += 2;
                    ++acknowledgements;
                }
                ok = ok && acknowledgements == expected;
            }
        }
        close(fd);
        return ok;
    }

    std::string                                      m_path;
    std::mutex                                       m_mutex;
    std::condition_variable                          m_cv;
    std::vector<std::pair<std::string, std::string>> m_pending;
    bool                                             m_stop = false;
    std::thread                                      m_thread;
};

void appendNumber(QString& out, double v) {
    if (!out.isEmpty())
        out += QLatin1Char(' ');
    v = std::round(v * 1000.0) / 1000.0;
    if (v == 0)
        v = 0; // no "-0"
    out += QString::number(v, 'g', 12);
}

} // namespace

// ── GlassSync ───────────────────────────────────────────────────────────────

GlassSync::GlassSync(QQuickItem* parent) : QQuickItem(parent) {}

GlassSync::~GlassSync() {
    // The window is going: its shapes go with it
    if (!m_lastMonitor.isEmpty() && !m_lastBody.isEmpty() && m_lastBody != QLatin1String("0"))
        send(m_lastMonitor, QStringLiteral("0"));
}

void GlassSync::setMonitor(const QString& m) {
    if (m == m_monitor)
        return;
    m_monitor = m;
    emit monitorChanged();
    poke();
}

void GlassSync::setNs(const QString& n) {
    if (n == m_ns)
        return;
    m_ns = n;
    emit nsChanged();
    resend();
}

void GlassSync::setActive(bool a) {
    if (a == m_active)
        return;
    m_active = a;
    emit activeChanged();
    if (!a) {
        // Hidden or no plugin: clear what was sent
        if (!m_lastMonitor.isEmpty() && !m_lastBody.isEmpty() && m_lastBody != QLatin1String("0"))
            send(m_lastMonitor, QStringLiteral("0"));
        m_lastBody.clear();
    }
    poke();
}

void GlassSync::setVersion(int v) {
    if (v == m_version)
        return;
    m_version = v;
    emit versionChanged();
    resend();
}

void GlassSync::add(QQuickItem* glass) {
    if (!glass)
        return;
    for (const auto& e : m_glass)
        if (e.item == glass)
            return;
    const QMetaObject* mo = glass->metaObject();
    Entry              e;
    e.item   = glass;
    e.margin = mo->property(mo->indexOfProperty("shadowMargin"));
    e.params     = mo->property(mo->indexOfProperty("pluginParams"));
    e.pluginMode = mo->property(mo->indexOfProperty("pluginMode"));
    if (!e.margin.isValid() || !e.params.isValid() || !e.pluginMode.isValid())
        return;
    // A material change may not redraw the window: look again
    const QMetaMethod pokeSlot = metaObject()->method(metaObject()->indexOfSlot("poke()"));
    for (const auto& prop : {e.params, e.pluginMode})
        if (prop.hasNotifySignal())
            connect(glass, prop.notifySignal(), this, pokeSlot);
    m_glass.push_back(e);
    poke();
}

void GlassSync::remove(QQuickItem* glass) {
    m_glass.erase(std::remove_if(m_glass.begin(), m_glass.end(), [&](const Entry& e) { return e.item.isNull() || e.item == glass; }), m_glass.end());
    if (glass)
        disconnect(glass, nullptr, this, nullptr);
    poke();
}

void GlassSync::resend() {
    m_lastBody.clear();
    poke();
}

void GlassSync::poke() {
    if (m_window)
        m_window->update();
}

void GlassSync::itemChange(ItemChange change, const ItemChangeData& data) {
    if (change == ItemSceneChange)
        attach(data.window);
    QQuickItem::itemChange(change, data);
}

void GlassSync::attach(QQuickWindow* w) {
    if (w == m_window)
        return;
    disconnect(m_frameConn);
    m_window = w;
    if (w)
        m_frameConn = connect(w, &QQuickWindow::afterAnimating, this, &GlassSync::frame);
    poke();
}

// Every frame the window draws (animations have just advanced): send the
// shapes if they changed
void GlassSync::frame() {
    if (!m_active || m_monitor.isEmpty() || m_ns.isEmpty())
        return;
    QString body;
    int     count = 0;
    for (const auto& e : m_glass) {
        const QString s = shapeOf(e);
        if (s.isEmpty())
            continue;
        body += QLatin1Char(' ') + s;
        count++;
    }
    body = QString::number(count) + body;
    if (body == m_lastBody && m_monitor == m_lastMonitor)
        return;
    // Moved to another screen: clear the old one
    if (!m_lastMonitor.isEmpty() && m_lastMonitor != m_monitor && !m_lastBody.isEmpty() && m_lastBody != QLatin1String("0"))
        send(m_lastMonitor, QStringLiteral("0"));
    m_lastBody    = body;
    m_lastMonitor = m_monitor;
    send(m_monitor, body);
}

// One glass item's numbers, in window coordinates (following parent moves,
// scales and fades), or empty when it shouldn't show
QString GlassSync::shapeOf(const Entry& e) const {
    QQuickItem* it = e.item.data();
    if (!it || !it->isVisible() || !e.pluginMode.read(it).toBool())
        return {};
    double o = 1;
    for (QQuickItem* p = it; p; p = p->parentItem())
        o *= p->opacity();
    if (o <= 0.002)
        return {};
    const double m  = e.margin.read(it).toDouble();
    const double w0 = it->width() - 2 * m, h0 = it->height() - 2 * m;
    if (w0 < 1 || h0 < 1)
        return {};
    const QRectF r = it->mapRectToScene(QRectF(m, m, w0, h0));
    if (r.width() < 1 || r.height() < 1)
        return {};
    const QVariantList p = e.params.read(it).toList();
    if (p.size() < 14)
        return {};
    const double k = r.width() / std::max(1.0, w0);
    // [x, y, w, h, radius, mode, refraction, depth, bevel, frost, chroma, rim,
    //  r, g, b, a, saturation, opacity, (gloss)]
    QString s;
    s.reserve(160);
    appendNumber(s, r.x());
    appendNumber(s, r.y());
    appendNumber(s, r.width());
    appendNumber(s, r.height());
    appendNumber(s, p[0].toDouble() * k);
    for (int i = 1; i <= 3; i++)
        appendNumber(s, p[i].toDouble());
    appendNumber(s, p[4].toDouble() * k);
    for (int i = 5; i <= 12; i++)
        appendNumber(s, p[i].toDouble());
    appendNumber(s, o);
    if (m_version >= 2)
        appendNumber(s, p[13].toDouble());
    return s;
}

void GlassSync::send(const QString& monitor, const QString& body) {
    const QString key = monitor + QLatin1Char('\n') + m_ns;
    GlassSender::get().submit(key.toStdString(), (QStringLiteral("irice-glass set ") + monitor + QLatin1Char(' ') + m_ns + QLatin1Char(' ') + body).toStdString());
    m_sent++;
    emit sentChanged();
}
