# Packages iRice needs, shared by install.sh and tools/setup-tty.sh (sourced).
# shellcheck disable=SC2034
PACKAGES=(
    # Compositor, shell, portals
    hyprland hypridle hyprpicker hyprsunset quickshell xdg-desktop-portal-hyprland xdg-desktop-portal-gtk
    # Building the glass plugin and shaders
    base-devel git pkgconf qt6-shadertools cmake ninja
    qt6-declarative qt6-svg qt6-imageformats qt6-multimedia qt6-multimedia-ffmpeg ffmpeg qt6-5compat qt6-wayland plasma-integration
    # Sound, network, Bluetooth, power, keyring, polkit
    pipewire pipewire-pulse pipewire-alsa wireplumber networkmanager bluez bluez-utils upower power-profiles-daemon
    gnome-keyring libsecret openssl polkit brightnessctl playerctl libnotify mesa mesa-utils xdg-user-dirs
    # Commands the shell runs: pactl/paplay, xdg-open, gio/gsettings/gdbus, gtk-launch, sound effects, drives, file dialogs
    libpulse xdg-utils glib2 gtk3 libcanberra udisks2 zenity inotify-tools
    # Shared Google sign-in through desktop Online Accounts (no user OAuth project)
    gnome-online-accounts gnome-control-center python-gobject
    # Android Quick Share, built in (shell/assets/scripts/quickshare): its encryption, its
    # message format, and Avahi to find phones and be found on the network (mDNS); without
    # a shared network, Bluetooth LE (bluez above), then Wi-Fi Direct through wpa_supplicant
    # (iw for its links, dnsmasq to hand the phone an address)
    python-cryptography python-protobuf avahi iw wpa_supplicant dnsmasq
    # Wi-Fi networks as QR codes: making them (Share network) and reading them in screenshots
    qrencode zbar
    # Screen tools, clipboard, input
    grim slurp wl-clipboard cliphist gpu-screen-recorder wf-recorder tesseract tesseract-data-eng wtype swappy imagemagick
    # hyprpm, for the optional hyprbars title bars (a package of its own on some
    # systems, part of hyprland on others), and what it builds plugins with
    hyprpm meson cpio
    # Spotlight, Quick Look, sharing, widgets, privacy dots, updates
    libqalculate ripgrep plocate poppler ffmpegthumbnailer file psmisc python jq curl translate-shell pacman-contrib matugen
    # Fonts
    inter-font ttf-jetbrains-mono noto-fonts noto-fonts-emoji
)
# Video decoding and recording on the GPU (VA-API) for the GPUs found: Intel's and
# NVIDIA's drivers (AMD's is in mesa). install.sh adds them; left out where a
# repository doesn't carry them
GPU_INTEL=(intel-media-driver)
GPU_NVIDIA=(libva-nvidia-driver)
APPS=(kitty nautilus gvfs firefox mpv loupe pavucontrol)     # if you have none: terminal, files (the most Finder-like), browser, player, viewer, volume mixer
# Emoji styles beyond the default Noto Color Emoji (shell/common/EmojiFont.qml). Off by
# default; Settings can also install them later, with yay.
EMOJI_AUR=(ttf-apple-emoji ttf-fluent-emoji)
# What tools/build-quickshell.sh needs, installed only when iRice builds its own
# Quickshell (yours is missing, or older than compat.json's minimum)
QUICKSHELL_BUILD=(git cmake ninja qt6-base qt6-declarative qt6-svg qt6-wayland qt6-shadertools spirv-tools
    wayland wayland-protocols cli11 cpptrace jemalloc libdrm libglvnd libpipewire libxcb mesa pam polkit glib2)
# The pointer, in both looks (GTK apps use Adwaita, which comes with gtk3)
AUR=(bibata-cursor-theme-bin)
# Icon themes, optional: iRice styles the apps' own icons without one (Glass
# clear, Material themed). name:package; Papirus is in the repositories, the
# others in the AUR. install.sh --icons=colloid,papirus… or its question
ICON_THEMES=(colloid:colloid-icon-theme-git tela-circle:tela-circle-icon-theme-git numix-circle:numix-circle-icon-theme-git
    flat-remix:flat-remix papirus:papirus-icon-theme)
ICON_THEMES_REPO=(papirus-icon-theme)
# Packages iRice used to need and no longer does. Software Update offers to
# remove the ones still installed (pacman -Rns, in its root part); install.sh ignores them.
DROPPED=(mactahoe-icon-theme-git mactahoe-gtk-theme apple_cursor apple_hyprcursor)  # (apps' own logos, Adwaita and Bibata since 3 Nov 2026)
