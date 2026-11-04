#!/usr/bin/env python3
# A window that redraws every frame, like a playing video or a scrolling page,
# for measuring what the glass costs under load (./dev.sh perf … load): it
# fills the screen, so it sits under the menu bar, the island and the dock.
import math

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk  # noqa: E402


def activate(app):
    win = Gtk.ApplicationWindow(application=app, title="redraw")
    win.set_default_size(1600, 900)
    area = Gtk.DrawingArea(hexpand=True, vexpand=True)

    def draw(_area, cr, w, h):
        t = area.get_frame_clock().get_frame_time() / 1e6 if area.get_frame_clock() else 0
        # moving colour bands over the whole window: every pixel changes
        for i in range(12):
            x = (i / 12 + t * 0.15) % 1.0
            cr.set_source_rgb(0.5 + 0.5 * math.sin(i + t), 0.5 + 0.5 * math.sin(i * 1.7 + t), 0.6)
            cr.rectangle((x - 1 / 12) * w, 0, w / 12 + 1, h)
            cr.fill()
            cr.rectangle(x * w, 0, w / 12 + 1, h)
            cr.fill()

    area.set_draw_func(draw)
    area.add_tick_callback(lambda a, _clock: (a.queue_draw(), True)[1])
    win.set_child(area)
    win.present()
    win.maximize()


app = Gtk.Application(application_id="org.tessera.redraw")
app.connect("activate", activate)
app.run(None)
