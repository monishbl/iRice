#pragma once
// GlassSync: keeps one window's glass shapes in the irice-glass Hyprland
// plugin up to date, from C++. It replaces the QML path (GlassWindow.syncGlass
// and Glass.pluginShape, run from a per-frame timer for a while after every
// change) with work done only on frames the window actually draws:
//   - each frame, after animations advance, it reads every registered glass
//     item's on-screen rect, opacity (through its parents) and visibility,
//     builds the plugin's numbers, and sends them only if they changed;
//   - when nothing moves the window draws no frames, and this does nothing.
// Requests go out from a worker thread (GlassSender), one at a time, newest
// per window, so neither the shell nor Hyprland ever waits on the other.
//
// QML side (glass/GlassWindow.qml): a GlassSync item inside the window;
// `add(glass)` / `remove(glass)` as glass items come and go. Each Glass item
// provides `pluginMode` (drawn by the plugin at all), `shadowMargin` and
// `pluginParams`:
//   [radius, mode, refraction, depth, bevel, frost, chroma, rim,
//    tintR, tintG, tintB, tintA, saturation, gloss]
// (radius and bevel in item pixels; the rest as the plugin takes them).
#include <QMetaProperty>
#include <QPointer>
#include <QQuickItem>
#include <QString>
#include <QVector>
#include <QtQml/qqmlregistration.h>

class GlassSync : public QQuickItem {
    Q_OBJECT
    QML_ELEMENT
    // Where the shapes go: the window's monitor and layer namespace
    Q_PROPERTY(QString monitor READ monitor WRITE setMonitor NOTIFY monitorChanged)
    Q_PROPERTY(QString ns READ ns WRITE setNs NOTIFY nsChanged)
    // Off (window hidden, plugin missing): shapes are cleared, nothing is read
    Q_PROPERTY(bool active READ active WRITE setActive NOTIFY activeChanged)
    // Plugin version: 2+ takes gloss (a 19th number per shape)
    Q_PROPERTY(int version READ version WRITE setVersion NOTIFY versionChanged)
    // Requests sent so far (diagnostics)
    Q_PROPERTY(int sent READ sent NOTIFY sentChanged)

public:
    explicit GlassSync(QQuickItem* parent = nullptr);
    ~GlassSync() override;

    QString monitor() const { return m_monitor; }
    void    setMonitor(const QString& m);
    QString ns() const { return m_ns; }
    void    setNs(const QString& n);
    bool    active() const { return m_active; }
    void    setActive(bool a);
    int     version() const { return m_version; }
    void    setVersion(int v);
    int     sent() const { return m_sent; }

    Q_INVOKABLE void add(QQuickItem* glass);
    Q_INVOKABLE void remove(QQuickItem* glass);
    // Send again even if unchanged (the plugin was reloaded and is blank)
    Q_INVOKABLE void resend();

public slots:
    // Look again on the next frame (a change the window may not redraw for)
    void poke();

signals:
    void monitorChanged();
    void nsChanged();
    void activeChanged();
    void versionChanged();
    void sentChanged();

protected:
    void itemChange(ItemChange change, const ItemChangeData& data) override;

private:
    struct Entry {
        QPointer<QQuickItem> item;
        QMetaProperty        margin, params, pluginMode;
    };
    void    attach(QQuickWindow* w);
    void    frame();
    QString shapeOf(const Entry& e) const;
    void    send(const QString& monitor, const QString& body);

    QVector<Entry>          m_glass;
    QPointer<QQuickWindow>  m_window;
    QMetaObject::Connection m_frameConn;
    QString                 m_monitor, m_ns;
    bool                    m_active  = false;
    int                     m_version = 2;
    int                     m_sent    = 0;
    QString                 m_lastBody;    // what went last ("count shapes...")
    QString                 m_lastMonitor; // and where
};
