#!/usr/bin/env bash
# ZEN setup - checks what you already have, installs only what's missing,
# builds ZEN, and optionally installs it.
#   ./setup.sh --check          report what's present and what's missing
#   ./setup.sh                  install missing deps, then build
#   ./setup.sh --install        ... and install ZEN system-wide
#   ./setup.sh --help           full option list
# System library lists are kept in sync with .github/workflows/ci.yml.

set -euo pipefail

# Self-update needs both of these resolved before anything else runs. "$@" inside a
# function is that function's args, and "$0" stops meaning anything useful the moment
# a relative path outlives a directory change.
ZEN_ARGV=("$@")
ZEN_SELF="$(cd "$(dirname "$0")" 2>/dev/null && pwd)/$(basename "$0")"

PREFIX="${PREFIX:-/usr/local}"
PRESET=""
ASSUME_YES=0
DO_DEPS=1
DO_BUILD=1
DO_INSTALL=0
DO_UPDATE=0
RESET_CONFIG=0
CLASSIC=0
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

Arch Linux and Arch-based distributions only.

  --check            Report what is installed and what is missing, then exit.
  --reset-config     Replace ~/.config/zen/config.kdl with the shipped one, keeping
                     a timestamped backup. The way to pick up new and changed binds.
                     Changes nothing. Safe to run first.
  --update           Pull, rebuild and reinstall. Shows what changed, and any
                     config options you have not got yet.
  --preset WHICH     What to install without asking: essentials, recommended,
                     everything, or a file written by an earlier run
                     (~/.config/zen/setup-choices is the one setup keeps).
  --classic          Use waybar, fuzzel, mako and swaylock instead of ZEN Shell.
  --deps-only        Install missing dependencies and exit.
  --build-only       Skip dependency handling; just build.
  --install          Install ZEN after building (needs root for PREFIX).
  --greeter WHICH    Set up a login screen: zen, ly, greetd, or none. zen is
                     ZEN's own glass greeter on greetd. Choosing ly removes
                     greetd first, since both want VT 1.
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
        --reset-config)         RESET_CONFIG=1; ANY_FLAG=1 ;;
        --update)               DO_UPDATE=1; DO_BUILD=1; DO_INSTALL=1 ;;
        --deps-only)            DO_BUILD=0; DO_INSTALL=0 ;;
        --build-only)           DO_DEPS=0 ;;
        --install)              DO_INSTALL=1 ;;
        --greeter)              shift; GREETER="${1:-ask}"; W_GREETER=1 ;;
        --preset)               [ $# -ge 2 ] || die "--preset needs essentials, recommended, everything or a file"
                                PRESET="$2"; shift ;;
        --preset=*)             PRESET="${1#*=}" ;;
        --classic)              CLASSIC=1 ;;
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

# detection

DISTRO_NAME="unknown"
PKG_MGR=unknown

detect_distro() {
    DISTRO_NAME="unknown"
    if [ -r /etc/os-release ]; then
        DISTRO_NAME="$(. /etc/os-release && printf '%s' "${PRETTY_NAME:-${ID:-unknown}}")"
    fi
    if have pacman; then PKG_MGR=pacman; else PKG_MGR=unknown; fi
}

require_arch() {
    detect_distro
    [ "$PKG_MGR" = pacman ] && return 0
    die "ZEN supports Arch Linux and distributions built on it, and nothing else.

    This looks like $DISTRO_NAME, which has no pacman.
    Arch, EndeavourOS, CachyOS, Manjaro and Garuda all work."
}

# log

SETUP_LOG=""

start_log() {
    local dir="${XDG_CACHE_HOME:-$HOME/.cache}/zen"
    mkdir -p "$dir" 2>/dev/null || return 0
    SETUP_LOG="$dir/setup-$(date +%Y%m%d-%H%M%S).log"
    printf 'ZEN setup, %s, %s\n' "$(date)" "$DISTRO_NAME" > "$SETUP_LOG" 2>/dev/null || SETUP_LOG=""
}

run() {
    printf '    %s$ %s%s\n' "$C_DIM" "$*" "$C_RESET"
    [ -n "$SETUP_LOG" ] && printf '$ %s\n' "$*" >> "$SETUP_LOG"
    local rc=0
    "$@" || rc=$?
    [ -n "$SETUP_LOG" ] && printf '  exit %s\n' "$rc" >> "$SETUP_LOG"
    return "$rc"
}

# build deps

LIBS="\
wayland-server|wayland|Wayland compositor core
wayland-client|wayland|Wayland client side
libinput|libinput|keyboard, mouse and touchpad
libudev|systemd-libs|device discovery and hotplug
xkbcommon|libxkbcommon|keyboard layouts
gbm|mesa|GPU buffer management
egl|mesa|OpenGL ES contexts
libseat|seatd|seat and session management
libdisplay-info|libdisplay-info|monitor identification
pangocairo|pango|text for on-screen UI
dbus-1|dbus|desktop integration
libpipewire-0.3|pipewire|screencasting
gtk4|gtk4|the settings app
libadwaita-1|libadwaita|the settings app"

MISSING_PKGS=""
N_OK=0
N_MISSING=0

add_missing() {
    local pkg
    for pkg in "$@"; do
        [ -n "$pkg" ] || continue
        case " $MISSING_PKGS " in *" $pkg "*) continue ;; esac
        MISSING_PKGS="$MISSING_PKGS $pkg"
    done
}

tool_row() {
    local label="$1" pkg="$2" cmd
    shift 2
    for cmd in "$@"; do
        if have "$cmd"; then
            row_ok "$label" ""
            N_OK=$((N_OK + 1))
            return 0
        fi
    done
    row_miss "$label" "$pkg"
    add_missing "$pkg"
    N_MISSING=$((N_MISSING + 1))
}

