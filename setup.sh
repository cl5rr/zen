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
DO_UPDATE=0
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
  --update           Pull, rebuild and reinstall. Shows what changed, and any
                     config options you have not got yet.
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

ANY_FLAG=0
while [ $# -gt 0 ]; do
    ANY_FLAG=1
    case "$1" in
        --check)                CHECK_ONLY=1 ;;
        --update)               DO_UPDATE=1; DO_BUILD=1; DO_INSTALL=1 ;;
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

# ---------------------------------------------------------------- update ----

# Reports config options that exist in the shipped default but not in yours.
#
# Your config is a *copy* taken at install time, not a live view of the default, so
# options added later never appear in it. They fall back to their defaults, which is
# harmless, but you would never learn they exist. This is a hint, not a merge: your
# file is yours, and nothing here edits it.
config_drift() {
    local user="${XDG_CONFIG_HOME:-$HOME/.config}/zen/config.kdl"
    local shipped="resources/default-config.kdl"
    [ -f "$user" ] || return 0
    [ -f "$shipped" ] || return 0

    local missing="" sec
    for sec in $(grep -oE '^[a-z][a-z-]* \{' "$shipped" | sed 's/ {$//' | sort -u); do
        grep -qE "^[[:space:]]*$sec[[:space:]]*\{" "$user" || missing="$missing $sec"
    done

    missing="${missing# }"
    [ -n "$missing" ] || return 0

    printf '\n'
    info "New config sections you do not have yet:"
    info "  ${C_BOLD}$missing${C_RESET}"
    dim "they are using their defaults; see resources/default-config.kdl"
    dim "compare with: diff $user resources/default-config.kdl"
}

# Whether a ZEN session is running right now, so we can say when the update lands.
zen_is_running() {
    pgrep -x zen >/dev/null 2>&1
}

update_zen() {
    step "Updating ZEN"

    have git   || die "git is not installed"
    [ -d .git ] || die "this is not a git checkout, so there is nothing to pull"

    if ! git diff --quiet HEAD 2>/dev/null; then
        warn "you have uncommitted changes in this checkout"
        dim "a pull may conflict; commit or stash them first if you care about them"
        confirm || return 1
    fi

    local before after
    before=$(git rev-parse HEAD)

    info "fetching"
    git pull --ff-only || die "pull failed. If it says 'diverged', your local commits and the remote have both moved; sort that out by hand."

    after=$(git rev-parse HEAD)

    if [ "$before" = "$after" ]; then
        # Deliberately still building and installing. "The pull fetched nothing" is not the
        # same as "what is installed matches this checkout", and conflating them breaks the
        # most likely path of all: you have to `git pull` by hand to get this option in the
        # first place, so the very first --update almost always finds nothing to fetch. If
        # that skipped the build, the code you just pulled would never be compiled.
        #
        # Rebuilding when nothing changed costs nothing; cargo does the work of noticing.
        ok "nothing to fetch, already at $(git rev-parse --short HEAD)"
        dim "rebuilding anyway, in case what is installed is older than this checkout"
        return 0
    fi

    printf '\n'
    info "What changed:"
    git --no-pager log --oneline --no-decorate "$before..$after" | head -20 | while read -r l; do
        dim "$l"
    done
    printf '\n'
}

update_epilogue() {
    config_drift

    printf '\n'
    if zen_is_running; then
        warn "the ZEN you are running is still the old one."
        info "Replacing the binary cannot change a process that is already running, and a"
        info "compositor cannot swap itself out without taking your windows with it."
        info "The update applies when you next start a session: log out and back in."
    else
        info "Start it with ${C_BOLD}zen${C_RESET}, or log in through your greeter."
    fi
}

# ------------------------------------------------------------------- tui ----
#
# Arrow-key menus, the way archinstall works. Deliberately hand-rolled ANSI
# rather than dialog/whiptail: this script's whole job is running on a machine
# where nothing is installed yet, so it cannot depend on a TUI toolkit being
# there. Everything below is bash builtins and escape codes.
#
# Flags still work and still win. The wizard only appears when the script is run
# with no arguments on a real terminal, so scripting and CI are unaffected.

UI_TTY=0
if [ -t 0 ] && [ -t 1 ]; then UI_TTY=1; fi

