#!/usr/bin/env bash

set -u
cd "$(dirname "$0")/.." || exit 1

ZEN_SETUP_LIB=1
export ZEN_SETUP_LIB
# shellcheck source=/dev/null
. ./setup.sh
set +e

fail=0

check() {
    local what="$1" want="$2"
    shift 2
    local got
    if "$@" >/dev/null 2>&1; then got=found; else got=absent; fi
    if [ "$got" = "$want" ]; then
        printf '  ok   %s\n' "$what"
    else
        printf '  FAIL %s: said %s, should be %s\n' "$what" "$got" "$want"
        fail=1
    fi
}

root=$(mktemp -d) || exit 1

mkdir -p "$root/lib" "$root/lib/polkit-gnome"
: > "$root/lib/xdg-desktop-portal"
: > "$root/lib/xdg-desktop-portal-gnome"
: > "$root/lib/polkit-gnome/polkit-gnome-authentication-agent-1"

LIBEXEC_DIRS="$root/libexec $root/lib"

printf 'setup.sh must not call an installed thing missing\n\n'

check "a file next to a missing one"        found  any_file "$root/nothing" "$root/lib/xdg-desktop-portal"
check "an unmatched glob beside a real one" found  any_file "$root/nope/"* "$root/lib/xdg-desktop-portal"
check "nothing at all"                      absent any_file "$root/nothing" "$root/also-nothing"

check "the portal, in the second dir"       found  libexec_any xdg-desktop-portal
check "a backend, by glob"                  found  libexec_any 'xdg-desktop-portal-*'
check "the gnome backend, by name"          found  libexec_any xdg-desktop-portal-gnome
check "a backend that is not installed"     absent libexec_any xdg-desktop-portal-kde
check "a polkit agent one level down"       found  libexec_any 'polkit-*/polkit-*-authentication-agent-1'

stub=$(mktemp -d) || exit 1
trap 'rm -rf "$root" "$stub"' EXIT
printf '#!/bin/sh\n' > "$stub/awww"
chmod +x "$stub/awww"
PATH="$stub:$PATH"

check "swww, installed under its other name" found  have_any_cmd "swww,awww"
check "neither name installed"               absent have_any_cmd "swww-nope,awww-nope"

if printf '%s' "$BIND_APPS" | grep -q 'swww,awww:swww'; then
    printf '  ok   the wallpaper bind asks about both names\n'
else
    printf '  FAIL the wallpaper bind asks about one name only\n'
    fail=1
fi

check "xdg-desktop-portal as a runtime dep" found  runtime_present xdg-desktop-portal
check "a runtime dep that really is absent" absent runtime_present zen-no-such-program

offenders=$(awk '
    /^[[:space:]]*#/ { next }
    /[^[:alnum:]_-]ls / {
        rest = $0
        sub(/^.*[^[:alnum:]_-]ls /, "", rest)
        sub(/[[:space:]]*[0-9]*[<>|;)&].*$/, "", rest)
        n = 0
        split(rest, w, /[[:space:]]+/)
        for (i in w) if (w[i] != "" && substr(w[i], 1, 1) != "-") n++
        if (n > 1 || $0 ~ /\\$/) printf "%d: %s\n", NR, $0
    }
' setup.sh)

if [ -n "$offenders" ]; then
    printf '  FAIL a probe asks ls about more than one path:\n'
    printf '%s\n' "$offenders" | sed 's/^/       /'
    fail=1
else
    printf '  ok   no probe asks ls about two paths at once\n'
fi

printf '\n'
if [ "$fail" -eq 0 ]; then
    printf 'all good\n'
else
    printf 'setup.sh would offer to install something the machine already has\n'
fi
exit "$fail"