check_build() {
    printf '\n    %sbuild tools%s\n' "$C_BOLD" "$C_RESET"
    tool_row "C compiler" base-devel cc gcc
    tool_row "pkg-config" pkgconf pkg-config pkgconf
    tool_row "clang" clang clang
    tool_row "git" git git
    tool_row "curl" curl curl

    local PATH_SAVE="$PATH"
    [ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"
    if have cargo; then
        local rust_ver rust_major rust_minor
        rust_ver="$(cargo --version 2>/dev/null | awk '{print $2}' || true)"
        rust_major="${rust_ver%%.*}"
        rust_minor="${rust_ver#*.}"; rust_minor="${rust_minor%%.*}"
        if [ "${rust_major:-0}" -gt 1 ] 2>/dev/null \
            || { [ "${rust_major:-0}" -eq 1 ] && [ "${rust_minor:-0}" -ge 87 ]; } 2>/dev/null; then
            row_ok "Rust" "$rust_ver"
            N_OK=$((N_OK + 1))
        else
            row_miss "Rust" "$rust_ver is too old, ZEN needs 1.87 or newer"
            N_MISSING=$((N_MISSING + 1))
        fi
    else
        row_miss "Rust" "installed with rustup into ~/.cargo, no root needed"
        N_MISSING=$((N_MISSING + 1))
    fi
    PATH="$PATH_SAVE"

    printf '\n    %slibraries%s\n' "$C_BOLD" "$C_RESET"
    local lib pkg desc
    while IFS='|' read -r lib pkg desc; do
        [ -n "$lib" ] || continue
        if { have pkg-config || have pkgconf; } && pkg-config --exists "$lib" 2>/dev/null; then
            row_ok "$lib" "$(pkg-config --modversion "$lib" 2>/dev/null || true)"
            N_OK=$((N_OK + 1))
        else
            row_miss "$lib" "$desc"
            add_missing "$pkg"
            N_MISSING=$((N_MISSING + 1))
        fi
    done <<EOF
$LIBS
EOF
}

install_build_deps() {
    MISSING_PKGS="${MISSING_PKGS# }"
    step "Build dependencies"
    if [ -z "$MISSING_PKGS" ]; then
        ok "everything already present"
        return 0
    fi
    info "via pacman:"
    dim "$MISSING_PKGS"
    if ! confirm; then
        [ "$DO_UPDATE" = 1 ] || die "aborted"
        dim "skipped; ZEN will still be rebuilt"
        return 0
    fi
    sync_system
    local y=""; [ "$ASSUME_YES" = 1 ] && y=1
    # shellcheck disable=SC2086
    run $SUDO pacman -S --needed ${y:+--noconfirm} $MISSING_PKGS \
        || warn "pacman reported a problem; carrying on"
}

# manifest

MANIFEST="resources/packages.list"
declare -gA M_CAT M_PROF M_PROBE M_PAC M_AUR M_FLAT M_WHY M_ON M_NEW
M_IDS=()

PICK_CATS="system apps media office creative gaming dev fun"
ALWAYS_CATS="core desktop shell"
PROFILE=recommended

trim_into() {
    local -n trim_out="$1"
    local s="$2"
    s="${s#"${s%%[![:space:]]*}"}"
    s="${s%"${s##*[![:space:]]}"}"
    trim_out="$s"
}

load_manifest() {
    local file="${1:-$MANIFEST}"
    [ -f "$file" ] || die "$file is missing; run this from the root of the ZEN repository"
    M_IDS=()
    M_CAT=(); M_PROF=(); M_PROBE=(); M_PAC=(); M_AUR=(); M_FLAT=(); M_WHY=()
    M_ON=(); M_NEW=()

    local line id cat prof probe pac aur flat why
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in ''|'#'*) continue ;; esac
        IFS='|' read -r id cat prof probe pac aur flat why <<<"$line"
        trim_into id "$id"
        [ -n "$id" ] || continue
        trim_into cat "$cat"; trim_into prof "$prof"; trim_into probe "$probe"
        trim_into pac "$pac"; trim_into aur "$aur"; trim_into flat "$flat"
        trim_into why "$why"
        case "$pac" in '') pac="$id" ;; -) pac="" ;; esac

        M_IDS+=("$id")
        M_CAT[$id]="$cat"
        M_PROF[$id]="$prof"
        M_PROBE[$id]="${probe:-$id}"
        M_PAC[$id]="$pac"
        M_AUR[$id]="$aur"
        M_FLAT[$id]="$flat"
        M_WHY[$id]="$why"
        M_ON[$id]=0
    done < "$file"
}

# probe

FLATPAK_APPS=""
FLATPAK_LISTED=0

flatpak_apps() {
    if [ "$FLATPAK_LISTED" = 0 ]; then
        FLATPAK_APPS=""
        have flatpak && FLATPAK_APPS="$(flatpak list --app --columns=application 2>/dev/null || true)"
        FLATPAK_LISTED=1
    fi
    printf '%s\n' "$FLATPAK_APPS"
}

