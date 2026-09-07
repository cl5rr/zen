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

# Self-update needs both of these resolved before anything else runs. "$@" inside a
# function is that function's args, and "$0" stops meaning anything useful the moment
# a relative path outlives a directory change.
ZEN_ARGV=("$@")
ZEN_SELF="$(cd "$(dirname "$0")" 2>/dev/null && pwd)/$(basename "$0")"

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
  --greeter WHICH    Set up a login screen: ly, greetd, or none. Choosing ly
                     removes greetd first, since both want VT 1.
  --debug            Build the debug profile instead of release.
  --with-visual-tests
                     Also build zen-visual-tests, the shader iteration harness.
                     GTK4/libadwaita are checked either way, for the settings app.
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
        --greeter)              shift; GREETER="${1:-ask}"; W_GREETER=1 ;;
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
dbus-1|desktop integration
libpipewire-0.3|screencasting and the screen-share portal"

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
        libpipewire-0.3) echo pipewire ;;
        esac ;;
    apt) case "$lib" in
        wayland-server|wayland-client) echo libwayland-dev ;;
        libinput) echo libinput-dev ;; libudev) echo libudev-dev ;;
        xkbcommon) echo libxkbcommon-dev ;; gbm) echo libgbm-dev ;;
        egl) echo libegl1-mesa-dev ;; libseat) echo libseat-dev ;;
        libdisplay-info) echo libdisplay-info-dev ;;
        pangocairo) echo libpango1.0-dev ;; dbus-1) echo libdbus-1-dev ;;
        libpipewire-0.3) echo libpipewire-0.3-dev ;;
        esac ;;
    dnf) case "$lib" in
        wayland-server|wayland-client) echo wayland-devel ;;
        libinput) echo libinput-devel ;; libudev) echo systemd-devel ;;
        xkbcommon) echo libxkbcommon-devel ;; gbm) echo libgbm-devel ;;
        egl) echo mesa-libEGL-devel ;; libseat) echo libseat-devel ;;
        libdisplay-info) echo libdisplay-info-devel ;;
        pangocairo) echo pango-devel ;; dbus-1) echo dbus-devel ;;
        libpipewire-0.3) echo pipewire-devel ;;
        esac ;;
    apk) case "$lib" in
        wayland-server|wayland-client) echo wayland-dev ;;
        libinput) echo libinput-dev ;; libudev) echo eudev-dev ;;
        xkbcommon) echo libxkbcommon-dev ;; gbm|egl) echo mesa-dev ;;
        libseat) echo libseat-dev ;; libdisplay-info) echo libdisplay-info-dev ;;
        pangocairo) echo pango-dev ;; dbus-1) echo dbus-dev ;;
        libpipewire-0.3) echo pipewire-dev ;;
        esac ;;
    zypper) case "$lib" in
        wayland-server|wayland-client) echo wayland-devel ;;
        libinput) echo libinput-devel ;; libudev) echo systemd-devel ;;
        xkbcommon) echo libxkbcommon-devel ;; gbm) echo libgbm-devel ;;
        egl) echo Mesa-libEGL-devel ;; libseat) echo libseat-devel ;;
        libdisplay-info) echo libdisplay-info-devel ;;
        pangocairo) echo pango-devel ;; dbus-1) echo dbus-1-devel ;;
        libpipewire-0.3) echo pipewire-devel ;;
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

gtk_packages_for() {
    case "$1" in
        pacman) echo "gtk4 libadwaita" ;; apt) echo "libgtk-4-dev libadwaita-1-dev" ;;
        dnf)    echo "gtk4-devel libadwaita-devel" ;; apk) echo "gtk4.0-dev libadwaita-dev" ;;
        zypper) echo "gtk4-devel libadwaita-devel" ;;
    esac
}

# What a session needs at runtime, as opposed to what the build needs. None of this
# is linked into the binary, so a build can succeed and the session still be unusable:
# no X11 apps, no file picker, no fonts. Checked by command name, because that is what
# actually has to be on PATH.
#
#   Xwayland             the X server itself
#   xwayland-satellite   what actually starts it. ZEN does not run Xwayland directly;
#                        without this binary there is no $DISPLAY and every X11 app
#                        fails with "Missing X server or $DISPLAY"
#   xdg-desktop-portal   file pickers, screen sharing, and the "open with" dialog
#   dbus-daemon          the session bus nearly every desktop app talks to
#   fc-list              font discovery; with no fonts nothing draws text at all
RUNTIME_PROGS="Xwayland|the X server, for Discord, Steam and older Electron apps
xwayland-satellite|what starts it. With no satellite there is no X server at all
xdg-desktop-portal|file pickers, screen sharing and app portals
dbus-daemon|the session bus nearly every desktop app needs
fc-list|font discovery, without which nothing draws text"

