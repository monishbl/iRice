#!/usr/bin/env python3
# A drag source for testing drops (the drop shelf): a small GTK window whose
# label can be dragged, carrying a file (fakedrag.py /path/to/file), and a
# text field next to it to select text in (which mustn't count as a drag).
import sys

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gdk, Gio, GObject, Gtk  # noqa: E402

PATH = sys.argv[1] if len(sys.argv) > 1 else "/etc/hostname"


def activate(app):
    win = Gtk.ApplicationWindow(application=app, title="fakedrag")
    win.set_default_size(420, 160)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12, margin_top=16, margin_start=16, margin_end=16)
    label = Gtk.Label(label="Drag me: " + PATH.split("/")[-1])
    label.set_size_request(380, 48)
    source = Gtk.DragSource()
    files = Gdk.FileList.new_from_list([Gio.File.new_for_path(PATH)])
    source.connect("prepare", lambda *a: Gdk.ContentProvider.new_for_value(GObject.Value(Gdk.FileList, files)))
    label.add_controller(source)
    entry = Gtk.Entry(text="Select this text back and forth, it is not a drag")
    box.append(label)
    box.append(entry)
    win.set_child(box)
    win.present()


app = Gtk.Application(application_id="dev.irice.fakedrag")
app.connect("activate", activate)
app.run([])