present() {
    local id="$1" tok f app
    local -a toks
    IFS=, read -ra toks <<<"${M_PROBE[$id]:-$id}"
    for tok in "${toks[@]}"; do
        trim_into tok "$tok"
        case "$tok" in
            '~/'*|/*)
                tok="${tok/#\~/$HOME}"
                for f in $tok; do
                    [ -e "$f" ] && return 0
                done ;;
            flatpak:*)
                while IFS= read -r app; do
                    [ -n "$app" ] || continue
                    # shellcheck disable=SC2053
                    [[ $app == ${tok#flatpak:} ]] && return 0
                done < <(flatpak_apps)
                ;;
            *)
                have "$tok" && return 0 ;;
        esac
    done
    return 1
}

# selection

profile_letter() {
    case "$1" in
        essentials) printf e ;;
        everything) printf x ;;
        *)          printf r ;;
    esac
}

is_pickable() {
    case " $PICK_CATS " in *" ${M_CAT[$1]} "*) return 0 ;; esac
    return 1
}

is_always() {
    case " $ALWAYS_CATS " in *" ${M_CAT[$1]} "*) return 0 ;; esac
    return 1
}

apply_profile() {
    local letter id
    letter="$(profile_letter "$PROFILE")"
    for id in "${M_IDS[@]}"; do
        if [ "$CLASSIC" = 1 ] && [ "${M_CAT[$id]}" = shell ]; then
            M_ON[$id]=0
        elif [ "$CLASSIC" = 1 ] && [ "${M_CAT[$id]}" = classic ]; then
            M_ON[$id]=1
        elif is_always "$id"; then
            M_ON[$id]=1
        elif is_pickable "$id"; then
            case "${M_PROF[$id]}" in
                *"$letter"*) M_ON[$id]=1 ;;
                *)           M_ON[$id]=0 ;;
            esac
        else
            M_ON[$id]=0
        fi
    done
}

CHOICES="${XDG_CONFIG_HOME:-$HOME/.config}/zen/setup-choices"

save_choices() {
    local file="${1:-$CHOICES}" id
    mkdir -p "$(dirname "$file")" 2>/dev/null || return 0
    {
        printf 'profile=%s\n' "$PROFILE"
        printf 'classic=%s\n' "$CLASSIC"
        for id in "${M_IDS[@]}"; do
            is_pickable "$id" && printf '%s=%s\n' "$id" "${M_ON[$id]}"
        done
    } > "$file" 2>/dev/null || true
}

load_choices() {
    local file="${1:-$CHOICES}" key value id
    [ -f "$file" ] || return 1
    local -A seen=()

    PROFILE="$(sed -n 's/^profile=//p' "$file" | head -1)"
    if [ "$CLASSIC" = 0 ]; then
        CLASSIC="$(sed -n 's/^classic=//p' "$file" | head -1)"
        [ "$CLASSIC" = 1 ] || CLASSIC=0
    fi
    case "$PROFILE" in essentials|recommended|everything) ;; *) PROFILE=recommended ;; esac
    apply_profile

    while IFS='=' read -r key value; do
        [ -n "$key" ] && [ "$key" != profile ] && [ "$key" != classic ] || continue
        [ -n "${M_CAT[$key]+x}" ] || continue
        is_pickable "$key" || continue
        case "$value" in 1) M_ON[$key]=1 ;; *) M_ON[$key]=0 ;; esac
        seen[$key]=1
    done < "$file"

    for id in "${M_IDS[@]}"; do
        if is_pickable "$id" && [ -z "${seen[$id]+x}" ]; then
            M_NEW[$id]=1
        fi
    done
    return 0
}

keep_what_you_have() {
    local id
    PROFILE=recommended
    apply_profile
    for id in "${M_IDS[@]}"; do
        is_pickable "$id" || continue
        if present "$id"; then M_ON[$id]=1; else M_ON[$id]=0; M_NEW[$id]=1; fi
    done
    MIGRATED=1
}

selected_ids() {
    local id out=""
    for id in "${M_IDS[@]}"; do
        [ "${M_ON[$id]:-0}" = 1 ] && out="$out $id"
    done
    printf '%s' "${out# }"
}

missing_ids() {
    local id out=""
    for id in "$@"; do
        present "$id" || out="$out $id"
    done
    printf '%s' "${out# }"
}

# gpu

nvidia_device() {
    local d vendor class
    for d in /sys/bus/pci/devices/*; do
        [ -r "$d/vendor" ] || continue
        vendor="$(cat "$d/vendor" 2>/dev/null || true)"
        class="$(cat "$d/class" 2>/dev/null || true)"
        if [ "$vendor" = 0x10de ]; then
            case "$class" in 0x03*) cat "$d/device" 2>/dev/null; return 0 ;; esac
        fi
    done
    return 1
}

nvidia_driver_id() {
    local dev
    dev="$(nvidia_device)" || return 1
    if [ $((dev)) -ge $((0x1e00)) ] 2>/dev/null; then
        printf nvidia
    else
        printf nvidia-legacy
    fi
}

kernel_headers() {
    local k out=""
    for k in $(pacman -Qq 2>/dev/null | grep -E '^linux(-[a-z0-9]+)?$' || true); do
        case "$k" in linux-firmware|linux-api-headers) continue ;; esac
        out="$out $k-headers"
    done
    printf '%s' "${out# }"
}

# check

check_manifest() {
    local cat id label shown
    for cat in $ALWAYS_CATS classic $PICK_CATS greeter gpu; do
        shown=0
        for id in "${M_IDS[@]}"; do
            [ "${M_CAT[$id]}" = "$cat" ] && [ "${M_ON[$id]}" = 1 ] || continue
            if [ "$shown" = 0 ]; then
                label="$(cat_label "$cat")"
                printf '\n    %s%s%s\n' "$C_BOLD" "$label" "$C_RESET"
                shown=1
            fi
            if present "$id"; then
                row_ok "$id" ""
                N_OK=$((N_OK + 1))
            else
                row_miss "$id" "${M_WHY[$id]}"
                N_MISSING=$((N_MISSING + 1))
            fi
        done
    done
}

cat_label() {
    case "$1" in
        core)     printf 'what ZEN needs' ;;
        shell)    printf 'ZEN Shell' ;;
        classic)  printf 'the classic desktop' ;;
        desktop)  printf 'what the keybinds open' ;;
        system)   printf 'System' ;;
        apps)     printf 'Apps' ;;
        media)    printf 'Media and recording' ;;
        office)   printf 'Office' ;;
        creative) printf 'Creative' ;;
        gaming)   printf 'Gaming' ;;
        dev)      printf 'Development' ;;
        fun)      printf 'Fun, from Nyarch and friends' ;;
        greeter)  printf 'login screen' ;;
        gpu)      printf 'graphics driver' ;;
        *)        printf '%s' "$1" ;;
    esac
}

check_deps() {
    step "Checking what you have"
    info "$DISTRO_NAME"
    check_build
    check_manifest
    check_screencast
    check_system

    printf '\n'
    if [ "$N_MISSING" -eq 0 ]; then
        printf '    %sall %s checks passed.%s\n' "$C_GREEN" "$N_OK" "$C_RESET"
    else
        printf '    %s%s present, %s missing.%s\n' "$C_BOLD" "$N_OK" "$N_MISSING" "$C_RESET"
    fi
}

check_screencast() {
    printf '\n    %sscreen sharing%s\n' "$C_BOLD" "$C_RESET"
    local zen_bin="$PREFIX/bin/zen"
    [ -x "$zen_bin" ] || zen_bin="target/$BUILD_PROFILE/zen"

    if [ -x "$zen_bin" ] && have strings; then
        if (set +o pipefail; strings -a "$zen_bin" 2>/dev/null \
                | grep -q "org.gnome.Mutter.ScreenCast"); then
            row_ok "screencast build" ""
            N_OK=$((N_OK + 1))
        else
            row_miss "screencast build" "this zen was built without xdp-gnome-screencast"
            N_MISSING=$((N_MISSING + 1))
        fi
    fi

    if have pgrep && pgrep -x pipewire >/dev/null 2>&1; then
        row_ok "pipewire running" ""
        N_OK=$((N_OK + 1))
    elif have pipewire; then
        row_miss "pipewire running" "installed but not started: systemctl --user start pipewire"
        N_MISSING=$((N_MISSING + 1))
    fi

    if [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && have busctl; then
        if busctl --user status org.gnome.Mutter.ScreenCast >/dev/null 2>&1; then
            row_ok "screencast service" "zen is answering on the bus"
            N_OK=$((N_OK + 1))
        else
            row_miss "screencast service" "zen is not answering org.gnome.Mutter.ScreenCast"
            N_MISSING=$((N_MISSING + 1))
        fi
    fi

    if [ -n "${XDG_CURRENT_DESKTOP:-}" ]; then
        case "$XDG_CURRENT_DESKTOP" in
            *zen*) row_ok "desktop name" "$XDG_CURRENT_DESKTOP"
                   N_OK=$((N_OK + 1)) ;;
            *) row_miss "desktop name" "XDG_CURRENT_DESKTOP is $XDG_CURRENT_DESKTOP, so the portal reads the wrong config"
               N_MISSING=$((N_MISSING + 1)) ;;
        esac
    fi

    local routing="" candidate
    for candidate in \
        "$PREFIX/share/xdg-desktop-portal/zen-portals.conf" \
        "/usr/local/share/xdg-desktop-portal/zen-portals.conf" \
        "/usr/share/xdg-desktop-portal/zen-portals.conf" \
        "/etc/xdg-desktop-portal/zen-portals.conf"
    do
        [ -f "$candidate" ] && { routing="$candidate"; break; }
    done

    if [ -n "$routing" ]; then
        row_ok "portal routing" "$routing"
        N_OK=$((N_OK + 1))
    else
        row_miss "portal routing" "no zen-portals.conf yet; installing ZEN puts it there"
        N_MISSING=$((N_MISSING + 1))
    fi

    if [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && have busctl; then
        local version
        version=$(busctl --user get-property org.freedesktop.portal.Desktop \
                      /org/freedesktop/portal/desktop \
                      org.freedesktop.portal.ScreenCast version 2>/dev/null \
                  | awk '{print $2}' || true)
        if [ -n "$version" ] && [ "$version" != "0" ]; then
            row_ok "portal screencast" "version $version"
            N_OK=$((N_OK + 1))
        else
            row_miss "portal screencast" "the portal offers no ScreenCast; OBS will show nothing"
            N_MISSING=$((N_MISSING + 1))
        fi
    fi
}

check_system() {
    printf '\n    %ssystem%s\n' "$C_BOLD" "$C_RESET"

    local cards
    cards="$(ls -d /dev/dri/card* 2>/dev/null | tr '\n' ' ' || true)"
    if [ -n "$cards" ]; then
        row_ok "GPU" "$cards"
        N_OK=$((N_OK + 1))
    else
        row_miss "GPU" "no /dev/dri/card*; ZEN can run nested but not on a TTY"
        N_MISSING=$((N_MISSING + 1))
    fi

    if [ -d /run/systemd/system ]; then
        row_ok "seat" "systemd-logind"
        N_OK=$((N_OK + 1))
    elif have seatd; then
        if pgrep -x seatd >/dev/null 2>&1; then
            row_ok "seat" "seatd running"
            N_OK=$((N_OK + 1))
        else
            row_miss "seat" "seatd is installed but not running"
            N_MISSING=$((N_MISSING + 1))
        fi
    else
        row_miss "seat" "no logind and no seatd; nothing can hand ZEN the GPU"
        add_missing seatd
        N_MISSING=$((N_MISSING + 1))
    fi

    local driver
    if driver="$(nvidia_driver_id)"; then
        if present "$driver"; then
            row_ok "NVIDIA driver" ""
            N_OK=$((N_OK + 1))
        else
            row_miss "NVIDIA driver" "an NVIDIA card with no NVIDIA driver; setup can install it"
            N_MISSING=$((N_MISSING + 1))
        fi
    fi
}

# routes

AUR_HELPER=""
FAILED_IDS=""

SYNCED=0

sync_system() {
    [ "$SYNCED" = 1 ] && return 0
    SYNCED=1
    step "Bringing Arch up to date"
    dim "Arch expects the system to be current before anything new is installed"
    need_root
    local y=""; [ "$ASSUME_YES" = 1 ] && y=1
    run $SUDO pacman -Syu ${y:+--noconfirm} || warn "the system update did not finish; carrying on"
}

pkg_available() {
    local pkg
    for pkg in "$@"; do
        pacman -Si -- "$pkg" >/dev/null 2>&1 || return 1
    done
    return 0
}

ensure_aur_helper() {
    [ -n "$AUR_HELPER" ] && return 0
    local helper
    for helper in paru yay; do
        if have "$helper"; then
            AUR_HELPER="$helper"
            return 0
        fi
    done

    step "Installing paru"
    dim "some of what you picked lives in the AUR, and paru is what builds it"
    if [ "$(id -u)" = 0 ]; then
        warn "the AUR cannot be built as root; run ./setup.sh as your own user"
        return 1
    fi
    need_root
    if ! run $SUDO pacman -S --needed --noconfirm base-devel git; then
        warn "base-devel and git did not install, so paru cannot be built"
        return 1
    fi

    local tmp rc=0
    tmp="$(mktemp -d)" || return 1
    run git clone --depth 1 https://aur.archlinux.org/paru-bin.git "$tmp/paru-bin" \
        && (cd "$tmp/paru-bin" && run makepkg -si --noconfirm) || rc=$?
    rm -rf "$tmp"

    if have paru; then
        AUR_HELPER=paru
        ok "paru installed"
        return 0
    fi
    warn "paru did not install (exit $rc)"
    return 1
}

ensure_flatpak() {
    if ! have flatpak; then
        need_root
        run $SUDO pacman -S --needed --noconfirm flatpak || return 1
    fi
    run flatpak remote-add --user --if-not-exists flathub \
        https://dl.flathub.org/repo/flathub.flatpakrepo || return 1
}

install_gh_bundle() {
    local spec="$1" repo asset tmp rc=0
    repo="${spec%/*}"
    asset="${spec##*/}"
    tmp="$(mktemp -d)" || return 1
    run curl -fL --retry 2 -o "$tmp/$asset" \
        "https://github.com/$repo/releases/latest/download/$asset" \
        && run flatpak install --user -y --noninteractive "$tmp/$asset" || rc=$?
    rm -rf "$tmp"
    return "$rc"
}

