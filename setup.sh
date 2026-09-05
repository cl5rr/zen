#!/usr/bin/env bash
#
# ZEN setup - checks what you already have, installs only what's missing,
# builds ZEN, and optionally installs it.
#
#   ./setup.sh --check          report what's present and what's missing
#   ./setup.sh                  install missing deps, then build
#   ./setup.sh --install        ... and install ZEN system-wide
#   ./setup.sh --help           full option list
#
# System library lists are kept in sync with .github/workflows/ci.yml.

set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
ASSUME_YES=0
DO_DEPS=1
DO_BUILD=1
DO_INSTALL=0
CHECK_ONLY=0
WITH_VISUAL_TESTS=0
BUILD_PROFILE=release
CARGO_FEATURES=""

# ---------------------------------------------------------------- output ----

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'; C_DIM=$'\033[2m'
    C_RED=$'\033[31m'; C_GREEN=$'\033[32m'; C_YELLOW=$'\033[33m'; C_BLUE=$'\033[34m'
else
    C_RESET=""; C_BOLD=""; C_DIM=""; C_RED=""; C_GREEN=""; C_YELLOW=""; C_BLUE=""
fi

step() { printf '\n%s==>%s %s%s%s\n' "$C_BLUE" "$C_RESET" "$C_BOLD" "$*" "$C_RESET"; }
info() { printf '    %s\n' "$*"; }
dim()  { printf '    %s%s%s\n' "$C_DIM" "$*" "$C_RESET"; }
warn() { printf '%swarning:%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
ok()   { printf '  %sok%s   %s\n' "$C_GREEN" "$C_RESET" "$*"; }
die()  { printf '\n%serror:%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; exit 1; }

# Two-column status line: "  ok   name    detail"
row_ok()      { printf '      %sok  %s %-18s %s%s%s\n' "$C_GREEN" "$C_RESET" "$1" "$C_DIM" "${2:-}" "$C_RESET"; }
row_miss()    { printf '      %smiss%s %-18s %s\n' "$C_YELLOW" "$C_RESET" "$1" "${2:-}"; }
row_unknown() { printf '      %s?   %s %-18s %s%s%s\n' "$C_DIM" "$C_RESET" "$1" "$C_DIM" "${2:-}" "$C_RESET"; }

# ---------------------------------------------------------------- banner ----
#
# Stored with literal backslash-033 sequences and printed with printf's %b, so this file
# stays plain ASCII instead of carrying raw control bytes. Falls back to the
# uncoloured art whenever colour is unavailable (NO_COLOR, or not a terminal).

BANNER_COLOR=$(cat <<'BANNER_EOF'
\033[0;97;40m█\033[0;97;47m▀▀▀▀▀▀▀▀▀▀▄▄\033[0;90;47m▀\033[0;37;40m▄\033[0;90;40m▄\033[0;37;40m        \033[0;97;40m▄\033[0;97;47m▀\033[0;97;40m▀▀▀▀▀\033[0;97;47m▄▄\033[0;90;47m▀\033[0;37;40m▄\033[0;90;40m▄\033[0;37;40m \033[0;97;40m█\033[0;97;47m▀▀▀▀▀▀▀▀▀▀▄▄\033[0;90;47m▀\033[0;37;40m▄\033[0;90;40m▄\033[0;37;40m    \033[0m
\033[0;97;40m█\033[0;37;40m█      \033[0;96;40m■\033[0;37;40m \033[0;96;40m▄▄\033[0;37;40m \033[0;97;40m▀\033[0;97;47m▄\033[0;37;40m█\033[0;90;47m▀\033[0;90;40m▄\033[0;37;40m    \033[0;97;40m▄\033[0;97;47m▀\033[0;37;40m▀        \033[0;97;40m█\033[0;37;40m█\033[0;90;47m▀\033[0;90;40m▄\033[0;97;40m█\033[0;37;40m█      \033[0;96;40m■\033[0;37;40m \033[0;96;40m▄▄\033[0;37;40m \033[0;97;40m▀\033[0;97;47m▄\033[0;37;40m█\033[0;90;47m▀\033[0;90;40m▄\033[0;37;40m  \033[0m
\033[0;97;40m▀\033[0;97;47m▄\033[0;37;40m▄         \033[0;96;40m▀▄\033[0;37;40m \033[0;97;40m█\033[0;37;40m█\033[0;90;47m▀\033[0;90;40m▄\033[0;37;40m  \033[0;97;40m█\033[0;37;40m█  \033[0;96;40m▄■·\033[0;37;40m     \033[0;97;40m█\033[0;97;47m \033[0;37;40m█\033[0;90;40m█\033[0;97;40m█\033[0;37;40m█ \033[0;34;40m░\033[0;37;40m        \033[0;96;40m▀▄\033[0;37;40m \033[0;97;40m█\033[0;37;40m█\033[0;90;47m▀\033[0;90;40m▄\033[0;37;40m \033[0m
\033[0;37;40m  \033[0;97;40m▀\033[0;97;47m▄\033[0;97;40m▄\033[0;37;40m▄ \033[0;97;40m█\033[0;97;47m▀▄\033[0;37;40m▄   \033[0;96;40m▌\033[0;97;40m▐\033[0;97;47m▌\033[0;37;40m█\033[0;90;47m▐\033[0;90;40m▌\033[0;97;40m▐\033[0;97;47m▌\033[0;37;40m▌\033[0;96;40m▄▀\033[0;37;40m  \033[0;97;40m▄\033[0;97;47m▀▀\033[0;97;40m▄▄\033[0;97;47m▀\033[0;37;40m█\033[0;90;47m▄\033[0;90;40m▀\033[0;37;40m \033[0;97;40m█\033[0;37;40m█\033[0;34;40m▒▒░░\033[0;37;40m \033[0;97;40m▄\033[0;97;47m▀\033[0;97;40m▄\033[0;37;40m▄ \033[0;34;40m░\033[0;37;40m \033[0;96;40m▌\033[0;97;40m▐\033[0;97;47m▌\033[0;37;40m█\033[0;90;47m▐\033[0;90;40m▌\033[0m
\033[0;37;40m     \033[0;97;40m▀▀▀\033[0;97;47m▄\033[0;97;40m▄\033[0;97;47m▀\033[0;37;40m   \033[0;97;40m▄█\033[0;37;40m██\033[0;90;40m█\033[0;37;40m \033[0;97;40m█\033[0;37;40m█ \033[0;96;40m▌\033[0;37;40m  \033[0;97;40m█\033[0;97;47m▄▄\033[0;97;40m▄▄\033[0;37;40m▄\033[0;90;40m▄\033[0;37;40m    \033[0;97;40m█\033[0;37;40m█\033[0;34;40m▓▓▓▓\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;97;40m█\033[0;34;40m▒\033[0;94;44m░\033[0;34;40m▓▒░\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0m
\033[0;37;40m  \033[0;97;40m▄▄\033[0;97;47m▀\033[0;97;40m▀▀▀\033[0;37;40m▀  \033[0;97;40m▄▄█\033[0;97;47m▀\033[0;37;40m█\033[0;90;47m▄\033[0;37;40m   \033[0;97;40m█\033[0;37;40m█   \033[0;94;40m▄▄■\033[0;37;40m  \033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;37;40m   \033[0;97;40m█\033[0;37;40m█\033[0;34;40m█\033[0;94;44m░░\033[0;34;40m█\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;97;40m█\033[0;94;44m░▒░\033[0;34;40m█▓\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0m
\033[0;37;40m \033[0;97;40m▐\033[0;97;47m▌\033[0;37;40m▌ \033[0;94;40m▄▌\033[0;97;40m█\033[0;97;47m▀\033[0;97;40m▀▀▄▄▄▄▄\033[0;37;40m▄\033[0;90;40m▄\033[0;37;40m  \033[0;97;40m█\033[0;37;40m█ \033[0;94;40m▄██\033[0;97;40m▐█\033[0;97;47m▀\033[0;97;40m▀▀\033[0;37;40m▀\033[0;90;40m▀\033[0;37;40m    \033[0;97;40m█\033[0;37;40m█\033[0;94;44m░░▒▓\033[0;97;40m█\033[0;37;40m█\033[0;97;47m \033[0;90;40m█\033[0;97;40m█\033[0;94;44m░\033[0;94;40m██\033[0;34;40m██\033[0;97;40m█\033[0;97;47m \033[0;37;40m█\033[0;90;40m█\033[0m
\033[0;37;40m \033[0;97;40m▐\033[0;97;47m▌\033[0;37;40m▌\033[0;34;40m█\033[0;94;44m▓▒\033[0;97;40m█\033[0;97;47m▄▀\033[0;37;40m▀ \033[0;94;40m█▓\033[0;94;44m▒\033[0;97;40m█\033[0;37;40m█\033[0;90;47m▐\033[0;90;40m▌\033[0;37;40m \033[0;97;40m█\033[0;37;40m█ \033[0;94;40m░\033[0;94;44m▒▓\033[0;34;40m▌\033[0;97;40m▀\033[0;97;47m▄▄\033[0;97;40m▀▀\033[0;97;47m▄\033[0;37;40m█\033[0;90;47m▀\033[0;90;40m▄\033[0;37;40m \033[0;97;41m█\033[0;37;40m█\033[0;94;44m░░▒▓\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;97;40m█\033[0;94;44m▒\033[0;94;40m█▓\033[0;94;44m▒\033[0;34;40m█\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0m
\033[0;37;40m \033[0;97;40m█\033[0;37;40m█ \033[0;94;44m▓▒░\033[0;34;40m▄▄\033[0;37;40m \033[0;34;40m▓\033[0;94;44m░▒░\033[0;37;40m \033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;37;40m \033[0;97;40m▐\033[0;97;47m▌\033[0;37;40m▌ \033[0;94;40m░▒\033[0;94;44m▒░\033[0;34;40m█▓▒░\033[0;97;40m▀\033[0;97;47m▄\033[0;37;40m█\033[0;90;47m▀\033[0;90;40m▄\033[0;97;41m█\033[0;37;40m█\033[0;94;44m░▒▓▒\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;97;40m█\033[0;94;44m▓█▓▒░\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0m
\033[0;97;40m▄\033[0;97;47m▀\033[0;37;40m▀\033[0;34;40m▄\033[0;94;44m░\033[0;34;40m█▓▒░\033[0;37;40m \033[0;34;40m▒░▓▀\033[0;97;40m▄\033[0;97;47m▀ \033[0;90;47m▄\033[0;90;40m▀\033[0;37;40m  \033[0;97;40m▀\033[0;97;47m▄\033[0;37;40m▄ \033[0;34;40m▀\033[0;94;44m░\033[0;34;40m██▓▒░\033[0;37;40m \033[0;97;40m█\033[0;97;47m  \033[0;90;40m█\033[0;97;41m█\033[0;37;40m█\033[0;94;44m░▒▓▓\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;97;40m█\033[0;94;44m░▒░\033[0;34;40m█░\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0m
\033[0;97;40m█\033[0;97;47m▄\033[0;97;40m▄▄▄▄▄▄▄▄▄▄\033[0;97;47m▀▀\033[0;90;47m▄\033[0;37;40m▀\033[0;90;40m▀\033[0;37;40m      \033[0;97;40m▀▀\033[0;97;47m▄\033[0;97;40m▄▄▄▄▄\033[0;97;47m▀▀\033[0;90;47m▄\033[0;37;40m▀\033[0;90;40m▀\033[0;37;40m \033[0;97;41m█\033[0;97;47m▄\033[0;97;44m▄▄▄▄\033[0;97;40m█\033[0;37;40m██\033[0;90;40m█\033[0;97;40m█\033[0;97;44m▄▄▄▄\033[0;97;40m▄█\033[0;37;40m██\033[0;90;40m█\033[0m
BANNER_EOF
)

BANNER_PLAIN=$(cat <<'BANNER_EOF'
█▀▀▀▀▀▀▀▀▀▀▄▄▀▄▄        ▄▀▀▀▀▀▀▄▄▀▄▄ █▀▀▀▀▀▀▀▀▀▀▄▄▀▄▄
██      ■ ▄▄ ▀▄█▀▄    ▄▀▀        ██▀▄██      ■ ▄▄ ▀▄█▀▄
▀▄▄         ▀▄ ██▀▄  ██  ▄■·     █ ████ ░        ▀▄ ██▀▄
  ▀▄▄▄ █▀▄▄   ▌▐▌█▐▌▐▌▌▄▀  ▄▀▀▄▄▀█▄▀ ██▒▒░░ ▄▀▄▄ ░ ▌▐▌█▐▌
     ▀▀▀▄▄▀   ▄████ ██ ▌  █▄▄▄▄▄▄    ██▓▓▓▓█████▒░▓▒░████
  ▄▄▀▀▀▀▀  ▄▄█▀█▄   ██   ▄▄■  ████   ███░░██████░▒░█▓████
 ▐▌▌ ▄▌█▀▀▀▄▄▄▄▄▄▄  ██ ▄██▐█▀▀▀▀▀    ██░░▒▓██ ██░█████ ██
 ▐▌▌█▓▒█▄▀▀ █▓▒██▐▌ ██ ░▒▓▌▀▄▄▀▀▄█▀▄ ██░░▒▓█████▒█▓▒█████
 ██ ▓▒░▄▄ ▓░▒░ ████ ▐▌▌ ░▒▒░█▓▒░▀▄█▀▄██░▒▓▒█████▓█▓▒░████
▄▀▀▄░█▓▒░ ▒░▓▀▄▀ ▄▀  ▀▄▄ ▀░██▓▒░ █  ███░▒▓▓█████░▒░█░████
█▄▄▄▄▄▄▄▄▄▄▄▀▀▄▀▀      ▀▀▄▄▄▄▄▄▀▀▄▀▀ █▄▄▄▄▄█████▄▄▄▄▄████
BANNER_EOF
)

banner() {
    if [ -n "$C_RESET" ]; then
        # The art is an argument, not the format string, so a stray percent sign
        # in it could never be read as a conversion.
        printf '%b
' "$BANNER_COLOR"
    else
        printf '%s
' "$BANNER_PLAIN"
    fi
}

usage() {
    cat <<'EOF'
ZEN setup

Usage: ./setup.sh [options]

  --check            Report what is installed and what is missing, then exit.
                     Changes nothing. Safe to run first.
  --deps-only        Install missing dependencies and exit.
  --build-only       Skip dependency handling; just build.
  --install          Install ZEN after building (needs root for PREFIX).
  --debug            Build the debug profile instead of release.
  --with-visual-tests
                     Also require GTK4/libadwaita, used by zen-visual-tests
                     (the shader iteration harness).
  --features LIST    Extra cargo features.
  --no-default-features
                     Build without dbus/systemd/screencast. Leaner, and handy
                     for nested development.
  --prefix DIR       Install prefix (default: /usr/local, or $PREFIX).
  -y, --yes          Do not prompt; pass the package manager its yes flag.
  -h, --help         This message.

Environment:
  PREFIX             Same as --prefix.
  ZEN_SKIP_RUST      Set to 1 to skip the Rust toolchain check.

Examples:
  ./setup.sh --check                   # see what a clean system is missing
  ./setup.sh -y                        # install what's missing, then build
  ./setup.sh --no-default-features     # lean build for nested development
  sudo ./setup.sh --install            # build and install system-wide
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --check)                CHECK_ONLY=1 ;;
        --deps-only)            DO_BUILD=0; DO_INSTALL=0 ;;
        --build-only)           DO_DEPS=0 ;;
        --install)              DO_INSTALL=1 ;;
        --debug)                BUILD_PROFILE=debug ;;
        --with-visual-tests)    WITH_VISUAL_TESTS=1 ;;
        --features)             [ $# -ge 2 ] || die "--features needs an argument"
                                CARGO_FEATURES="$CARGO_FEATURES --features $2"; shift ;;
        --no-default-features)  CARGO_FEATURES="$CARGO_FEATURES --no-default-features" ;;
        --prefix)               [ $# -ge 2 ] || die "--prefix needs an argument"
                                PREFIX="$2"; shift ;;
        --prefix=*)             PREFIX="${1#*=}" ;;
        -y|--yes)               ASSUME_YES=1 ;;
        -h|--help)              usage; exit 0 ;;
        *)                      die "unknown option: $1 (try --help)" ;;
    esac
    shift
done

have() { command -v "$1" >/dev/null 2>&1; }

confirm() {
    [ "$ASSUME_YES" = 1 ] && return 0
    [ -t 0 ] || return 0
    printf '    %scontinue? [Y/n]%s ' "$C_BOLD" "$C_RESET"
    local reply=""
    read -r reply </dev/tty || return 1
    case "$reply" in [nN]*) return 1 ;; *) return 0 ;; esac
}

# ------------------------------------------------------------------ root ----

SUDO=""
# Resolve how to escalate, and verify it will actually work *before* running a
# long command - an un-authenticated sudo blocks on /dev/tty forever, which is
# a miserable way to discover the problem.
need_root() {
    if [ "$(id -u)" = 0 ]; then
        SUDO=""
        return 0
    fi
    if have sudo; then
        SUDO="sudo"
    elif have doas; then
        SUDO="doas"
    else
        die "this step needs root, but neither sudo nor doas is installed.

    On a minimal Arch install sudo is not present by default. Either:
      • re-run as root:            su -c './setup.sh'
      • or install sudo first:     pacman -S sudo   (as root)
      • or install the packages yourself and re-run with --build-only
        (./setup.sh --check will list exactly what is needed)"
    fi

    if ! $SUDO -n true 2>/dev/null; then
        if [ ! -t 0 ]; then
            die "$SUDO needs a password but there is no terminal to ask on.
    Re-run this script directly in a terminal, or as root."
        fi
        info "$SUDO needs your password:"
        $SUDO -v || die "could not obtain root via $SUDO"
    fi
}

# -------------------------------------------------------------- detection ---

DISTRO_NAME=""; PKG_MGR=""

detect_distro() {
    if [ -r /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        DISTRO_NAME="${PRETTY_NAME:-${ID:-unknown}}"
        local id="${ID:-}" like="${ID_LIKE:-}"
        case " $id $like " in
            *" arch "*|*" archarm "*|*" manjaro "*|*" endeavouros "*) PKG_MGR=pacman ;;
            *" debian "*|*" ubuntu "*)                                PKG_MGR=apt ;;
            *" fedora "*|*" rhel "*|*" centos "*)                     PKG_MGR=dnf ;;
            *" alpine "*)                                             PKG_MGR=apk ;;
            *" suse "*|*" opensuse "*)                                PKG_MGR=zypper ;;
        esac
    else
        DISTRO_NAME="unknown"
    fi
    if [ -z "$PKG_MGR" ]; then
        for m in pacman apt-get dnf apk zypper; do
            if have "$m"; then PKG_MGR="${m%-get}"; break; fi
        done
    fi
    [ -n "$PKG_MGR" ] || PKG_MGR=unknown
}

# ------------------------------------------------------- dependency model ---
#
# Each system library is "pkgconfig-name|human description". The package that
# provides it differs per distro, so provider lookup is a separate table.

LIBS="\
wayland-server|Wayland compositor core
wayland-client|Wayland client side (Xwayland, nested backend)
libinput|input devices: keyboard, mouse, touchpad
libudev|device discovery and hotplug
xkbcommon|keyboard layouts and keymaps
gbm|GPU buffer management (DRM/KMS backend)
egl|OpenGL ES context creation
libseat|seat and session management (seatd/logind)
libdisplay-info|EDID parsing for monitor identification
pangocairo|text rendering for on-screen UI
dbus-1|desktop integration"

# Provider package for a given pkg-config name, per package manager.
provider_for() {
    local mgr="$1" lib="$2"
    case "$mgr" in
    pacman) case "$lib" in
        wayland-server|wayland-client) echo wayland ;;
        libinput) echo libinput ;; libudev) echo systemd-libs ;;
        xkbcommon) echo libxkbcommon ;; gbm|egl) echo mesa ;;
        libseat) echo seatd ;; libdisplay-info) echo libdisplay-info ;;
        pangocairo) echo pango ;; dbus-1) echo dbus ;;
        esac ;;
    apt) case "$lib" in
        wayland-server|wayland-client) echo libwayland-dev ;;
        libinput) echo libinput-dev ;; libudev) echo libudev-dev ;;
        xkbcommon) echo libxkbcommon-dev ;; gbm) echo libgbm-dev ;;
        egl) echo libegl1-mesa-dev ;; libseat) echo libseat-dev ;;
        libdisplay-info) echo libdisplay-info-dev ;;
        pangocairo) echo libpango1.0-dev ;; dbus-1) echo libdbus-1-dev ;;
        esac ;;
    dnf) case "$lib" in
        wayland-server|wayland-client) echo wayland-devel ;;
        libinput) echo libinput-devel ;; libudev) echo systemd-devel ;;
        xkbcommon) echo libxkbcommon-devel ;; gbm) echo libgbm-devel ;;
        egl) echo mesa-libEGL-devel ;; libseat) echo libseat-devel ;;
        libdisplay-info) echo libdisplay-info-devel ;;
        pangocairo) echo pango-devel ;; dbus-1) echo dbus-devel ;;
        esac ;;
    apk) case "$lib" in
        wayland-server|wayland-client) echo wayland-dev ;;
        libinput) echo libinput-dev ;; libudev) echo eudev-dev ;;
        xkbcommon) echo libxkbcommon-dev ;; gbm|egl) echo mesa-dev ;;
        libseat) echo libseat-dev ;; libdisplay-info) echo libdisplay-info-dev ;;
        pangocairo) echo pango-dev ;; dbus-1) echo dbus-dev ;;
        esac ;;
    zypper) case "$lib" in
        wayland-server|wayland-client) echo wayland-devel ;;
        libinput) echo libinput-devel ;; libudev) echo systemd-devel ;;
        xkbcommon) echo libxkbcommon-devel ;; gbm) echo libgbm-devel ;;
        egl) echo Mesa-libEGL-devel ;; libseat) echo libseat-devel ;;
        libdisplay-info) echo libdisplay-info-devel ;;
        pangocairo) echo pango-devel ;; dbus-1) echo dbus-1-devel ;;
        esac ;;
    esac
}

# Package providing a build tool.
tool_package_for() {
    local mgr="$1" tool="$2"
    case "$mgr:$tool" in
        pacman:cc)  echo base-devel ;;  pacman:pkg-config) echo pkgconf ;;
        pacman:clang) echo clang ;;
        apt:cc)     echo gcc ;;         apt:pkg-config)    echo pkg-config ;;
        apt:clang)  echo clang ;;
        dnf:cc)     echo gcc ;;         dnf:pkg-config)    echo pkgconf-pkg-config ;;
        dnf:clang)  echo clang ;;
        apk:cc)     echo build-base ;;  apk:pkg-config)    echo pkgconf ;;
        apk:clang)  echo clang-libclang ;;
        zypper:cc)  echo gcc ;;         zypper:pkg-config) echo pkg-config ;;
        zypper:clang) echo clang ;;
    esac
}

optional_packages_for() {
    case "$1" in
        pacman) echo "pipewire" ;;  apt) echo "libpipewire-0.3-dev" ;;
        dnf)    echo "pipewire-devel" ;; apk) echo "pipewire-dev" ;;
        zypper) echo "pipewire-devel" ;;
    esac
}

visual_test_packages_for() {
    case "$1" in
        pacman) echo "gtk4 libadwaita" ;; apt) echo "libgtk-4-dev libadwaita-1-dev" ;;
        dnf)    echo "gtk4-devel libadwaita-devel" ;; apk) echo "gtk4.0-dev libadwaita-dev" ;;
        zypper) echo "gtk4-devel libadwaita-devel" ;;
    esac
}

# ------------------------------------------------------------- the check ----

MISSING_PKGS=""
N_OK=0
N_MISSING=0
N_UNKNOWN=0

add_missing() {
    local pkg="$1"
    [ -n "$pkg" ] || return 0
    case " $MISSING_PKGS " in *" $pkg "*) return 0 ;; esac
    MISSING_PKGS="$MISSING_PKGS $pkg"
}

check_deps() {
    step "Checking what you have"
    detect_distro
    info "$DISTRO_NAME"
    if [ "$PKG_MGR" = unknown ]; then
        info "package manager: ${C_YELLOW}not recognized${C_RESET}"
    else
        info "package manager: $PKG_MGR"
    fi

    printf '\n    %sbuild tools%s\n' "$C_BOLD" "$C_RESET"

    if have cc || have gcc || have clang; then
        local ccname; ccname="$( (have cc && cc --version) || (have gcc && gcc --version) \
            || clang --version 2>/dev/null )"
        row_ok "C compiler" "$(printf '%s' "$ccname" | head -1)"
        N_OK=$((N_OK + 1))
    else
        row_miss "C compiler" "required to build native dependencies"
        add_missing "$(tool_package_for "$PKG_MGR" cc)"
        N_MISSING=$((N_MISSING + 1))
    fi

    if have pkg-config || have pkgconf; then
        row_ok "pkg-config" "$(pkg-config --version 2>/dev/null || pkgconf --version 2>/dev/null)"
        N_OK=$((N_OK + 1))
    else
        row_miss "pkg-config" "required to locate system libraries"
        add_missing "$(tool_package_for "$PKG_MGR" pkg-config)"
        N_MISSING=$((N_MISSING + 1))
    fi

    if have clang; then
        row_ok "clang" "$(clang --version 2>/dev/null | head -1)"
        N_OK=$((N_OK + 1))
    else
        row_miss "clang" "required by bindgen for libdisplay-info"
        add_missing "$(tool_package_for "$PKG_MGR" clang)"
        N_MISSING=$((N_MISSING + 1))
    fi

    # Rust is reported here but installed separately, via rustup.
    local PATH_SAVE="$PATH"
    [ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"
    if have cargo; then
        row_ok "Rust" "$(cargo --version 2>/dev/null)"
        N_OK=$((N_OK + 1))
    else
        row_miss "Rust" "needed >= 1.87; installed via rustup, no root required"
        N_MISSING=$((N_MISSING + 1))
    fi
    PATH="$PATH_SAVE"

    printf '\n    %ssystem libraries%s\n' "$C_BOLD" "$C_RESET"

    if ! have pkg-config && ! have pkgconf; then
        dim "pkg-config is not installed yet, so these cannot be probed individually."
        dim "Listing all of them; your package manager will skip any already present."
        local lib desc
        while IFS='|' read -r lib desc; do
            [ -n "$lib" ] || continue
            row_unknown "$lib" "$desc"
            add_missing "$(provider_for "$PKG_MGR" "$lib")"
            N_UNKNOWN=$((N_UNKNOWN + 1))
        done <<EOF
$LIBS
EOF
    else
        local lib desc ver pkg
        while IFS='|' read -r lib desc; do
            [ -n "$lib" ] || continue
            if pkg-config --exists "$lib" 2>/dev/null; then
                ver="$(pkg-config --modversion "$lib" 2>/dev/null || true)"
                row_ok "$lib" "$ver"
                N_OK=$((N_OK + 1))
            else
                row_miss "$lib" "$desc"
                pkg="$(provider_for "$PKG_MGR" "$lib")"
                add_missing "$pkg"
                N_MISSING=$((N_MISSING + 1))
            fi
        done <<EOF
$LIBS
EOF
    fi

    # Optional extras.
    if [ "$WITH_VISUAL_TESTS" = 1 ]; then
        printf '\n    %svisual tests (GTK4)%s\n' "$C_BOLD" "$C_RESET"
        if pkg-config --exists gtk4 2>/dev/null && pkg-config --exists libadwaita-1 2>/dev/null; then
            row_ok "gtk4 + libadwaita" "$(pkg-config --modversion gtk4 2>/dev/null)"
            N_OK=$((N_OK + 1))
        else
            row_miss "gtk4 + libadwaita" "shader iteration harness"
            for pkg in $(visual_test_packages_for "$PKG_MGR"); do add_missing "$pkg"; done
            N_MISSING=$((N_MISSING + 1))
        fi
    fi

    # PipeWire is only needed for the default screencast feature.
    case "$CARGO_FEATURES" in
        *--no-default-features*) : ;;
        *)
            if ! pkg-config --exists libpipewire-0.3 2>/dev/null; then
                for pkg in $(optional_packages_for "$PKG_MGR"); do add_missing "$pkg"; done
            fi
            ;;
    esac

    printf '\n'
    if [ "$N_MISSING" -eq 0 ] && [ "$N_UNKNOWN" -eq 0 ]; then
        printf '    %sall %s checks passed - nothing to install.%s\n' "$C_GREEN" "$N_OK" "$C_RESET"
    elif [ "$N_UNKNOWN" -gt 0 ]; then
        printf '    %s%s present, %s missing, %s unverifiable until pkg-config is installed.%s\n' \
            "$C_BOLD" "$N_OK" "$N_MISSING" "$N_UNKNOWN" "$C_RESET"
    else
        printf '    %s%s present, %s missing.%s\n' "$C_BOLD" "$N_OK" "$N_MISSING" "$C_RESET"
    fi
}

install_deps() {
    MISSING_PKGS="${MISSING_PKGS# }"

    if [ -z "$MISSING_PKGS" ]; then
        step "Dependencies"
        ok "everything already present"
        return 0
    fi

    if [ "$PKG_MGR" = unknown ]; then
        warn "unrecognized distribution - install these yourself, then re-run with --build-only:"
        printf '\n'
        printf '%s\n' "$LIBS" | while IFS='|' read -r lib desc; do
            [ -n "$lib" ] && printf '      %-18s %s\n' "$lib" "$desc"
        done
        printf '\n    plus a C compiler, clang, pkg-config, and Rust >= 1.87.\n'
        exit 1
    fi

    step "Installing missing packages"
    # shellcheck disable=SC2086
    set -- $MISSING_PKGS
    info "$# package(s) via $PKG_MGR:"
    dim "$*"
    confirm || die "aborted"

    need_root
    local y=""; [ "$ASSUME_YES" = 1 ] && y=1

    case "$PKG_MGR" in
        pacman) $SUDO pacman -S --needed ${y:+--noconfirm} "$@" ;;
        apt)    $SUDO apt-get update && $SUDO apt-get install ${y:+-y} "$@" ;;
        dnf)    $SUDO dnf install ${y:+-y} "$@" ;;
        apk)    $SUDO apk add "$@" ;;
        zypper) $SUDO zypper install ${y:+-y} "$@" ;;
    esac
    ok "packages installed"
}

# ------------------------------------------------------------------ rust ----

ensure_rust() {
    [ "${ZEN_SKIP_RUST:-0}" = 1 ] && return 0
    [ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"
    have cargo && return 0

    step "Installing Rust"
    info "ZEN needs Rust >= 1.87. rustup installs it into ~/.cargo - no root needed."
    confirm || die "cannot build without a Rust toolchain"

    have curl || die "curl is required to install rustup (install curl, or install Rust yourself)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable
    PATH="$HOME/.cargo/bin:$PATH"
    have cargo || die "rustup finished but cargo is not on PATH; open a new shell and re-run"
    ok "$(cargo --version)"
}

# ----------------------------------------------------------------- build ----

build() {
    step "Building ZEN ($BUILD_PROFILE)"
    [ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"
    dim "this takes a few minutes on a first build"

    local args="build"
    [ "$BUILD_PROFILE" = release ] && args="$args --release"
    # shellcheck disable=SC2086
    cargo $args $CARGO_FEATURES

    local bin="target/$BUILD_PROFILE/zen"
    [ -x "$bin" ] || die "build reported success but $bin is missing"
    ok "$bin"
}

# --------------------------------------------------------------- install ----

install_zen() {
    step "Installing to $PREFIX"
    SUDO=""
    [ -w "$PREFIX" ] || need_root

    local bin="target/$BUILD_PROFILE/zen"
    [ -x "$bin" ] || die "$bin not found; build first"

    $SUDO install -Dm755 "$bin"                     "$PREFIX/bin/zen"
    $SUDO install -Dm755 resources/zen-session      "$PREFIX/bin/zen-session"
    $SUDO install -Dm644 resources/zen.desktop      "$PREFIX/share/wayland-sessions/zen.desktop"
    $SUDO install -Dm644 resources/zen-portals.conf "$PREFIX/share/xdg-desktop-portal/zen-portals.conf"
    $SUDO install -Dm644 resources/zen.png          "$PREFIX/share/pixmaps/zen.png"
    $SUDO install -Dm644 resources/zen.png          "$PREFIX/share/icons/hicolor/512x512/apps/zen.png"

    if have systemctl; then
        $SUDO install -Dm644 resources/zen.service         "$PREFIX/lib/systemd/user/zen.service"
        $SUDO install -Dm644 resources/zen-shutdown.target "$PREFIX/lib/systemd/user/zen-shutdown.target"
        dim "systemd user units installed"
    fi

    have gtk-update-icon-cache && \
        $SUDO gtk-update-icon-cache -qtf "$PREFIX/share/icons/hicolor" 2>/dev/null || true

    ok "installed to $PREFIX"
    printf '\n'
    info "Next:"
    info "  • log out and pick ${C_BOLD}ZEN${C_RESET} from your display manager, or"
    info "  • run ${C_BOLD}zen${C_RESET} from a TTY, or"
    info "  • run ${C_BOLD}zen${C_RESET} inside an existing Wayland session (nested, for development)"
}

# ------------------------------------------------------------------ main ----

main() {
    { [ -f Cargo.toml ] && grep -q '^name = "zen"' Cargo.toml; } \
        || die "run this from the root of the ZEN repository"

    banner
    printf '\n%sZEN setup%s\n' "$C_BOLD$C_BLUE" "$C_RESET"

    if [ "$CHECK_ONLY" = 1 ]; then
        check_deps
        printf '\n'
        MISSING_PKGS="${MISSING_PKGS# }"
        if [ -n "$MISSING_PKGS" ]; then
            dim "would install: $MISSING_PKGS"
            info "run ${C_BOLD}./setup.sh${C_RESET} to install these and build"
        else
            info "run ${C_BOLD}./setup.sh --build-only${C_RESET} to build"
        fi
        exit 0
    fi

    if [ "$DO_DEPS" = 1 ]; then check_deps; install_deps; fi
    if [ "$DO_BUILD" = 1 ]; then ensure_rust; build; fi
    if [ "$DO_INSTALL" = 1 ]; then install_zen; fi

    printf '\n%sdone%s\n' "$C_GREEN$C_BOLD" "$C_RESET"
    if [ "$DO_BUILD" = 1 ] && [ "$DO_INSTALL" = 0 ]; then
        dim "binary at target/$BUILD_PROFILE/zen - ./setup.sh --install to install it"
    fi
}

main "$@"