runtime_package_for() {
    local mgr="$1" cmd="$2"
    case "$mgr" in
    pacman) case "$cmd" in
        Xwayland) echo xorg-xwayland ;; xwayland-satellite) echo xwayland-satellite ;;
        xdg-desktop-portal) echo xdg-desktop-portal ;;
        dbus-daemon) echo dbus ;; fc-list) echo fontconfig ;;
        esac ;;
    apt) case "$cmd" in
        Xwayland) echo xwayland ;; xwayland-satellite) echo xwayland-satellite ;;
        xdg-desktop-portal) echo xdg-desktop-portal ;;
        dbus-daemon) echo dbus ;; fc-list) echo fontconfig ;;
        esac ;;
    dnf) case "$cmd" in
        Xwayland) echo xorg-x11-server-Xwayland ;; xwayland-satellite) echo xwayland-satellite ;;
        xdg-desktop-portal) echo xdg-desktop-portal ;;
        dbus-daemon) echo dbus ;; fc-list) echo fontconfig ;;
        esac ;;
    apk) case "$cmd" in
        Xwayland) echo xwayland ;; xwayland-satellite) echo xwayland-satellite ;;
        xdg-desktop-portal) echo xdg-desktop-portal ;;
        dbus-daemon) echo dbus ;; fc-list) echo fontconfig ;;
        esac ;;
    zypper) case "$cmd" in
        Xwayland) echo xwayland ;; xwayland-satellite) echo xwayland-satellite ;;
        xdg-desktop-portal) echo xdg-desktop-portal ;;
        dbus-daemon) echo dbus ;; fc-list) echo fontconfig ;;
        esac ;;
    esac
}

# A portal with no backend answers nothing, which is the usual reason a file picker
# never opens and a screen share hangs at a blank chooser. Any one of these will do.
portal_backend_package_for() {
    case "$1" in
        pacman) echo xdg-desktop-portal-gtk ;; apt) echo xdg-desktop-portal-gtk ;;
        dnf)    echo xdg-desktop-portal-gtk ;; apk) echo xdg-desktop-portal-gtk ;;
        zypper) echo xdg-desktop-portal-gtk ;;
    esac
}

# ScreenCast is only implemented by some backends. gnome is the one ZEN targets.
screencast_backend_package_for() {
    case "$1" in
        pacman) echo xdg-desktop-portal-gnome ;; apt) echo xdg-desktop-portal-gnome ;;
        dnf)    echo xdg-desktop-portal-gnome ;; apk) echo xdg-desktop-portal-gnome ;;
        zypper) echo xdg-desktop-portal-gnome ;;
    esac
}

# Emoji, plus the symbol and arrow ranges a status bar reaches for. Noto is the one
# every distro packages and the one with the widest coverage.
emoji_packages_for() {
    case "$1" in
        pacman) echo "noto-fonts noto-fonts-emoji ttf-nerd-fonts-symbols" ;;
        apt)    echo "fonts-noto-core fonts-noto-color-emoji" ;;
        dnf)    echo "google-noto-sans-fonts google-noto-color-emoji-fonts" ;;
        apk)    echo "font-noto font-noto-emoji" ;;
        zypper) echo "noto-sans-fonts noto-coloremoji-fonts" ;;
    esac
}

font_package_for() {
    case "$1" in
        pacman) echo ttf-dejavu ;; apt) echo fonts-dejavu-core ;;
        dnf)    echo dejavu-sans-fonts ;; apk) echo font-dejavu ;;
        zypper) echo dejavu-fonts ;;
    esac
}

git_package_for() {
    case "$1" in
        pacman) echo git ;; apt) echo git ;; dnf) echo git ;;
        apk)    echo git ;; zypper) echo git ;;
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

    # This script updates itself with git, so an install without git can be built
    # once and then never updated again.
    if have git; then
        row_ok "git" "$(git --version 2>/dev/null)"
        N_OK=$((N_OK + 1))
    else
        row_miss "git" "required by ./setup.sh --update"
        add_missing "$(git_package_for "$PKG_MGR")"
        N_MISSING=$((N_MISSING + 1))
    fi

    # Rust is reported here but installed separately, via rustup.
    local PATH_SAVE="$PATH"
    [ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"
    if have cargo; then
        # Presence is not enough: a distro rust a few releases behind fails deep in a
        # dependency with an error that says nothing about the version.
        local rust_ver rust_major rust_minor
        rust_ver="$(cargo --version 2>/dev/null | awk '{print $2}')"
        rust_major="${rust_ver%%.*}"
        rust_minor="${rust_ver#*.}"; rust_minor="${rust_minor%%.*}"
        if [ "${rust_major:-0}" -gt 1 ] 2>/dev/null             || { [ "${rust_major:-0}" -eq 1 ] && [ "${rust_minor:-0}" -ge 87 ]; } 2>/dev/null; then
            row_ok "Rust" "$(cargo --version 2>/dev/null)"
            N_OK=$((N_OK + 1))
        else
            row_miss "Rust" "$rust_ver is too old, ZEN needs >= 1.87 (rustup update)"
            N_MISSING=$((N_MISSING + 1))
        fi
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

    # The settings app. Optional, but it is a shipped feature on Mod+, so a plain
    # install should get it rather than silently skipping it.
    printf '\n    %ssettings app (GTK4)%s\n' "$C_BOLD" "$C_RESET"
    if pkg-config --exists gtk4 2>/dev/null && pkg-config --exists libadwaita-1 2>/dev/null; then
        row_ok "gtk4 + libadwaita" "$(pkg-config --modversion gtk4 2>/dev/null)"
        N_OK=$((N_OK + 1))
    else
        row_miss "gtk4 + libadwaita" "zen-settings, and the shader harness"
        for pkg in $(gtk_packages_for "$PKG_MGR"); do add_missing "$pkg"; done
        N_MISSING=$((N_MISSING + 1))
    fi

    check_runtime
    check_system

    printf '\n'
    if [ "$N_MISSING" -eq 0 ] && [ "$N_UNKNOWN" -eq 0 ] && [ -z "${MISSING_PKGS# }" ]; then
        printf '    %sall %s checks passed - nothing to install.%s\n' "$C_GREEN" "$N_OK" "$C_RESET"
    elif [ "$N_MISSING" -eq 0 ] && [ "$N_UNKNOWN" -eq 0 ]; then
        printf '    %sall %s checks passed, plus optional packages below.%s\n' "$C_GREEN" "$N_OK" "$C_RESET"
    elif [ "$N_UNKNOWN" -gt 0 ]; then
        printf '    %s%s present, %s missing, %s unverifiable until pkg-config is installed.%s\n' \
            "$C_BOLD" "$N_OK" "$N_MISSING" "$N_UNKNOWN" "$C_RESET"
    else
        printf '    %s%s present, %s missing.%s\n' "$C_BOLD" "$N_OK" "$N_MISSING" "$C_RESET"
    fi
}