flatpak_shim() {
    local id="$1" ref="${M_FLAT[$1]}" cmd
    case "$ref" in ''|gh:*) return 0 ;; esac
    cmd="${M_PROBE[$id]%%,*}"
    case "$cmd" in ''|/*|'~'*|flatpak:*) return 0 ;; esac
    have "$cmd" && return 0
    mkdir -p "$HOME/.local/bin" || return 0
    printf '#!/bin/sh\nexec flatpak run %s "$@"\n' "$ref" > "$HOME/.local/bin/$cmd"
    chmod +x "$HOME/.local/bin/$cmd"
    dim "$cmd now opens the Flatpak $ref"
}

install_native() {
    local pkgs="$*" pkg
    [ -n "$pkgs" ] || return 0
    need_root
    # shellcheck disable=SC2086
    if run $SUDO pacman -S --needed --noconfirm $pkgs; then
        return 0
    fi
    warn "pacman refused the batch, so trying each package on its own"
    for pkg in $pkgs; do
        run $SUDO pacman -S --needed --noconfirm "$pkg" \
            || warn "$pkg did not install"
    done
}

install_ids() {
    local want="$*" id p native="" extra sync=0
    [ -n "$want" ] || return 0
    for id in $want; do
        [ -n "${M_PAC[$id]}${M_AUR[$id]}" ] && sync=1
    done
    if [ "$sync" = 1 ]; then sync_system; fi

    for id in $want; do
        p="${M_PAC[$id]}"
        [ -n "$p" ] && pkg_available $p && native="$native $p"
        if [ "$id" = nvidia ] || [ "$id" = nvidia-legacy ]; then
            extra="$(kernel_headers)"
            [ -n "$extra" ] && native="$native $extra"
        fi
    done
    # shellcheck disable=SC2086
    install_native ${native# }
    FLATPAK_LISTED=0

    local aur=""
    for id in $want; do
        present "$id" && continue
        p="${M_AUR[$id]}"
        if [ -z "$p" ] && [ -n "${M_PAC[$id]}" ] && ! pkg_available ${M_PAC[$id]}; then
            p="${M_PAC[$id]}"
        fi
        [ -n "$p" ] && aur="$aur $p"
    done
    if [ -n "${aur# }" ]; then
        if ensure_aur_helper; then
            # shellcheck disable=SC2086
            run "$AUR_HELPER" -S --needed --noconfirm ${aur# } \
                || warn "the AUR build did not finish"
        fi
    fi

    local flat=""
    for id in $want; do
        present "$id" && continue
        [ -n "${M_FLAT[$id]}" ] && flat="$flat $id"
    done
    if [ -n "${flat# }" ] && ensure_flatpak; then
        for id in $flat; do
            case "${M_FLAT[$id]}" in
                gh:*) install_gh_bundle "${M_FLAT[$id]#gh:}" || warn "$id did not install" ;;
                *)    if run flatpak install --user -y --noninteractive flathub "${M_FLAT[$id]}"; then
                          FLATPAK_LISTED=0
                          flatpak_shim "$id"
                      else
                          warn "$id did not install"
                      fi ;;
            esac
            FLATPAK_LISTED=0
        done
    fi

    FLATPAK_LISTED=0
    for id in $want; do
        if ! present "$id"; then
            case " $FAILED_IDS " in *" $id "*) ;; *) FAILED_IDS="$FAILED_IDS $id" ;; esac
        fi
    done
    FAILED_IDS="${FAILED_IDS# }"
}

report_failed() {
    if [ -z "$FAILED_IDS" ]; then
        ok "everything you picked is installed"
        return 0
    fi

    printf '\n'
    warn "these did not install:"
    local id
    for id in $FAILED_IDS; do
        printf '      %-20s %s\n' "$id" "${M_WHY[$id]}"
    done
    [ -n "$SETUP_LOG" ] && dim "every command and its result is in $SETUP_LOG"

    [ "$UI_TTY" = 1 ] && [ "$ANY_FLAG" = 0 ] || return 1
    ui_menu "Try those again?" "Yes, try again" "No, carry on without them" || return 1
    [ "$UI_CHOICE" = 0 ] || return 1

    local again="$FAILED_IDS"
    FAILED_IDS=""
    # shellcheck disable=SC2086
    install_ids $again
    report_failed
}

enable_services() {
    have systemctl || return 0
    [ -d /run/systemd/system ] || return 0

    if present networkmanager && ! systemctl is-enabled -q NetworkManager 2>/dev/null; then
        if systemctl is-active -q systemd-networkd iwd connman 2>/dev/null; then
            dim "another network manager is running, so NetworkManager stays off"
        else
            need_root
            run $SUDO systemctl enable --now NetworkManager || warn "NetworkManager did not start"
        fi
    fi

    if present bluetooth && compgen -G "/sys/class/bluetooth/*" >/dev/null \
        && ! systemctl is-enabled -q bluetooth 2>/dev/null; then
        need_root
        run $SUDO systemctl enable --now bluetooth || warn "bluetooth did not start"
    fi

    if present power-profiles && ! systemctl is-enabled -q power-profiles-daemon 2>/dev/null; then
        need_root
        run $SUDO systemctl enable --now power-profiles-daemon \
            || warn "power-profiles-daemon did not start"
    fi
}

install_selection() {
    step "Installing what you picked"
    local new="" id
    for id in "${M_IDS[@]}"; do
        [ "${M_NEW[$id]:-0}" = 1 ] && [ "${M_ON[$id]}" = 1 ] && new="$new $id"
    done
    [ -n "${new# }" ] && dim "new since your last install:${new}"
    if [ "$MIGRATED" = 1 ]; then
        dim "setup now lets you choose your apps; this update keeps exactly the ones you have"
        dim "run ${C_BOLD}./setup.sh${C_RESET} and pick Install to add more"
    fi

    local want
    # shellcheck disable=SC2046
    want="$(missing_ids $(selected_ids))"
    if [ -z "$want" ]; then
        ok "everything you picked is already installed"
    else
        info "$(printf '%s\n' $want | wc -l | tr -d ' ' || true) to install:"
        dim "$want"
        if confirm; then
            # shellcheck disable=SC2086
            install_ids $want
            report_failed || true
        else
            dim "skipped"
        fi
    fi

    enable_services
    save_choices
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

    [ "$CLASSIC" = 1 ] || build_shell
}

# shell

SHELL_BUILD="target/zen-shell"
SHELL_FONT_URL="https://raw.githubusercontent.com/google/fonts/main/ofl/googlesansflex/GoogleSansFlex%5BGRAD%2CROND%2Copsz%2Cslnt%2Cwdth%2Cwght%5D.ttf"
SHELL_FONT_NAME="GoogleSansFlex-VariableFont_GRAD,ROND,opsz,slnt,wdth,wght.ttf"

build_shell() {
    step "Building ZEN Shell"
    if ! have cmake || ! have ninja; then
        warn "cmake and ninja are missing, so ZEN Shell cannot be built"
        return 0
    fi

    local rev
    rev="$(git rev-parse --short HEAD 2>/dev/null || printf zen)"
    rm -rf "$SHELL_BUILD"
    if run cmake -GNinja -S zen-shell -B "$SHELL_BUILD" \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX="$PREFIX" \
            -DINSTALL_QSCONFDIR="$PREFIX/share/zen/shell" \
            -DVERSION=1.0.0 -DGIT_REVISION="$rev" >/dev/null \
        && run cmake --build "$SHELL_BUILD"; then
        ok "ZEN Shell built"
    else
        rm -rf "$SHELL_BUILD"
        warn "ZEN Shell did not build; ZEN will fall back to waybar and fuzzel"
        [ -n "$SETUP_LOG" ] && dim "the build output is above, and the commands are in $SETUP_LOG"
    fi
}

install_shell() {
    [ -d "$SHELL_BUILD" ] || return 0
    step "Installing ZEN Shell"
    run $SUDO cmake --install "$SHELL_BUILD" >/dev/null || {
        warn "ZEN Shell did not install"
        return 0
    }

    local fonts="$PREFIX/share/zen/shell/assets/google-sans-flex"
    if [ ! -f "$fonts/$SHELL_FONT_NAME" ]; then
        local tmp
        tmp="$(mktemp)" || return 0
        if run curl -fsSL --retry 2 -o "$tmp" "$SHELL_FONT_URL"; then
            $SUDO install -Dm644 "$tmp" "$fonts/$SHELL_FONT_NAME"
        else
            warn "could not download Google Sans Flex; the shell will use Rubik instead"
        fi
        rm -f "$tmp"
    fi
    rm -rf "$SHELL_BUILD"
    ok "ZEN Shell installed to $PREFIX/share/zen/shell"
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
    $SUDO install -Dm755 resources/zen-ly           "$PREFIX/bin/zen-ly"
    $SUDO install -Dm755 resources/zen-polkit       "$PREFIX/bin/zen-polkit"
    $SUDO install -Dm755 resources/zen-shell        "$PREFIX/bin/zen-shell"
    $SUDO install -Dm755 resources/zen-greeter      "$PREFIX/bin/zen-greeter"
    for part in greeter.qml Script.js zen.kdl; do
        $SUDO install -Dm644 "resources/greeter/$part" "$PREFIX/share/zen/greeter/$part"
    done
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
    for icon in resources/icons/*.svg; do
        [ -f "$icon" ] && $SUDO install -Dm644 "$icon"             "$PREFIX/share/icons/hicolor/scalable/apps/$(basename "$icon")"
    done

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
# Your config is a *copy* taken at install time, not a live view of the default, so
# options added later never appear in it. They fall back to their defaults, which is
# harmless, but you would never learn they exist. This is a hint, not a merge: your
# file is yours, and nothing here edits it.
# bind drift
# A rebind keeps the node name and changes what it does, so the node compare cannot see
# it and the line count says only that something moved. Someone whose Mod+F still runs
# the old action has no other way to find out.
# Only single-line binds are read. A chord is recognised by shape rather than by
# position, so a section header like `layout {` is never mistaken for one.
bind_pairs() {
    awk '
        /^[[:space:]]*\/\// { next }
        {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            if (line !~ /\{/ || line !~ /\}/) next

            chord = line
            sub(/[[:space:]].*$/, "", chord)
            sub(/\{.*$/, "", chord)
            if (chord == "") next

            is_chord = (chord ~ /\+/) ||
                       (chord ~ /^(Print|ModTap|XF86[A-Za-z]+)$/)
            if (!is_chord) next

            action = line
            sub(/^[^{]*\{[[:space:]]*/, "", action)
            sub(/[[:space:];].*$/, "", action)
            sub(/\}.*$/, "", action)
            if (action == "") next

            print chord, action
        }
    ' "$1" | sort -u
}

