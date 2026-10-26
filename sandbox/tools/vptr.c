// vptr: drive a virtual pointer in the sandbox compositor (wlr-virtual-pointer),
// so tests can hover, click, drag and scroll. Coordinates are local to one
// output (default IRICE-TEST, the headless test screen).
//
//   vptr [-o OUTPUT] move X Y | click [left|right|middle] | down [btn] | up [btn]
//        | scroll DY | glide X Y MS | sleep MS   (any number of commands in a row)
//
// Built by sandbox/tools/build.sh inside the container; run via ./dev.sh pointer.
#define _POSIX_C_SOURCE 200809L
#include <linux/input-event-codes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <wayland-client.h>
#include "wlr-virtual-pointer-unstable-v1-client-protocol.h"

struct out {
    struct wl_output *wl;
    char name[64];
    int32_t w, h;
};

static struct zwlr_virtual_pointer_manager_v1 *manager;
static struct wl_seat *seat;
static struct out outs[16];
static int nouts;

static void out_geometry(void *d, struct wl_output *o, int32_t x, int32_t y, int32_t pw, int32_t ph, int32_t sp, const char *mk, const char *md, int32_t t) {}
static void out_mode(void *d, struct wl_output *o, uint32_t flags, int32_t w, int32_t h, int32_t r) {
    struct out *u = d;
    if (flags & WL_OUTPUT_MODE_CURRENT) {
        u->w = w;
        u->h = h;
    }
}
static void out_done(void *d, struct wl_output *o) {}
static void out_scale(void *d, struct wl_output *o, int32_t s) {}
static void out_name(void *d, struct wl_output *o, const char *name) {
    snprintf(((struct out *)d)->name, sizeof ((struct out *)d)->name, "%s", name);
}
static void out_desc(void *d, struct wl_output *o, const char *desc) {}
static const struct wl_output_listener out_listener = {out_geometry, out_mode, out_done, out_scale, out_name, out_desc};

static void global(void *d, struct wl_registry *r, uint32_t id, const char *iface, uint32_t ver) {
    if (!strcmp(iface, zwlr_virtual_pointer_manager_v1_interface.name))
        manager = wl_registry_bind(r, id, &zwlr_virtual_pointer_manager_v1_interface, ver < 2 ? ver : 2);
    else if (!strcmp(iface, wl_seat_interface.name) && !seat)
        seat = wl_registry_bind(r, id, &wl_seat_interface, 1);
    else if (!strcmp(iface, wl_output_interface.name) && nouts < 16 && ver >= 4) {
        struct out *u = &outs[nouts++];
        u->wl = wl_registry_bind(r, id, &wl_output_interface, 4);
        wl_output_add_listener(u->wl, &out_listener, u);
    }
}
static void global_remove(void *d, struct wl_registry *r, uint32_t id) {}
static const struct wl_registry_listener reg_listener = {global, global_remove};

static uint32_t now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1000 + t.tv_nsec / 1000000;
}
static void sleep_ms(int ms) {
    struct timespec t = {ms / 1000, (ms % 1000) * 1000000L};
    nanosleep(&t, NULL);
}
static uint32_t button(const char *s) {
    if (!s || !strcmp(s, "left")) return BTN_LEFT;
    if (!strcmp(s, "right")) return BTN_RIGHT;
    if (!strcmp(s, "middle")) return BTN_MIDDLE;
    return BTN_LEFT;
}