# Everything a session needs once ZEN is running. A build can succeed and leave you
# with no X11 apps, no file picker and no fonts, and none of that shows up as a build
# error, so it gets its own pass.
check_runtime() {
    printf '\n    %sruntime programs%s\n' "$C_BOLD" "$C_RESET"

    local cmd desc pkg
    while IFS='|' read -r cmd desc; do
        [ -n "$cmd" ] || continue
        if have "$cmd"; then
            row_ok "$cmd" ""
            N_OK=$((N_OK + 1))
        else
            row_miss "$cmd" "$desc"
            add_missing "$(runtime_package_for "$PKG_MGR" "$cmd")"
            N_MISSING=$((N_MISSING + 1))
        fi
    done <<EOF
$RUNTIME_PROGS
EOF

    # Checked whether or not the portal itself is present: installing xdg-desktop-portal
    # alone leaves you in exactly the broken state this row is about. ZEN's shipped
    # zen-portals.conf asks for the gnome backend, then the gtk one.
    if have xdg-desktop-portal-gtk || have xdg-desktop-portal-wlr ||
        have xdg-desktop-portal-gnome || have xdg-desktop-portal-kde ||
        ls /usr/libexec/xdg-desktop-portal-* /usr/lib/xdg-desktop-portal-* >/dev/null 2>&1
    then
        row_ok "portal backend" ""
        N_OK=$((N_OK + 1))
    else
        row_miss "portal backend" "a portal with no backend never answers a file picker"
        add_missing "$(portal_backend_package_for "$PKG_MGR")"
        N_MISSING=$((N_MISSING + 1))
    fi

    # Screen sharing is a separate backend from the file picker. ZEN implements
    # screencasting against xdg-desktop-portal-gnome, and the gtk backend does not
    # implement ScreenCast at all, so with only gtk installed OBS records a black frame
    # and nothing says why.
    if have xdg-desktop-portal-gnome         || ls /usr/libexec/xdg-desktop-portal-gnome /usr/lib/xdg-desktop-portal-gnome              >/dev/null 2>&1; then
        row_ok "screen sharing" ""
        N_OK=$((N_OK + 1))
    else
        row_miss "screen sharing" "OBS and screen share need xdg-desktop-portal-gnome"
        add_missing "$(screencast_backend_package_for "$PKG_MGR")"
        N_MISSING=$((N_MISSING + 1))
    fi

    # fontconfig being installed says nothing about there being a font to find.
    if have fc-list; then
        local nfonts nmono
        nfonts="$(fc-list 2>/dev/null | wc -l || true)"
        nmono="$(fc-list :spacing=100 2>/dev/null | wc -l || true)"
        if [ "${nfonts:-0}" -eq 0 ]; then
            row_miss "fonts" "fontconfig is installed but finds no font at all"
            add_missing "$(font_package_for "$PKG_MGR")"
            N_MISSING=$((N_MISSING + 1))
        elif [ "${nmono:-0}" -eq 0 ]; then
            row_miss "monospace font" "the shipped terminal themes ask for one by name"
            add_missing "$(font_package_for "$PKG_MGR")"
            N_MISSING=$((N_MISSING + 1))
        else
            row_ok "fonts" "$nfonts installed, $nmono monospace"
            N_OK=$((N_OK + 1))
        fi

        # Emoji and the symbol ranges are a separate font from the text one, and
        # nothing degrades gracefully without them: a missing glyph is a hollow box in
        # a chat window, a bar, or a window title. Asked for by codepoint rather than
        # by package name, because the package differs per distro and per font.
        if fc-list ':charset=1F600' 2>/dev/null | grep -q .             && fc-list ':charset=2764' 2>/dev/null | grep -q .; then
            row_ok "emoji" ""
            N_OK=$((N_OK + 1))
        else
            row_miss "emoji" "without this, emoji render as hollow boxes everywhere"
            for pkg in $(emoji_packages_for "$PKG_MGR"); do add_missing "$pkg"; done
            N_MISSING=$((N_MISSING + 1))
        fi
    fi

    printf '\n    %swhat the keybinds spawn%s\n' "$C_BOLD" "$C_RESET"
    local pair
    for pair in $BIND_APPS; do
        cmd="${pair%%:*}"
        pkg="${pair##*:}"
        if have "$cmd"; then
            row_ok "$cmd" ""
            N_OK=$((N_OK + 1))
        else
            row_miss "$cmd" "$pkg"
            add_missing "$pkg"
            N_MISSING=$((N_MISSING + 1))
        fi
    done
}

