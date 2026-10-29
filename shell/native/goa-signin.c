// Adds a Google account to GNOME Online Accounts straight from iRice's
// Settings, without the GNOME Settings app (which iRice doesn't need, and
// which opened on a page you then had to find Google on).
//
// It asks GNOME Online Accounts' own Google provider to add an account, as
// GNOME Settings does: GOA's dialog opens the browser at Google's sign-in,
// and when you approve, Google redirects to GOA's handler, which hands the
// result back to this process over D-Bus (it owns
// org.gnome.OnlineAccounts.OAuth2 while the dialog waits). The account is
// then GOA's like any other, with Calendar and Tasks access (GOA's Google
// scope includes both); iRice never sees a password or refresh token.
//
// The dialog has a "Sign In" button before it opens the browser; it's pressed
// for you, so connecting is one click in Settings. What's printed (one line
// of JSON) is what Google.qml reads:
//     {"account": "/org/gnome/OnlineAccounts/Accounts/…", "identity": "you@…"}
//     {"error": "…", "cancelled": true|false}
//
//     goa-signin           (ends by itself; 5 minutes at most)
#define GOA_API_IS_SUBJECT_TO_CHANGE
#define GOA_BACKEND_API_IS_SUBJECT_TO_CHANGE
#include <adwaita.h>
#include <goa/goa.h>
#include <goabackend/goabackend.h>
#include <json-glib/json-glib.h>
#include <stdio.h>

static GtkWindow* window;
static GCancellable* cancellable;
static GApplication* app;
static gboolean printed;
static guint clicker;
static int tries;

static void print_json(const char* key, const char* value, const char* key2, const char* value2, gboolean cancelled) {
    if (printed)
        return;
    printed = TRUE;
    g_autoptr(JsonBuilder) b = json_builder_new();
    json_builder_begin_object(b);
    json_builder_set_member_name(b, key);
    json_builder_add_string_value(b, value ? value : "");
    if (key2) {
        json_builder_set_member_name(b, key2);
        json_builder_add_string_value(b, value2 ? value2 : "");
    }
    if (g_strcmp0(key, "error") == 0) {
        json_builder_set_member_name(b, "cancelled");
        json_builder_add_boolean_value(b, cancelled);
    }
    json_builder_end_object(b);
    g_autoptr(JsonGenerator) gen = json_generator_new();
    g_autoptr(JsonNode) root = json_builder_get_root(b);
    json_generator_set_root(gen, root);
    g_autofree char* text = json_generator_to_data(gen, NULL);
    printf("%s\n", text);
    fflush(stdout);
}

// GOA's dialog waits for a click on "Sign In" before it opens the browser:
// find that button in the window and press it (the dialog is drawn inside
// our window, so it's in this widget tree)
static GtkWidget* find_sign_in(GtkWidget* w) {
    if (GTK_IS_BUTTON(w)) {
        const char* label = gtk_button_get_label(GTK_BUTTON(w));
        if (label && (g_strcmp0(label, "_Sign In") == 0 || g_strcmp0(label, "Sign In") == 0) && gtk_widget_is_sensitive(w)
                && gtk_widget_get_mapped(w))
            return w;
    }
    for (GtkWidget* c = gtk_widget_get_first_child(w); c; c = gtk_widget_get_next_sibling(c)) {
        GtkWidget* f = find_sign_in(c);
        if (f)
            return f;
    }
    return NULL;
}
static gboolean press_sign_in(gpointer) {
    GtkWidget* button = find_sign_in(GTK_WIDGET(window));
    if (button) {
        gtk_widget_activate(button);
        clicker = 0;
        return G_SOURCE_REMOVE;
    }
    // Not there (yet): GOA may show it a moment later; after 5 s, leave the
    // dialog as it is, for a click by hand
    if (++tries > 50) {
        clicker = 0;
        return G_SOURCE_REMOVE;
    }
    return G_SOURCE_CONTINUE;
}

static void added(GObject* source, GAsyncResult* res, gpointer) {
    g_autoptr(GError) error = NULL;
    g_autoptr(GoaObject) object = goa_provider_add_account_finish(GOA_PROVIDER(source), res, &error);
    if (object) {
        GoaAccount* account = goa_object_peek_account(object);
        print_json("account", g_dbus_object_get_object_path(G_DBUS_OBJECT(object)), "identity",
            account ? goa_account_get_presentation_identity(account) : "", FALSE);
    } else {
        gboolean cancelled = g_error_matches(error, G_IO_ERROR, G_IO_ERROR_CANCELLED)
            || g_error_matches(error, GOA_ERROR, GOA_ERROR_DIALOG_DISMISSED);
        print_json("error", cancelled ? "Sign-in was cancelled." : error->message, NULL, NULL, cancelled);
    }
    g_application_quit(app);
}

static void with_provider(GoaClient* client) {
    GoaProvider* provider = goa_provider_get_for_provider_type("google");
    if (!provider) {
        print_json("error", "This GNOME Online Accounts has no Google provider.", NULL, NULL, FALSE);
        g_application_quit(app);
        return;
    }
    goa_provider_add_account(provider, client, GTK_WIDGET(window), cancellable, added, NULL);
    clicker = g_timeout_add(100, press_sign_in, NULL);
}

static void client_ready(GObject*, GAsyncResult* res, gpointer) {
    g_autoptr(GError) error = NULL;
    GoaClient* client = goa_client_new_finish(res, &error);
    if (!client) {
        print_json("error", "GNOME Online Accounts isn't running. Install gnome-online-accounts, then try again.", NULL, NULL, FALSE);
        g_application_quit(app);
        return;
    }
    with_provider(client);
}

static gboolean give_up(gpointer) {
    g_cancellable_cancel(cancellable);
    print_json("error", "Sign-in took too long. Try again.", NULL, NULL, TRUE);
    g_application_quit(app);
    return G_SOURCE_REMOVE;
}

static gboolean closed(GtkWindow*, gpointer) {
    g_cancellable_cancel(cancellable);
    print_json("error", "Sign-in was cancelled.", NULL, NULL, TRUE);
    g_application_quit(app);
    return FALSE;
}

static void activate(GApplication* application, gpointer) {
    // A plain small window for GOA's dialog to sit in (it needs a parent)
    window = GTK_WINDOW(adw_application_window_new(GTK_APPLICATION(application)));
    gtk_window_set_title(window, "Connect Google account");
    gtk_window_set_default_size(window, 460, 520);
    adw_application_window_set_content(ADW_APPLICATION_WINDOW(window),
        GTK_WIDGET(adw_status_page_new()));
    g_signal_connect(window, "close-request", G_CALLBACK(closed), NULL);
    gtk_window_present(window);
    cancellable = g_cancellable_new();
    goa_client_new(cancellable, client_ready, NULL);
    g_timeout_add_seconds(300, give_up, NULL);
}

int main(int argc, char** argv) {
    adw_init();
    AdwApplication* a = adw_application_new("org.irice.GoogleSignIn", G_APPLICATION_NON_UNIQUE);
    app = G_APPLICATION(a);
    g_signal_connect(a, "activate", G_CALLBACK(activate), NULL);
    int status = g_application_run(app, argc, argv);
    if (!printed)
        print_json("error", "Sign-in ended without an answer. Try again.", NULL, NULL, FALSE);
    g_object_unref(a);
    return status;
}