ui_cursor_hide() { [ "$UI_TTY" = 1 ] && printf '\033[?25l' || true; }
ui_cursor_show() { [ "$UI_TTY" = 1 ] && printf '\033[?25h' || true; }

# Always give the terminal its cursor back, however we leave.
trap 'ui_cursor_show' EXIT INT TERM

# Reads one keypress and echoes a name for it.
ui_key() {
    local k rest
    IFS= read -rsn1 k 2>/dev/null || { echo quit; return; }
    case "$k" in
        '')  echo enter ;;
        ' ') echo space ;;
        q|Q) echo quit ;;
        $'\033')
            if IFS= read -rsn2 -t 0.02 rest 2>/dev/null; then
                case "$rest" in
                    '[A') echo up ;;
                    '[B') echo down ;;
                    *)    echo other ;;
                esac
            else
                echo quit
            fi ;;
        *) echo other ;;
    esac
}

# ui_menu "Title" "option" ...  -> UI_CHOICE = selected index, or returns 1
UI_CHOICE=0
ui_menu() {
    local title="$1"; shift
    local n=$# sel=0 drawn=0 i opt
    local -a opts=("$@")

    ui_cursor_hide
    while :; do
        [ "$drawn" = 1 ] && printf '\033[%dA' "$((n + 3))"
        drawn=1

        printf '  %s%s%s\033[K\n\n' "$C_BOLD" "$title" "$C_RESET"
        i=0
        for opt in "${opts[@]}"; do
            if [ "$i" = "$sel" ]; then
                printf '   %s>%s %s%s%s\033[K\n' "$C_BLUE" "$C_RESET" "$C_BOLD" "$opt" "$C_RESET"
            else
                printf '     %s%s%s\033[K\n' "$C_DIM" "$opt" "$C_RESET"
            fi
            i=$((i + 1))
        done
        printf '\n   %sup/down to move, enter to select, q to quit%s\033[K\n' \
               "$C_DIM" "$C_RESET"
        printf '\033[%dA' 1

        case "$(ui_key)" in
            up)    sel=$(( (sel - 1 + n) % n )) ;;
            down)  sel=$(( (sel + 1) % n )) ;;
            enter) printf '\n'; ui_cursor_show; UI_CHOICE=$sel; return 0 ;;
            quit)  printf '\n'; ui_cursor_show; return 1 ;;
        esac
    done
}

# ui_multi "Title" "label:on" ... -> UI_PICKED = space separated indices chosen
UI_PICKED=""
ui_multi() {
    local title="$1"; shift
    local n=$# sel=0 drawn=0 i entry label mark
    local -a opts=("$@") state=()

    for entry in "${opts[@]}"; do
        case "$entry" in
            *:on)  state+=(1) ;;
            *)     state+=(0) ;;
        esac
    done

    ui_cursor_hide
    while :; do
        [ "$drawn" = 1 ] && printf '\033[%dA' "$((n + 3))"
        drawn=1

        printf '  %s%s%s\033[K\n\n' "$C_BOLD" "$title" "$C_RESET"
        i=0
        for entry in "${opts[@]}"; do
            label="${entry%:*}"
            if [ "${state[$i]}" = 1 ]; then mark="[x]"; else mark="[ ]"; fi
            if [ "$i" = "$sel" ]; then
                printf '   %s>%s %s %s%s%s\033[K\n' \
                       "$C_BLUE" "$C_RESET" "$mark" "$C_BOLD" "$label" "$C_RESET"
            else
                printf '     %s %s%s%s\033[K\n' "$mark" "$C_DIM" "$label" "$C_RESET"
            fi
            i=$((i + 1))
        done
        printf '\n   %sspace to toggle, enter to confirm, q to cancel%s\033[K\n' \
               "$C_DIM" "$C_RESET"
        printf '\033[%dA' 1

        case "$(ui_key)" in
            up)    sel=$(( (sel - 1 + n) % n )) ;;
            down)  sel=$(( (sel + 1) % n )) ;;
            space) if [ "${state[$sel]}" = 1 ]; then state[$sel]=0; else state[$sel]=1; fi ;;
            enter)
                printf '\n'; ui_cursor_show
                UI_PICKED=""
                i=0
                for entry in "${opts[@]}"; do
                    [ "${state[$i]}" = 1 ] && UI_PICKED="$UI_PICKED $i"
                    i=$((i + 1))
                done
                UI_PICKED="${UI_PICKED# }"
                return 0 ;;
            quit)  printf '\n'; ui_cursor_show; return 1 ;;
        esac
    done
}

