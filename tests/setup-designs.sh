#!/usr/bin/env bash
#
# The wallpapers and the waybar, launcher, terminal and lock screen themes used to be
# applied from inside write_user_config, after its early return for "you already have a
# config". ZEN writes that config file itself the first time it starts, so anyone who
# booted the session before finishing setup got no wallpaper, an unthemed bar and an
# unthemed launcher, for good, and nothing ever said why.
#
# Also here: a package step that reports success for a package that is not installed,
# which is how "Ly installed" printed above a machine with no Ly on it.
#
#   bash tests/setup-designs.sh

set -u
cd "$(dirname "$0")/.." || exit 1

ZEN_SETUP_LIB=1
export ZEN_SETUP_LIB
# shellcheck source=/dev/null
. ./setup.sh
set +e

fail=0

say_ok() { printf '  ok   %s\n' "$1"; }
say_no() { printf '  FAIL %s\n' "$1"; fail=1; }

is_there() {
    if [ -s "$2" ]; then
        say_ok "$1"
    else
        say_no "$1: nothing at $2"
    fi
}

home=$(mktemp -d) || exit 1
trap 'rm -rf "$home"' EXIT

XDG_CONFIG_HOME="$home/.config"
export XDG_CONFIG_HOME
NO_COLOR=1
export NO_COLOR

printf 'the design pass must not hang off whether config.kdl was just written\n\n'

# Exactly the case that broke: the config file is already there, because ZEN put it
# there the first time it ran.
mkdir -p "$XDG_CONFIG_HOME/zen"
cp resources/default-config.kdl "$XDG_CONFIG_HOME/zen/config.kdl"

# What a plain `./setup.sh --install` does after the build. Not apply_designs called
# by hand: the bug was never in that function, it was in nothing reaching it.
RESET_CONFIG=0
W_CONFIG=0
W_GREETER=0
DO_INSTALL=1
DO_UPDATE=0
post_install_steps >/dev/null 2>&1

is_there "the waybar config"   "$XDG_CONFIG_HOME/waybar/config.jsonc"
is_there "the waybar style"    "$XDG_CONFIG_HOME/waybar/style.css"
is_there "the bar modules"     "$XDG_CONFIG_HOME/waybar/zen-modules.jsonc"
is_there "the pill fill"       "$XDG_CONFIG_HOME/waybar/zen-pills.css"
is_there "the launcher theme"  "$XDG_CONFIG_HOME/fuzzel/fuzzel.ini"
is_there "the alacritty theme" "$XDG_CONFIG_HOME/alacritty/alacritty.toml"
is_there "the lock theme"      "$XDG_CONFIG_HOME/swaylock/config"

if [ -n "$(ls -A "$XDG_CONFIG_HOME/zen/wallpapers" 2>/dev/null)" ]; then
    say_ok "a wallpaper to start with"
else
    say_no "a wallpaper to start with: the folder is empty"
fi

# A config of your own is still yours.
printf 'mine\n' > "$XDG_CONFIG_HOME/fuzzel/fuzzel.ini"
apply_designs >/dev/null 2>&1
if [ "$(cat "$XDG_CONFIG_HOME/fuzzel/fuzzel.ini")" = "mine" ]; then
    say_ok "a config you edited is left alone"
else
    say_no "a config you edited is left alone: it was overwritten"
fi

# And writing a config file has to reach it too, without being the only thing that does.
rm -rf "$XDG_CONFIG_HOME/waybar" "$XDG_CONFIG_HOME/fuzzel"
DO_INSTALL=0
W_CONFIG=1
post_install_steps >/dev/null 2>&1
is_there "the waybar config, on the config path" "$XDG_CONFIG_HOME/waybar/config.jsonc"

# A package step that cannot install anything must say so rather than return success.
printf '\n'
PKG_MGR=pacman
out=$(pkg_install zen-no-such-package 2>&1)
verdict=$?

if [ "$verdict" -eq 0 ]; then
    say_no "a package that did not install reported success"
else
    say_ok "a package that did not install reports failure"
fi

if printf '%s\n' "$out" | grep -q "still not installed"; then
    say_ok "and names it"
else
    say_no "and names it: it did not"
fi

printf '\n'
if [ "$fail" -eq 0 ]; then
    printf 'all good\n'
else
    printf 'a fresh install would come up unthemed, with no wallpaper\n'
fi
exit "$fail"