bind_drift() {
    local user="$1" shipped="$2"
    local tmp_ship tmp_user
    tmp_ship=$(mktemp) || return 0
    tmp_user=$(mktemp) || { rm -f "$tmp_ship"; return 0; }

    bind_pairs "$shipped" > "$tmp_ship"
    bind_pairs "$user"    > "$tmp_user"

    local reported=0 chord ship_action user_action added=""
    while read -r chord ship_action; do
        [ -n "$chord" ] || continue
        user_action=$(awk -v c="$chord" '$1 == c { print $2; exit }' "$tmp_user")

        if [ -z "$user_action" ]; then
            added="$added $chord"
            continue
        fi
        [ "$user_action" = "$ship_action" ] && continue

        if [ "$reported" -eq 0 ]; then
            printf '\n'
            warn "these binds do something different in the config ZEN now ships"
            reported=1
        fi
        printf '      %-20s yours: %-24s now: %s\n' "$chord" "$user_action" "$ship_action"
    done < "$tmp_ship"

    if [ -n "${added# }" ]; then
        printf '\n'
        warn "these binds are new and your config does not have them"
        dim "$(printf '%s' "${added# }" | cut -c1-240)"
        reported=1
    fi

    rm -f "$tmp_ship" "$tmp_user"
    [ "$reported" -eq 0 ] || dim "./setup.sh --reset-config takes the shipped config"
}

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
    missing=$(comm -23 "$tmp_ship" "$tmp_user" | tr '\n' ' ' || true)
    rm -f "$tmp_ship" "$tmp_user"

    missing="${missing%% }"

    # Missing node names are only the loud half. A changed *value* on a node you
    # already have is silent, and that is how a whole visual change goes missing.
    local changed
    changed=$(diff "$user" "$shipped" 2>/dev/null | grep -c '^[<>]' || true)
    [ -n "$missing" ] || [ "${changed:-0}" -gt 0 ] || return 0

    printf '\n'
    if [ -n "$missing" ]; then
        local count
        count=$(printf '%s\n' $missing | wc -l | tr -d ' ' || true)
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
    bind_drift "$user" "$shipped"
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
# Arrow-key menus, the way archinstall works. Deliberately hand-rolled ANSI
# rather than dialog/whiptail: this script's whole job is running on a machine
# where nothing is installed yet, so it cannot depend on a TUI toolkit being
# there. Everything below is bash builtins and escape codes.
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

# picker

pick_profile() {
    ui_menu "How much should setup install?" \
        "Recommended   ZEN plus everyday apps: browser, files, media, recording" \
        "Essentials    ZEN and what it needs to run, and no apps" \
        "Everything    adds office, creative, gaming, dev tools and the fun pack" \
        || return 1
    case "$UI_CHOICE" in
        0) PROFILE=recommended ;;
        1) PROFILE=essentials ;;
        2) PROFILE=everything ;;
    esac
    apply_profile
}