# --------------------------------------------------------- extra install ----
#
# The steps a person otherwise has to find out about by reading a guide on their
# phone. Every one of these was manual friction the first time around.

DESKTOP_APPS="alacritty fuzzel"
EXTRA_APPS="firefox swaybg mako swaylock"
GREETER_PKGS="greetd cage greetd-regreet"

pacman_install() {
    [ "$PKG_MGR" = pacman ] || {
        warn "only pacman is supported for this step; install these yourself: $*"
        return 1
    }
    need_root
    $SUDO pacman -S --needed --noconfirm "$@"
}

install_desktop_apps() {
    step "Installing the apps ZEN's default keybinds expect"
    dim "Mod+T opens a terminal, Mod+Space opens the launcher"
    pacman_install $DESKTOP_APPS && ok "terminal and launcher installed"
}

install_extra_apps() {
    step "Installing optional extras"
    pacman_install $EXTRA_APPS && ok "extras installed"
}

write_user_config() {
    step "Writing your config"
    local dir="${XDG_CONFIG_HOME:-$HOME/.config}/zen"
    local dst="$dir/config.kdl"

    if [ -f "$dst" ]; then
        ok "config already exists at $dst, left alone"
        return 0
    fi
    mkdir -p "$dir"
    cp resources/default-config.kdl "$dst"
    ok "wrote $dst"
    dim "it is heavily commented, and reloads live while ZEN is running"
}

install_greeter() {
    step "Installing the login screen"
    dim "greetd runs the session, ReGreet draws it, cage hosts it"

    pacman_install $GREETER_PKGS || return 1

    need_root
    if [ -f /etc/greetd/config.toml ] && ! grep -q regreet /etc/greetd/config.toml 2>/dev/null; then
        $SUDO cp /etc/greetd/config.toml /etc/greetd/config.toml.bak
        dim "existing config saved as /etc/greetd/config.toml.bak"
    fi

    $SUDO mkdir -p /etc/greetd
    printf '%s\n' \
        '[terminal]' \
        'vt = 1' \
        '' \
        '[default_session]' \
        'command = "cage -s -- regreet"' \
        'user = "greeter"' \
        | $SUDO tee /etc/greetd/config.toml >/dev/null

    if [ ! -f /etc/greetd/regreet.toml ]; then
        printf '%s\n' \
            '[background]' \
            'path = "/usr/share/pixmaps/zen.png"' \
            'fit = "Cover"' \
            '' \
            '[GTK]' \
            'application_prefer_dark_theme = true' \
            'cursor_theme_name = "Adwaita"' \
            'font_name = "Cantarell 14"' \
            | $SUDO tee /etc/greetd/regreet.toml >/dev/null
    fi

    ok "greetd configured"
    printf '\n'
    warn "NOT enabling it yet, on purpose."
    info "Test ZEN from a TTY first:  ${C_BOLD}zen${C_RESET}"
    info "If that works, enable the login screen with:"
    info "  ${C_BOLD}sudo systemctl enable --now greetd${C_RESET}"
    info "If a login screen ever leaves you at a black screen, press"
    info "  ${C_BOLD}Ctrl+Alt+F2${C_RESET} and run ${C_BOLD}sudo systemctl disable --now greetd${C_RESET}"
}

# ---------------------------------------------------------------- wizard ----

wizard_summary() {
    printf '\n  %sAbout to do this:%s\n\n' "$C_BOLD" "$C_RESET"
    [ "$DO_UPDATE"   = 1 ] && info "• pull the latest ZEN and show what changed"
    [ "$DO_DEPS"     = 1 ] && info "• install build dependencies"
    [ "$W_APPS"      = 1 ] && info "• install a terminal and an app launcher"
    [ "$W_EXTRAS"    = 1 ] && info "• install optional extras ($EXTRA_APPS)"
    [ "$DO_BUILD"    = 1 ] && info "• build ZEN (this is the slow part, 5 to 15 minutes)"
    [ "$DO_INSTALL"  = 1 ] && info "• install ZEN to $PREFIX"
    [ "$W_CONFIG"    = 1 ] && info "• write your config file"
    [ "$W_GREETER"   = 1 ] && info "• install and configure the login screen"
    printf '\n'
}

