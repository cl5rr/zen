#!/usr/bin/env bash

set -u
cd "$(dirname "$0")/.." || exit 1

# shellcheck source=tests/stubs.sh
. tests/stubs.sh

fail=0
say_ok() { printf '  ok   %s\n' "$1"; }
say_no() { printf '  FAIL %s\n' "$1"; fail=1; }

stub_init
HOME="$STUB/home"
XDG_CONFIG_HOME="$HOME/.config"
XDG_CACHE_HOME="$HOME/.cache"
mkdir -p "$HOME"
export HOME XDG_CONFIG_HOME XDG_CACHE_HOME
ZEN_SETUP_LIB=1
export ZEN_SETUP_LIB
# shellcheck source=/dev/null
. ./setup.sh
set +e
PATH="$STUB/bin:$PATH"
PKG_MGR=pacman
ASSUME_YES=1
UI_TTY=0
NO_COLOR=1
load_manifest

PREFIX="$STUB/prefix"
GREETD_DIR="$STUB/etc/greetd"
GREETER_CACHE="$STUB/cache/zen-greeter"
stub_repo greetd
stub_provides greetd greetd

# install

printf '\nthe ZEN greeter goes in through greetd\n\n'

install_greeter_zen >/dev/null 2>&1
if [ ! -f "$GREETD_DIR/config.toml" ]; then
    say_ok "nothing is configured while the greeter itself is missing"
else
    say_no "nothing is configured while the greeter itself is missing: config.toml was written"
fi

mkdir -p "$PREFIX/bin" "$PREFIX/share/zen/greeter" "$GREETD_DIR"
printf '#!/bin/sh\n' > "$PREFIX/bin/zen-greeter"
chmod +x "$PREFIX/bin/zen-greeter"
: > "$PREFIX/share/zen/greeter/greeter.qml"
printf '[default_session]\ncommand = "agreety --cmd sway"\n' > "$GREETD_DIR/config.toml"

GREETER=ask
install_greeter >/dev/null 2>&1
cfg="$GREETD_DIR/config.toml"

if stub_logged "pacman -S.*greetd"; then say_ok "greetd is installed when missing"; else say_no "greetd is installed when missing"; fi
if grep -qx "command = \"$PREFIX/bin/zen-greeter\"" "$cfg"; then
    say_ok "greetd starts zen-greeter by its full path"
else
    say_no "greetd starts zen-greeter by its full path: $(grep command "$cfg")"
fi
if grep -qx 'user = "greeter"' "$cfg"; then say_ok "it runs as the greeter user"; else say_no "it runs as the greeter user"; fi
if grep -qx 'vt = 1' "$cfg"; then say_ok "on VT 1"; else say_no "on VT 1"; fi
if grep -q agreety "$GREETD_DIR/config.toml.bak" 2>/dev/null; then
    say_ok "the old greetd config is kept"
else
    say_no "the old greetd config is kept"
fi
if [ -d "$GREETER_CACHE" ]; then say_ok "the last-user cache exists"; else say_no "the last-user cache exists"; fi

cp "$cfg" "$STUB/first"
install_greeter_zen >/dev/null 2>&1
if grep -q agreety "$GREETD_DIR/config.toml.bak"; then
    say_ok "running it again does not overwrite the backup with its own config"
else
    say_no "running it again does not overwrite the backup with its own config"
fi

# shipped

printf '\nwhat install_zen copies exists\n\n'

for part in $(sed -n 's/^    for part in \(.*\); do$/\1/p' setup.sh); do
    if [ -s "resources/greeter/$part" ]; then say_ok "resources/greeter/$part"; else say_no "resources/greeter/$part is missing"; fi
done
if [ -s resources/zen-greeter ]; then say_ok "resources/zen-greeter"; else say_no "resources/zen-greeter is missing"; fi
if grep -q 'namespace="^zen-greeter-glass\$"' resources/greeter/zen.kdl; then
    say_ok "the greeter's own config gives its letters glass"
else
    say_no "the greeter's own config gives its letters glass"
fi
if grep -q 'WlrLayershell.namespace: "zen-greeter-glass"' resources/greeter/greeter.qml; then
    say_ok "and the greeter uses that namespace"
else
    say_no "and the greeter uses that namespace"
fi

stub_done
printf '\n'
if [ "$fail" = 0 ]; then echo "all good"; else echo "something is wrong"; exit 1; fi