int main(int argc, char **argv) {
    const char *want = "IRICE-TEST";
    int i = 1;
    if (argc > 2 && !strcmp(argv[1], "-o")) {
        want = argv[2];
        i = 3;
    }
    struct wl_display *dpy = wl_display_connect(NULL);
    if (!dpy) {
        fprintf(stderr, "vptr: can't connect to the Wayland display\n");
        return 1;
    }
    struct wl_registry *reg = wl_display_get_registry(dpy);
    wl_registry_add_listener(reg, &reg_listener, NULL);
    wl_display_roundtrip(dpy);
    wl_display_roundtrip(dpy);
    if (!manager) {
        fprintf(stderr, "vptr: compositor has no wlr virtual pointer\n");
        return 1;
    }
    struct out *target = NULL;
    for (int k = 0; k < nouts; k++)
        if (!strcmp(outs[k].name, want))
            target = &outs[k];
    if (!target) {
        fprintf(stderr, "vptr: no output named %s\n", want);
        return 1;
    }
    struct zwlr_virtual_pointer_v1 *ptr = zwlr_virtual_pointer_manager_v1_create_virtual_pointer_with_output(manager, seat, target->wl);
    double cx = target->w / 2.0, cy = target->h / 2.0;

#define MOVE(x, y) do { \
        zwlr_virtual_pointer_v1_motion_absolute(ptr, now_ms(), (uint32_t)(x), (uint32_t)(y), target->w, target->h); \
        zwlr_virtual_pointer_v1_frame(ptr); wl_display_flush(dpy); } while (0)
#define BUTTON(b, st) do { \
        zwlr_virtual_pointer_v1_button(ptr, now_ms(), b, st); \
        zwlr_virtual_pointer_v1_frame(ptr); wl_display_flush(dpy); } while (0)

    for (; i < argc; i++) {
        const char *c = argv[i];
        if (!strcmp(c, "move") && i + 2 < argc) {
            cx = atof(argv[++i]);
            cy = atof(argv[++i]);
            MOVE(cx, cy);
            // Hyprland's enter event carries (-1,-1) for virtual pointers; a
            // real mouse keeps moving, so nudge once more with the true position
            sleep_ms(40);
            MOVE(cx + 1, cy);
            sleep_ms(30);
            MOVE(cx, cy);
            sleep_ms(120);
        } else if (!strcmp(c, "glide") && i + 3 < argc) {
            // Smooth move over MS milliseconds at ~120 Hz
            double x = atof(argv[++i]), y = atof(argv[++i]);
            int ms = atoi(argv[++i]), steps = ms / 8 > 0 ? ms / 8 : 1;
            for (int s = 1; s <= steps; s++) {
                MOVE(cx + (x - cx) * s / steps, cy + (y - cy) * s / steps);
                sleep_ms(8);
            }
            cx = x;
            cy = y;
        } else if (!strcmp(c, "click")) {
            const char *b = (i + 1 < argc && (!strcmp(argv[i + 1], "left") || !strcmp(argv[i + 1], "right") || !strcmp(argv[i + 1], "middle"))) ? argv[++i] : NULL;
            BUTTON(button(b), WL_POINTER_BUTTON_STATE_PRESSED);
            sleep_ms(90);
            BUTTON(button(b), WL_POINTER_BUTTON_STATE_RELEASED);
        } else if (!strcmp(c, "down") || !strcmp(c, "up")) {
            const char *b = (i + 1 < argc && (!strcmp(argv[i + 1], "left") || !strcmp(argv[i + 1], "right") || !strcmp(argv[i + 1], "middle"))) ? argv[++i] : NULL;
            BUTTON(button(b), !strcmp(c, "down") ? WL_POINTER_BUTTON_STATE_PRESSED : WL_POINTER_BUTTON_STATE_RELEASED);
        } else if (!strcmp(c, "scroll") && i + 1 < argc) {
            zwlr_virtual_pointer_v1_axis_source(ptr, WL_POINTER_AXIS_SOURCE_WHEEL);
            zwlr_virtual_pointer_v1_axis_discrete(ptr, now_ms(), WL_POINTER_AXIS_VERTICAL_SCROLL, wl_fixed_from_double(atof(argv[i + 1]) * 15), atoi(argv[i + 1]));
            zwlr_virtual_pointer_v1_frame(ptr);
            wl_display_flush(dpy);
            i++;
        } else if (!strcmp(c, "sleep") && i + 1 < argc) {
            wl_display_flush(dpy);
            sleep_ms(atoi(argv[++i]));
        } else {
            fprintf(stderr, "vptr: unknown or incomplete command '%s'\n", c);
            return 2;
        }
        sleep_ms(16);
    }
    wl_display_roundtrip(dpy);
    zwlr_virtual_pointer_v1_destroy(ptr);
    wl_display_roundtrip(dpy);
    return 0;
}