category_counts() {
    local cat="$1" id on=0 total=0
    for id in "${M_IDS[@]}"; do
        [ "${M_CAT[$id]}" = "$cat" ] || continue
        total=$((total + 1))
        [ "${M_ON[$id]}" = 1 ] && on=$((on + 1))
    done
    printf '%s of %s' "$on" "$total"
}

pick_category() {
    local cat="$1" id state i
    local -a ids=() opts=()
    for id in "${M_IDS[@]}"; do
        [ "${M_CAT[$id]}" = "$cat" ] || continue
        ids+=("$id")
        state=off
        [ "${M_ON[$id]}" = 1 ] && state=on
        opts+=("$(printf '%-20s %s' "$id" "${M_WHY[$id]//:/ }"):$state")
    done
    [ "${#ids[@]}" -gt 0 ] || return 0

    ui_multi "$(cat_label "$cat")" "${opts[@]}" || return 0
    for id in "${ids[@]}"; do M_ON[$id]=0; done
    for i in $UI_PICKED; do M_ON[${ids[$i]}]=1; done
}

pick_groups() {
    local -a cats opts
    local cat
    read -ra cats <<<"$PICK_CATS"
    while :; do
        opts=()
        for cat in "${cats[@]}"; do
            opts+=("$(printf '%-30s %s' "$(cat_label "$cat")" "$(category_counts "$cat")")")
        done
        opts+=("Done, install these")
        ui_menu "What to install ($PROFILE). Open a group to untick anything." "${opts[@]}" \
            || return 1
        [ "$UI_CHOICE" -ge "${#cats[@]}" ] && return 0
        pick_category "${cats[$UI_CHOICE]}"
    done
}

pick_nvidia() {
    local driver
    driver="$(nvidia_driver_id)" || return 0
    present "$driver" && return 0
    ui_menu "This machine has an NVIDIA card and no NVIDIA driver. Install it?" \
        "Yes, install the NVIDIA driver" \
        "No, keep what I have" || return 0
    [ "$UI_CHOICE" = 0 ] && M_ON[$driver]=1
    return 0
}

# reset
# Changing a bind in the shipped config does nothing for anyone who already has a
# config, because theirs is theirs. That is how Mod+F kept opening the old action and
# why a new bind never appeared. This replaces it, keeping the old one beside it.
reset_user_config() {
    step "Resetting your config"
    local dir="${XDG_CONFIG_HOME:-$HOME/.config}/zen"
    local dst="$dir/config.kdl"

    mkdir -p "$dir"

    if [ -f "$dst" ]; then
        if cmp -s resources/default-config.kdl "$dst"; then
            ok "already identical to the shipped config"
            return 0
        fi

        warn "this replaces $dst with the one ZEN ships"
        dim "everything you changed in it goes back to the default"
        confirm || { dim "left alone"; return 0; }

        local backup
        backup="$dst.$(date +%Y%m%d-%H%M%S).bak"
        cp "$dst" "$backup" || die "could not back up $dst"
        ok "kept your old one at $backup"
    fi

    cp resources/default-config.kdl "$dst" || die "could not write $dst"
    ok "wrote $dst"
    dim "ZEN reloads it live, so the new binds are active already"
}

write_user_config() {
    step "Writing your config"
    local dir="${XDG_CONFIG_HOME:-$HOME/.config}/zen"
    local dst="$dir/config.kdl"

    if [ -f "$dst" ]; then
        ok "config already exists at $dst, left alone"
    else
        mkdir -p "$dir"
        cp resources/default-config.kdl "$dst"
        ok "wrote $dst"
        dim "it is heavily commented, and reloads live while ZEN is running"
    fi
}

# designs
apply_designs() {
    seed_wallpapers
    theme_all
}

post_install_steps() {
    if [ "$RESET_CONFIG" = 1 ]; then reset_user_config; apply_designs; fi
    if [ "$W_CONFIG" = 1 ]; then write_user_config; fi
    if [ "$DO_INSTALL" = 1 ] || [ "$DO_UPDATE" = 1 ]; then migrate_to_shell; fi
    if [ "$W_CONFIG" = 1 ] || { [ "$DO_INSTALL" = 1 ] && [ "$DO_UPDATE" = 0 ]; }; then
        apply_designs
    fi
    if [ "$W_GREETER" = 1 ]; then install_greeter; fi
}

# Themes the app launcher, unless you already have a config of your own.
# fuzzel's stock look is a grey box that reads as an unstyled dialog on a dark
# canvas. This is the single cheapest thing that stops ZEN looking half-dressed.
theme_launcher() {
    local base="${XDG_CONFIG_HOME:-$HOME/.config}"
    theme_file "$base/fuzzel/fuzzel.ini" resources/fuzzel.ini "the app launcher"
}

# Themes the terminal, unless you already have a config of your own.
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
    theme_file "$base/waybar/zen-modules.jsonc" resources/waybar/zen-modules.jsonc "the bar modules"
    theme_file "$base/waybar/zen-pills.css" resources/waybar/zen-pills.css "the pill fill"
    migrate_waybar_include "$base/waybar/config.jsonc"
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
    have=$(sha256sum < "$dst" | cut -d' ' -f1 || true)
    grep -q "^$src $have\$" "$history"
}

# migration
# Module definitions moved into zen-modules.jsonc so that they keep updating even after
# the settings app has rewritten config.jsonc, which it does the moment anyone toggles a
# pill. A config written before that split has no include, so every new module ZEN ships
# is invisible to it forever. One line is added; nothing else is touched.
migrate_waybar_include() {
    local dst="$1"
    [ -f "$dst" ] || return 0
    grep -q '"include"' "$dst" && return 0
    grep -q '"custom/power"' "$dst" && return 0

    cp "$dst" "$dst.bak" || return 0
    if sed -i '0,/^[[:space:]]*{/s||{\n    "include": ["~/.config/waybar/zen-modules.jsonc"],|' "$dst"; then
        ok "added the module include to $dst (kept $dst.bak)"
    else
        mv "$dst.bak" "$dst"
        warn "could not add the module include to $dst; add it by hand"
    fi
}

# shell migration

SHELL_RULE='layer-rule {
    match namespace="^caelestia"
    background-effect {
        glass true
        blur true
        alpha-mask true
    }
}'