# Conditions that no package can fix, so these report rather than add to the install
# list. Each one is something that makes ZEN fail to start in a way whose error
# message does not name the cause.
check_system() {
    printf '\n    %ssystem%s\n' "$C_BOLD" "$C_RESET"

    # No render node means no GPU to draw with, and the TTY backend simply exits.
    if ls /dev/dri/card* >/dev/null 2>&1; then
        row_ok "GPU" "$(ls -d /dev/dri/card* 2>/dev/null | tr '\n' ' ' || true)"
        N_OK=$((N_OK + 1))
    else
        row_miss "GPU" "no /dev/dri/card*; ZEN can run nested but not on a TTY"
        N_MISSING=$((N_MISSING + 1))
    fi

    # Seat management hands over the GPU and the input devices. With neither, ZEN
    # starts and then cannot open a single device.
    if [ -d /run/systemd/system ]; then
        row_ok "seat" "systemd-logind"
        N_OK=$((N_OK + 1))
    elif have seatd; then
        if pgrep -x seatd >/dev/null 2>&1; then
            row_ok "seat" "seatd running"
            N_OK=$((N_OK + 1))
        else
            row_miss "seat" "seatd is installed but not running (systemctl enable --now seatd)"
            N_MISSING=$((N_MISSING + 1))
        fi
    else
        row_miss "seat" "no logind and no seatd; nothing can hand ZEN the GPU"
        add_missing "seatd"
        N_MISSING=$((N_MISSING + 1))
    fi

    # Without logind, device access is group membership and nothing else.
    if [ ! -d /run/systemd/system ]; then
        local groups; groups="$(id -nG 2>/dev/null || true)"
        local want missing_groups=""
        for want in video input seat; do
            case " $groups " in *" $want "*) ;; *) missing_groups="$missing_groups $want" ;; esac
        done
        if [ -n "$missing_groups" ]; then
            local who="${USER:-$(id -un 2>/dev/null || echo you)}"
            row_miss "groups" "sudo usermod -aG$(echo "${missing_groups# }" | tr ' ' ',') $who"
            N_MISSING=$((N_MISSING + 1))
        else
            row_ok "groups" "video input seat"
            N_OK=$((N_OK + 1))
        fi
    fi

    # Audio and screencasting both ride on PipeWire at runtime, separately from the
    # headers the build needs.
    if have pipewire; then
        row_ok "pipewire" "$(pipewire --version 2>/dev/null | head -1 || true)"
        N_OK=$((N_OK + 1))
    else
        row_miss "pipewire" "audio and screen sharing at runtime"
        add_missing "pipewire"
        N_MISSING=$((N_MISSING + 1))
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
        # Same reasoning as the declined prompt below: on an update this is
        # information, not a reason to stop, because what is installed still needs
        # rebuilding from the code that was just pulled.
        [ "$DO_UPDATE" = 1 ] || exit 1
        return 0
    fi

    step "Installing missing packages"
    # shellcheck disable=SC2086
    set -- $MISSING_PKGS
    info "$# package(s) via $PKG_MGR:"
    dim "$*"

    # On an update, declining is a decision about packages, not about the update. Only a
    # fresh install treats it as aborting, because there the build needs them.
    if ! confirm; then
        [ "$DO_UPDATE" = 1 ] || die "aborted"
        dim "skipped; ZEN will still be built and installed"
        return 0
    fi

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

    # The settings app needs gtk4 and libadwaita, which the compositor does not.
    # It is a nicety, so a missing toolkit is a note rather than a failure.
    #
    # The binary is removed first on purpose. install_zen installs whatever is at that
    # path, so a build that fails here would leave the previous one in place and get it
    # reinstalled: the update reports success and the app silently stays old. That is
    # exactly what happened with the settings pages added after the first install.
    local settings="target/$BUILD_PROFILE/zen-settings"
    local had_settings=no
    [ -x "$settings" ] && had_settings=yes
    rm -f "$settings"

    if pkg-config --exists gtk4 libadwaita-1 2>/dev/null; then
        # shellcheck disable=SC2086
        if cargo $args -p zen-settings; then
            ok "$settings"
        else
            warn "the settings app did not build, so it will NOT be updated this run"
            warn "ZEN itself is unaffected. Send the cargo error above if you want it fixed"
        fi
    elif [ "$had_settings" = yes ]; then
        warn "gtk4/libadwaita are gone, so the settings app cannot be rebuilt"
        warn "install them and run this again: sudo pacman -S gtk4 libadwaita"
    else
        dim "gtk4/libadwaita missing, skipping the settings app (pacman -S gtk4 libadwaita)"
    fi
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
    $SUDO install -Dm755 resources/zen-wallpaper    "$PREFIX/bin/zen-wallpaper"
    $SUDO install -Dm755 resources/zen-lock         "$PREFIX/bin/zen-lock"
    $SUDO install -Dm755 resources/zen-power        "$PREFIX/bin/zen-power"
    $SUDO install -Dm644 resources/default-wallpaper.jpg \
                                                    "$PREFIX/share/zen/default-wallpaper.jpg"
    for wp in resources/wallpapers/*; do
        [ -f "$wp" ] && $SUDO install -Dm644 "$wp" \
            "$PREFIX/share/zen/wallpapers/$(basename "$wp")"
    done
    $SUDO install -Dm644 resources/zen.desktop      "$PREFIX/share/wayland-sessions/zen.desktop"
    $SUDO install -Dm644 resources/zen-portals.conf "$PREFIX/share/xdg-desktop-portal/zen-portals.conf"
    $SUDO install -Dm644 resources/zen.png          "$PREFIX/share/pixmaps/zen.png"
    $SUDO install -Dm644 resources/zen.png          "$PREFIX/share/icons/hicolor/512x512/apps/zen.png"

    if [ -x "target/$BUILD_PROFILE/zen-settings" ]; then
        $SUDO install -Dm755 "target/$BUILD_PROFILE/zen-settings" "$PREFIX/bin/zen-settings"
        $SUDO install -Dm644 resources/zen-settings.desktop                                                     "$PREFIX/share/applications/zen-settings.desktop"
        dim "settings app installed; ${C_BOLD}Mod+,${C_RESET} opens it"
    elif [ -x "$PREFIX/bin/zen-settings" ]; then
        warn "$PREFIX/bin/zen-settings is left at its old version; see the build warning above"
    fi

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

    # Sections are only half the story. A config written by an older ZEN keeps every
    # section it already had and carries stale values inside them, which then override
    # the new defaults silently. So compare every node name, not just the top level.
    local tmp_ship tmp_user
    tmp_ship=$(mktemp) || return 0
    tmp_user=$(mktemp) || { rm -f "$tmp_ship"; return 0; }

    config_names "$shipped" > "$tmp_ship"
    config_names "$user"    > "$tmp_user"

    local missing
    missing=$(comm -23 "$tmp_ship" "$tmp_user" | tr '\n' ' ')
    rm -f "$tmp_ship" "$tmp_user"

    missing="${missing%% }"

    # Missing node names are only the loud half. A changed *value* on a node you
    # already have is silent, and that is how a whole visual change goes missing.
    local changed
    changed=$(diff "$user" "$shipped" 2>/dev/null | grep -c '^[<>]')
    [ -n "$missing" ] || [ "${changed:-0}" -gt 0 ] || return 0

    printf '\n'
    if [ -n "$missing" ]; then
        local count
        count=$(printf '%s\n' $missing | wc -l | tr -d ' ')
        warn "your config does not have $count setting(s) that ZEN now ships"
        dim "$(printf '%s' "$missing" | cut -c1-300)"
    else
        warn "your config differs from the one ZEN ships"
    fi
    dim "$changed line(s) differ in total, values included"
    printf '\n'
    info "Your config wins over the defaults, so anything newer is not reaching you."
    info "The look lives there too: shadows, corners, the glass material and the"
    info "window transparency are config, not binary."
    printf '\n'
    dim "diff:  diff $user resources/default-config.kdl"

    if [ "$UI_TTY" != 1 ]; then
        dim "run ./setup.sh --update from a terminal to be offered the new config"
        return 0
    fi

    ui_menu "Take the config ZEN ships now?" \
        "Yes, and keep mine as config.kdl.bak" \
        "No, leave my config alone" || return 0
    [ "$UI_CHOICE" = 0 ] || return 0

    cp "$user" "$user.bak" || return 0
    cp "$shipped" "$user"  || return 0
    ok "installed the shipped config; yours is at $user.bak"
    dim "carry anything of your own over from the backup: output position, keyboard layout"
}

# Every node name in a config, one per line, sorted. Comments stripped, "/-" ignored,
# so a commented-out node does not count as present.
config_names() {
    sed 's|//.*||' "$1" \
        | grep -oE '^[[:space:]]{0,8}[A-Za-z][A-Za-z0-9+-]*' \
        | tr -d '[:blank:]' \
        | sort -u
}

# Whether a ZEN session is running right now, so we can say when the update lands.
zen_is_running() {
    pgrep -x zen >/dev/null 2>&1
}

# --------------------------------------------------------------- self ----
#
# bash executes a script as it reads it, so the copy running right now is the one
# that was on disk before the pull. Any step added by the very commit being fetched
# would be skipped, and the next run would be the first to have it. Restarting is
# the only way a pull can take effect on the run that performed it.

self_fingerprint() {
    cksum < "$ZEN_SELF" 2>/dev/null || wc -c < "$ZEN_SELF" 2>/dev/null || echo unknown
}

reexec_if_self_changed() {
    local before="$1" after
    after=$(self_fingerprint)
    [ "$before" != "$after" ] || return 0

    # One restart is enough: the second run pulls nothing, so a second change means
    # something is wrong rather than something new.
    if [ "${ZEN_SETUP_REEXEC:-0}" = 1 ]; then
        warn "setup.sh changed again after one restart; carrying on without another"
        return 0
    fi

    # Never hand control to a script that will not parse. A half-written checkout
    # should leave you on the working old copy, not a broken new one.
    if ! bash -n "$ZEN_SELF" 2>/dev/null; then
        warn "the setup.sh just pulled does not parse; continuing with the running one"
        dim "run ${C_BOLD}./setup.sh --update${C_RESET} again once the checkout is sound"
        return 0
    fi

    ok "setup.sh itself changed; restarting with the new one"
    export ZEN_SETUP_REEXEC=1
    exec bash "$ZEN_SELF" ${ZEN_ARGV[@]+"${ZEN_ARGV[@]}"}

    # exec only returns on failure, and continuing would silently run stale code.
    die "could not restart setup.sh at $ZEN_SELF"
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

    local before after self_before
    before=$(git rev-parse HEAD)
    self_before=$(self_fingerprint)
    info "fetching"
    if ! git pull --ff-only; then
        printf '\n'
        warn "the pull did not fast-forward"
        dim "if it says 'diverged', the remote history was rewritten and yours no longer"
        dim "matches it. Check ${C_BOLD}git status${C_RESET} for work of your own, then take the remote:"
        dim "    ${C_BOLD}git fetch origin && git reset --hard origin/main${C_RESET}"
        die "not pulling over a diverged history on my own"
    fi

    after=$(git rev-parse HEAD)
    reexec_if_self_changed "$self_before"

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

# Everything the shipped keybinds actually spawn, grouped by how much you would
# miss it. Derived from the spawn lines in resources/default-config.kdl; if you add
# a bind that spawns something, add its package here too.
#
#   core      Mod+T, Mod+Space, the wallpaper, the lock screen
#   media     the XF86 keys: volume, brightness, play/pause
#   apps      Mod+W, Mod+E, Mod+D and notifications
DESKTOP_APPS="alacritty fuzzel swaybg swaylock swayidle waybar"
MEDIA_APPS="wireplumber playerctl brightnessctl xdg-utils wl-clipboard cliphist"
EXTRA_APPS="firefox nautilus mako"
GREETER_PKGS="greetd cage greetd-regreet"
GREETER_LY_PKGS="ly"

pacman_install() {
    [ "$PKG_MGR" = pacman ] || {
        warn "only pacman is supported for this step; install these yourself: $*"
        return 1
    }
    need_root
    $SUDO pacman -S --needed --noconfirm "$@"
}

install_desktop_apps() {
    step "Installing what the keybinds expect"
    dim "Mod+T terminal, Mod+Space launcher, Mod+Shift+W wallpaper, Mod+Shift+Escape lock"
    pacman_install $DESKTOP_APPS || return 1
    ok "terminal, launcher, wallpaper and lock installed"

    dim "media and brightness keys: wpctl, playerctl, brightnessctl"
    pacman_install $MEDIA_APPS && ok "media keys will work"
}

# What the shipped keybinds spawn, as "command:package" pairs. Checked by command
# because that is what a bind actually needs to find on PATH.
BIND_APPS="waybar:waybar wl-copy:wl-clipboard cliphist:cliphist swayidle:swayidle alacritty:alacritty fuzzel:fuzzel swaybg:swaybg swaylock:swaylock wpctl:wireplumber playerctl:playerctl brightnessctl:brightnessctl xdg-open:xdg-utils firefox:firefox nautilus:nautilus mako:mako"

install_extra_apps() {
    step "Installing optional extras"
    dim "Mod+W browser, Mod+E files, and a notification daemon"
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

    seed_wallpapers
    theme_all
}

# Themes the app launcher, unless you already have a config of your own.
#
# fuzzel's stock look is a grey box that reads as an unstyled dialog on a dark
# canvas. This is the single cheapest thing that stops ZEN looking half-dressed.
theme_launcher() {
    local base="${XDG_CONFIG_HOME:-$HOME/.config}"
    theme_file "$base/fuzzel/fuzzel.ini" resources/fuzzel.ini "the app launcher"
}

# Themes the terminal, unless you already have a config of your own.
#
# Without this the terminal is opaque, which hides the glass material entirely and
# makes a fresh install look like any other compositor.
theme_terminal() {
    local base="${XDG_CONFIG_HOME:-$HOME/.config}"
    theme_file "$base/alacritty/alacritty.toml" resources/alacritty.toml "alacritty"
    theme_file "$base/foot/foot.ini"            resources/foot.ini      "foot"
    theme_file "$base/kitty/kitty.conf"         resources/kitty.conf    "kitty"
}

# Ly ships a config.ini whose keys vary between versions, so this only rewrites keys
# that are already in the file rather than replacing it wholesale. Anything Ly does
# not have is simply skipped instead of being silently ignored at runtime.
theme_ly() {
    local cfg=/etc/ly/config.ini
    [ -f "$cfg" ] || { dim "no $cfg to theme; Ly will use its defaults"; return 0; }

    need_root
    if [ ! -f "$cfg.zen-backup" ]; then
        $SUDO cp "$cfg" "$cfg.zen-backup"
        dim "kept your original as $cfg.zen-backup"
    fi

    # ZEN's ground and accent, in the terminal colours Ly speaks.
    ly_set "$cfg" bg 0
    ly_set "$cfg" fg 15
    ly_set "$cfg" border_fg 6
    ly_set "$cfg" clock "%H:%M"
    ly_set "$cfg" animation none
    ly_set "$cfg" asterisk "*"
    ly_set "$cfg" blank_box true
    ly_set "$cfg" hide_key_hints false
    ly_set "$cfg" save true
    ly_set "$cfg" waylandsessions /usr/share/wayland-sessions

    ok "themed the login screen: $cfg"
}

# Rewrites one key only if Ly already knows it, commented out or not.
ly_set() {
    local cfg="$1" key="$2" value="$3"
    grep -qE "^[#[:space:]]*$key[[:space:]]*=" "$cfg" || return 0
    $SUDO sed -i -E "s|^[#[:space:]]*$key[[:space:]]*=.*|$key = $value|" "$cfg"
}

# An unconfigured swaylock is a blank white panel you cannot tell apart from a
# crash, which is worse than no lock screen at all.
# The status bar. Two files, and the same rule as every other theme: installed only
# where there is nothing already, updated later only if it is still ZEN's copy.
theme_bar() {
    local base="${XDG_CONFIG_HOME:-$HOME/.config}"
    theme_file "$base/waybar/config.jsonc" resources/waybar/config.jsonc "waybar"
    theme_file "$base/waybar/style.css"    resources/waybar/style.css    "the waybar style"
    theme_file "$base/waybar/zen-pills.css" resources/waybar/zen-pills.css "the pill fill"
}

theme_lock() {
    local base="${XDG_CONFIG_HOME:-$HOME/.config}"
    theme_file "$base/swaylock/config" resources/swaylock.conf "swaylock"
}

# True when the file on disk is byte-identical to some version ZEN has shipped, so
# it is a copy we installed that has never been edited. That is the only case where
# overwriting is safe, and it is what lets a fix to a shipped theme reach someone who
# already has the broken copy. resources/theme-history.txt is generated by
# scripts/theme-history.sh from git history.
is_unmodified_zen_theme() {
    local dst="$1" src="$2" history="resources/theme-history.txt"
    [ -f "$history" ] || return 1
    command -v sha256sum >/dev/null 2>&1 || return 1
    local have
    have=$(sha256sum < "$dst" | cut -d' ' -f1)
    grep -q "^$src $have\$" "$history"
}

theme_file() {
    local dst="$1" src="$2" what="$3"
    [ -f "$src" ] || return 0
    if [ -f "$dst" ]; then
        if cmp -s "$src" "$dst"; then
            dim "$what theme already current: $dst"
            return 0
        fi
        if is_unmodified_zen_theme "$dst" "$src"; then
            cp "$src" "$dst" || return 0
            ok "updated the $what theme ZEN installed: $dst"
            return 0
        fi
        dim "you already have a $what config, left alone: $dst"
        dim "ZEN ships a newer one at $src if you want to compare"
        return 0
    fi
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst" || return 0
    ok "themed $what: $dst"
}

# Every shipped theme, applied only where the user has no config of their own.
# Safe to run repeatedly, which is why the update path can call it too: a theme
# added after someone installed would otherwise never reach them.
theme_all() {
    theme_launcher
    theme_terminal
    theme_lock
    theme_bar
}

# Puts the shipped wallpaper where the picker looks, so a fresh install has one.
#
# Copied rather than symlinked: it lands in a folder the user is invited to fill with
# their own images, and a symlink into /usr/share would be a surprise to delete.
seed_wallpapers() {
    local dir="${XDG_CONFIG_HOME:-$HOME/.config}/zen/wallpapers"
    mkdir -p "$dir"

    if [ -n "$(ls -A "$dir" 2>/dev/null)" ]; then
        dim "wallpaper folder already has images, left alone: $dir"
        return 0
    fi

    # Numbered so the one people see on first boot sorts first; the rest join the cycle.
    local n=0 wp
    for wp in resources/wallpapers/*; do
        [ -f "$wp" ] || continue
        cp "$wp" "$dir/" 2>/dev/null && n=$((n + 1))
    done

    if [ "$n" = 0 ]; then
        cp resources/default-wallpaper.jpg "$dir/01-forest.jpg" 2>/dev/null && n=1
    fi

    ok "wallpaper folder seeded with $n image(s): $dir"
    dim "drop more in there; ${C_BOLD}Mod+Shift+W${C_RESET} picks between them"
}

install_greeter() {
    if [ "$UI_TTY" = 1 ] && [ "$GREETER" = ask ]; then
        ui_menu "Which login screen?"             "Ly    (a small TTY greeter, no GTK, no Wayland session of its own)"             "greetd + ReGreet  (graphical, heavier, needs cage)"             "Skip" || return 0
        case "$UI_CHOICE" in
            0) GREETER=ly ;;
            1) GREETER=greetd ;;
            *) return 0 ;;
        esac
    fi

    case "$GREETER" in
        ly) install_greeter_ly ;;
        greetd|ask) install_greeter_greetd ;;
        none) return 0 ;;
    esac
}

# Ly reads /usr/share/wayland-sessions, which is where install_zen already puts
# zen.desktop, so there is nothing to configure: ZEN just appears in its list.
install_greeter_ly() {
    step "Installing the login screen"
    dim "Ly is a TTY greeter: it lists the sessions it finds and gets out of the way"

    remove_greetd_if_present

    pacman_install $GREETER_LY_PKGS || return 1
    need_root

    theme_ly

    ok "Ly installed"
    greeter_epilogue "ly"
}

install_greeter_greetd() {
    step "Installing the login screen"
    dim "greetd runs the session, ReGreet draws it, cage hosts it"

    pacman_install $GREETER_PKGS || return 1

    need_root
    if [ -f /etc/greetd/config.toml ] && ! grep -q regreet /etc/greetd/config.toml 2>/dev/null; then
        $SUDO cp /etc/greetd/config.toml /etc/greetd/config.toml.bak
        dim "existing config saved as /etc/greetd/config.toml.bak"
    fi

    $SUDO mkdir -p /etc/greetd
    printf '%s
'         '[terminal]'         'vt = 1'         ''         '[default_session]'         'command = "cage -s -- regreet"'         'user = "greeter"'         | $SUDO tee /etc/greetd/config.toml >/dev/null

    if [ ! -f /etc/greetd/regreet.toml ]; then
        printf '%s
'             '[background]'             'path = "/usr/share/pixmaps/zen.png"'             'fit = "Cover"'             ''             '[GTK]'             'application_prefer_dark_theme = true'             'cursor_theme_name = "Adwaita"'             'font_name = "Cantarell 14"'             | $SUDO tee /etc/greetd/regreet.toml >/dev/null
    fi

    ok "greetd configured"
    greeter_epilogue "greetd"
}

# Two greeters both wanting VT 1 is a black screen, so the old one goes first.
remove_greetd_if_present() {
    have greetd || [ -f /etc/greetd/config.toml ] || return 0

    need_root
    info "removing greetd first, so the two do not fight over the same VT"

    $SUDO systemctl disable --now greetd 2>/dev/null || true
    if [ "$PKG_MGR" = pacman ]; then
        $SUDO pacman -Rns --noconfirm greetd greetd-regreet cage 2>/dev/null             || $SUDO pacman -Rns --noconfirm greetd 2>/dev/null || true
    fi
    [ -d /etc/greetd ] && $SUDO mv /etc/greetd /etc/greetd.removed 2>/dev/null
    ok "greetd disabled and removed; its config is at /etc/greetd.removed"
}

greeter_epilogue() {
    local unit="$1"
    local enable="$unit" disable=""

    # Ly is a template unit: it runs on a specific tty, and the getty already sitting
    # on that tty has to go or the two fight over it.
    if [ "$unit" = ly ]; then
        enable="ly@tty2.service"
        disable="getty@tty2.service"
    fi
    printf '
'
    warn "NOT enabling it yet, on purpose."
    info "Test ZEN from a TTY first:  ${C_BOLD}zen${C_RESET}"
    info "If that works, enable the login screen with:"
    info "  ${C_BOLD}sudo systemctl enable --now $enable${C_RESET}"
    if [ -n "$disable" ]; then
        info "  ${C_BOLD}sudo systemctl disable --now $disable${C_RESET}"
    fi
    info "If a login screen ever leaves you at a black screen, press"
    info "  ${C_BOLD}Ctrl+Alt+F2${C_RESET} and run:"
    info "  ${C_BOLD}sudo systemctl disable --now $enable${C_RESET}"
    if [ -n "$disable" ]; then
        info "  ${C_BOLD}sudo systemctl enable --now $disable${C_RESET}"
    fi
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
GREETER=ask

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
        1)  DO_UPDATE=1; DO_DEPS=1; DO_BUILD=1; DO_INSTALL=1 ;;
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
    # An update runs the full audit too. A pull can add a bind that spawns something
    # new, or a library a new feature links against, and neither would ever be offered
    # otherwise: --update does not go through the install steps.
    if [ "$DO_UPDATE" = 1 ]; then theme_all; check_deps; install_deps; fi
    if [ "$DO_DEPS" = 1 ] && [ "$DO_UPDATE" = 0 ]; then check_deps; install_deps; fi
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
