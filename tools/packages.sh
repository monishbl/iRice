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
    # Screen tools, clipboard, input
    grim slurp wl-clipboard cliphist gpu-screen-recorder wf-recorder tesseract tesseract-data-eng wtype swappy imagemagick
    # (hyprpm, used for the optional hyprbars title bars, ships with the hyprland package)
    # Spotlight, Quick Look, sharing, widgets, privacy dots, updates
    libqalculate ripgrep plocate poppler ffmpegthumbnailer file psmisc python jq curl translate-shell pacman-contrib matugen
    # Fonts and icons
    inter-font ttf-jetbrains-mono noto-fonts noto-fonts-emoji papirus-icon-theme
)
APPS=(kitty nautilus gvfs firefox mpv loupe pavucontrol)     # if you have none: terminal, files (the most Finder-like), browser, player, viewer, volume mixer
# Emoji styles beyond the default Noto Color Emoji (shell/common/EmojiFont.qml). Off by
# default; Settings can also install them later, with yay.
EMOJI_AUR=(ttf-apple-emoji ttf-fluent-emoji)
AUR=(mactahoe-icon-theme-git apple_cursor apple_hyprcursor mactahoe-gtk-theme bibata-cursor-theme-bin)
# Packages iRice used to need and no longer does. Software Update offers to
# remove the ones still installed (pkexec pacman -Rns); install.sh ignores them.
DROPPED=()