migrate_to_shell() {
    local dst="${1:-${XDG_CONFIG_HOME:-$HOME/.config}/zen/config.kdl}"
    [ -f "$dst" ] || return 0
    [ "$CLASSIC" = 1 ] && return 0

    local tmp
    tmp="$(mktemp)" || return 0
    sed -E \
        -e 's|^([[:space:]]*)spawn-at-startup "waybar"[[:space:]]*$|\1spawn-at-startup "zen-shell"|' \
        -e 's|\{ spawn "fuzzel"; \}|{ spawn "zen-shell" "launcher"; }|' \
        -e 's|\{ spawn-sh "pkill -SIGUSR1 -x waybar"; \}|{ spawn "zen-shell" "bar"; }|' \
        "$dst" > "$tmp"
    if ! grep -q 'namespace="^caelestia' "$tmp"; then
        printf '\n%s\n' "$SHELL_RULE" >> "$tmp"
    fi

    if cmp -s "$dst" "$tmp"; then
        rm -f "$tmp"
        return 0
    fi

    local backup
    backup="$dst.$(date +%Y%m%d-%H%M%S).bak"
    cp "$dst" "$backup" || { rm -f "$tmp"; return 0; }
    step "Moving your config to ZEN Shell"
    diff "$dst" "$tmp" | grep -E '^[<>]' | sed 's/^/      /' | head -12 || true
    cat "$tmp" > "$dst"
    rm -f "$tmp"
    ok "your old config is at $backup"
    dim "ZEN reloads it live; ./setup.sh --classic puts waybar back"
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
        ui_menu "Which login screen?" \
            "ZEN greeter  (glass, written welcome, runs on greetd)" \
            "Ly    (a small TTY greeter, no GTK, no Wayland session of its own)" \
            "greetd + ReGreet  (graphical, heavier, needs cage)" \
            "Skip" || return 0
        case "$UI_CHOICE" in
            0) GREETER=zen ;;
            1) GREETER=ly ;;
            2) GREETER=greetd ;;
            *) return 0 ;;
        esac
    fi

    case "$GREETER" in
        zen|ask) install_greeter_zen ;;
        ly) install_greeter_ly ;;
        greetd) install_greeter_greetd ;;
        none) return 0 ;;
    esac
}

# Ly reads /usr/share/wayland-sessions, which is where install_zen already puts
# zen.desktop, so there is nothing to configure: ZEN just appears in its list.
install_greeter_ly() {
    step "Installing the login screen"
    dim "Ly is a TTY greeter: it lists the sessions it finds and gets out of the way"

    remove_greetd_if_present

    present ly || install_ids ly
    if ! present ly; then
        warn "Ly did not install, so there is nothing to configure"
        [ -n "$SETUP_LOG" ] && dim "what pacman said is in $SETUP_LOG"
        return 1
    fi
    need_root

    theme_ly

    ok "Ly installed"
    greeter_epilogue "ly"
}

GREETD_DIR="${GREETD_DIR:-/etc/greetd}"
GREETER_CACHE="${GREETER_CACHE:-/var/cache/zen-greeter}"

zen_greetd_config() {
    printf '%s\n' \
        '[terminal]' \
        'vt = 1' \
        '' \
        '[default_session]' \
        "command = \"$PREFIX/bin/zen-greeter\"" \
        'user = "greeter"'
}

install_greeter_zen() {
    step "Installing the login screen"
    dim "greetd runs the login, ZEN draws it: the same glass as your desktop"

    present greetd || install_ids greetd
    if ! present greetd; then
        warn "greetd did not install, so there is nothing to configure"
        [ -n "$SETUP_LOG" ] && dim "what pacman said is in $SETUP_LOG"
        return 1
    fi
    if [ ! -x "$PREFIX/bin/zen-greeter" ] || [ ! -f "$PREFIX/share/zen/greeter/greeter.qml" ]; then
        warn "the ZEN greeter is not installed yet; run ./setup.sh --install first"
        return 1
    fi

    need_root
    if [ -f "$GREETD_DIR/config.toml" ] && ! grep -q zen-greeter "$GREETD_DIR/config.toml" 2>/dev/null; then
        $SUDO cp "$GREETD_DIR/config.toml" "$GREETD_DIR/config.toml.bak"
        dim "existing config saved as $GREETD_DIR/config.toml.bak"
    fi
    $SUDO mkdir -p "$GREETD_DIR"
    zen_greetd_config | $SUDO tee "$GREETD_DIR/config.toml" >/dev/null

    $SUDO mkdir -p "$GREETER_CACHE"
    if id greeter >/dev/null 2>&1; then
        $SUDO chown greeter "$GREETER_CACHE" 2>/dev/null || true
    fi

    if have systemctl && systemctl is-enabled ly@tty2.service >/dev/null 2>&1; then
        info "Ly is enabled too; turn it off once the ZEN greeter works:"
        info "  ${C_BOLD}sudo systemctl disable ly@tty2.service${C_RESET}"
    fi

    ok "greetd configured for the ZEN greeter"
    dim "try it first without logging out: ${C_BOLD}zen-greeter --preview${C_RESET}"
    greeter_epilogue "greetd"
}