W_APPS=0
W_EXTRAS=0
W_CONFIG=0
W_GREETER=0

wizard() {
    banner

    local installed=""
    have zen && installed=" (you have it already)"

    ui_menu "What would you like to do?" \
        "Install ZEN  (everything: deps, build, apps, config)" \
        "Update ZEN   (pull, rebuild, reinstall)$installed" \
        "Choose what to install" \
        "Just check what is missing, change nothing" \
        "Quit" || return 1

    case "$UI_CHOICE" in
        0)  DO_DEPS=1; DO_BUILD=1; DO_INSTALL=1
            W_APPS=1; W_CONFIG=1
            ui_menu "Also set up a graphical login screen?" \
                "Yes, install and configure greetd" \
                "No, I will start ZEN from a TTY" || return 1
            [ "$UI_CHOICE" = 0 ] && W_GREETER=1
            ;;
        1)  DO_UPDATE=1; DO_DEPS=0; DO_BUILD=1; DO_INSTALL=1 ;;
        2)  ui_multi "Pick what to do  (space toggles)" \
                "Install build dependencies:on" \
                "Build ZEN:on" \
                "Install ZEN system-wide:on" \
                "Install a terminal and launcher:on" \
                "Write my config file:on" \
                "Install the login screen:off" \
                "Install optional extras (browser, wallpaper, notifications):off" \
                || return 1
            DO_DEPS=0; DO_BUILD=0; DO_INSTALL=0
            local idx
            for idx in $UI_PICKED; do
                case "$idx" in
                    0) DO_DEPS=1 ;;
                    1) DO_BUILD=1 ;;
                    2) DO_INSTALL=1 ;;
                    3) W_APPS=1 ;;
                    4) W_CONFIG=1 ;;
                    5) W_GREETER=1 ;;
                    6) W_EXTRAS=1 ;;
                esac
            done
            ;;
        3)  CHECK_ONLY=1; return 0 ;;
        4)  return 1 ;;
    esac

    wizard_summary
    ui_menu "Go ahead?" "Yes, do it" "No, quit" || return 1
    [ "$UI_CHOICE" = 0 ] || return 1
    return 0
}

# ------------------------------------------------------------------ main ----

main() {
    { [ -f Cargo.toml ] && grep -q '^name = "zen"' Cargo.toml; } \
        || die "run this from the root of the ZEN repository"

    # A bare `./setup.sh` on a real terminal gets the guided flow. Anything with a
    # flag, or piped into a script, keeps the old non-interactive behaviour.
    if [ "$ANY_FLAG" = 0 ] && [ "$UI_TTY" = 1 ]; then
        detect_distro
        wizard || { printf '\n%snothing done%s\n' "$C_DIM" "$C_RESET"; exit 0; }
        ASSUME_YES=1
    else
        banner
        printf '\n%sZEN setup%s\n' "$C_BOLD$C_BLUE" "$C_RESET"
    fi

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

    if [ "$DO_UPDATE" = 1 ]; then update_zen || exit 1; fi
    if [ "$DO_DEPS" = 1 ]; then check_deps; install_deps; fi
    if [ "$W_APPS" = 1 ]; then install_desktop_apps; fi
    if [ "$W_EXTRAS" = 1 ]; then install_extra_apps; fi
    if [ "$DO_BUILD" = 1 ]; then ensure_rust; build; fi
    if [ "$DO_INSTALL" = 1 ]; then install_zen; fi
    if [ "$W_CONFIG" = 1 ]; then write_user_config; fi
    if [ "$W_GREETER" = 1 ]; then install_greeter; fi

    printf '\n%sdone%s\n' "$C_GREEN$C_BOLD" "$C_RESET"
    if [ "$DO_UPDATE" = 1 ]; then update_epilogue; fi
    if [ "$DO_BUILD" = 1 ] && [ "$DO_INSTALL" = 0 ]; then
        dim "binary at target/$BUILD_PROFILE/zen - ./setup.sh --install to install it"
    fi
}

main "$@"