install_greeter_greetd() {
    step "Installing the login screen"
    dim "greetd runs the session, ReGreet draws it, cage hosts it"

    # shellcheck disable=SC2046
    install_ids $(missing_ids greetd cage regreet)
    if ! present greetd || ! present cage || ! present regreet; then
        warn "greetd did not install, so there is nothing to configure"
        [ -n "$SETUP_LOG" ] && dim "what pacman said is in $SETUP_LOG"
        return 1
    fi

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
    local count
    # shellcheck disable=SC2046
    count="$(printf '%s\n' $(missing_ids $(selected_ids)) | grep -c . || true)"
    printf '\n  %sAbout to do this:%s\n\n' "$C_BOLD" "$C_RESET"
    [ "$DO_UPDATE"   = 1 ] && info "- pull the latest ZEN and show what changed"
    [ "$DO_DEPS"     = 1 ] && info "- install what is missing: $count package(s), $PROFILE"
    [ "$DO_BUILD"    = 1 ] && info "- build ZEN (the slow part, 5 to 15 minutes)"
    [ "$DO_INSTALL"  = 1 ] && info "- install ZEN to $PREFIX"
    [ "$W_CONFIG"    = 1 ] && info "- write your config file"
    [ "$W_GREETER"   = 1 ] && info "- install and set up the login screen ($GREETER)"
    printf '\n'
}

W_CONFIG=0
W_GREETER=0
GREETER=ask
SELECTION_SET=0

pick_greeter() {
    ui_menu "Set up a login screen?" \
        "ZEN greeter  glass, a written welcome, same look as the desktop" \
        "Ly     a small TTY greeter, nothing else to install" \
        "greetd + ReGreet  graphical, heavier, needs cage" \
        "No, I will start ZEN from a TTY" || return 1
    case "$UI_CHOICE" in
        0) W_GREETER=1; GREETER=zen ;;
        1) W_GREETER=1; GREETER=ly ;;
        2) W_GREETER=1; GREETER=greetd ;;
        *) W_GREETER=0 ;;
    esac
}

pick_apps() {
    pick_profile || return 1
    pick_groups || return 1
    pick_nvidia
    SELECTION_SET=1
}

wizard() {
    banner

    local installed=""
    have zen && installed=" (you have it already)"

    ui_menu "What would you like to do?" \
        "Install ZEN  (pick what comes with it next)" \
        "Update ZEN   (pull, rebuild, reinstall)$installed" \
        "Choose the steps yourself" \
        "Just check what is missing, change nothing" \
        "Quit" || return 1

    case "$UI_CHOICE" in
        0)  DO_DEPS=1; DO_BUILD=1; DO_INSTALL=1; W_CONFIG=1
            pick_apps || return 1
            pick_greeter || return 1
            ;;
        1)  DO_UPDATE=1; DO_DEPS=1; DO_BUILD=1; DO_INSTALL=1 ;;
        2)  ui_multi "Pick the steps  (space toggles)" \
                "Install what is missing:on" \
                "Pick apps and tools first:on" \
                "Build ZEN:on" \
                "Install ZEN system-wide:on" \
                "Write my config file:on" \
                "Install the login screen:off" \
                || return 1
            DO_DEPS=0; DO_BUILD=0; DO_INSTALL=0
            local idx picking=0
            for idx in $UI_PICKED; do
                case "$idx" in
                    0) DO_DEPS=1 ;;
                    1) picking=1 ;;
                    2) DO_BUILD=1 ;;
                    3) DO_INSTALL=1 ;;
                    4) W_CONFIG=1 ;;
                    5) W_GREETER=1 ;;
                esac
            done
            if [ "$picking" = 1 ]; then
                DO_DEPS=1
                pick_apps || return 1
            fi
            [ "$W_GREETER" = 1 ] && { pick_greeter || return 1; }
            ;;
        3)  CHECK_ONLY=1; return 0 ;;
        4)  return 1 ;;
    esac

    [ "$SELECTION_SET" = 1 ] || init_selection
    wizard_summary
    ui_menu "Go ahead?" "Yes, do it" "No, quit" || return 1
    [ "$UI_CHOICE" = 0 ] || return 1
    return 0
}

MIGRATED=0

init_selection() {
    case "$PRESET" in
        '')
            if ! load_choices; then
                if [ "$DO_UPDATE" = 1 ]; then
                    keep_what_you_have
                else
                    PROFILE=recommended
                    apply_profile
                fi
            fi ;;
        essentials|recommended|everything)
            PROFILE="$PRESET"; apply_profile ;;
        *)
            [ -f "$PRESET" ] || die "no preset called $PRESET, and no file by that name"
            load_choices "$PRESET" || die "could not read $PRESET" ;;
    esac
    SELECTION_SET=1
}

# ------------------------------------------------------------------ main ----

main() {
    { [ -f Cargo.toml ] && grep -q '^name = "zen"' Cargo.toml; } \
        || die "run this from the root of the ZEN repository"
    require_arch
    load_manifest

    # A bare `./setup.sh` on a real terminal gets the guided flow. Anything with a
    # flag, or piped into a script, keeps the old non-interactive behaviour.
    if [ "$ANY_FLAG" = 0 ] && [ "${ZEN_SETUP_REEXEC:-0}" = 1 ]; then
        DO_UPDATE=1; DO_DEPS=1; DO_BUILD=1; DO_INSTALL=1; ASSUME_YES=1
        banner
        printf '\n%sZEN setup, carrying on with the update%s\n' "$C_BOLD$C_BLUE" "$C_RESET"
    elif [ "$ANY_FLAG" = 0 ] && [ "$UI_TTY" = 1 ]; then
        wizard || { printf '\n%snothing done%s\n' "$C_DIM" "$C_RESET"; exit 0; }
        ASSUME_YES=1
    else
        banner
        printf '\n%sZEN setup%s\n' "$C_BOLD$C_BLUE" "$C_RESET"
    fi

    [ "$SELECTION_SET" = 1 ] || init_selection
    start_log

    if [ "$RESET_CONFIG" = 1 ] && [ "$DO_UPDATE" = 0 ]; then
        DO_DEPS=0; DO_BUILD=0; DO_INSTALL=0
    fi

    if [ "$CHECK_ONLY" = 1 ]; then
        check_deps
        printf '\n'
        local would
        # shellcheck disable=SC2046
        would="${MISSING_PKGS# } $(missing_ids $(selected_ids))"
        would="${would# }"
        if [ -n "${would% }" ]; then
            dim "would install: $would"
            info "run ${C_BOLD}./setup.sh${C_RESET} to install these and build"
        else
            info "nothing missing; ${C_BOLD}./setup.sh --build-only${C_RESET} builds"
        fi
        exit 0
    fi

    if [ "$DO_UPDATE" = 1 ]; then update_zen || exit 1; fi
    # An update runs the full audit too. A pull can add a bind that spawns something
    # new, or a library a new feature links against, and neither would ever be offered
    # otherwise: --update does not go through the install steps.
    if [ "$DO_UPDATE" = 1 ]; then apply_designs; fi
    if [ "$DO_DEPS" = 1 ]; then
        check_deps
        install_build_deps
        install_selection
    fi
    if [ "$DO_BUILD" = 1 ]; then ensure_rust; build; fi
    if [ "$DO_INSTALL" = 1 ]; then install_zen; install_shell; fi
    post_install_steps

    printf '\n%sdone%s\n' "$C_GREEN$C_BOLD" "$C_RESET"
    if [ "$DO_UPDATE" = 1 ]; then update_epilogue; fi
    if [ "$DO_BUILD" = 1 ] && [ "$DO_INSTALL" = 0 ]; then
        dim "binary at target/$BUILD_PROFILE/zen - ./setup.sh --install to install it"
    fi
}

[ "${ZEN_SETUP_LIB:-0}" = 1 ] || main "$@"
