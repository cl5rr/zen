#!/usr/bin/env bash
#
#   bash tests/setup-probes.sh

set -u
cd "$(dirname "$0")/.." || exit 1

ZEN_SETUP_LIB=1
export ZEN_SETUP_LIB
# shellcheck source=/dev/null
. ./setup.sh
set +e

fail=0

check() {
    local what="$1" want="$2" probe="$3" got
    M_PROBE[probe]="$probe"
    FLATPAK_LISTED=0
    if present probe; then got=found; else got=absent; fi
    if [ "$got" = "$want" ]; then
        printf '  ok   %s\n' "$what"
    else
        printf '  FAIL %s: said %s, should be %s\n' "$what" "$got" "$want"
        fail=1
    fi
}

root=$(mktemp -d) || exit 1
stub=$(mktemp -d) || exit 1
trap 'rm -rf "$root" "$stub"' EXIT

mkdir -p "$root/lib/polkit-gnome" "$HOME/.zen-probe-test-$$"
: > "$root/lib/xdg-desktop-portal"
: > "$root/lib/polkit-gnome/polkit-gnome-authentication-agent-1"
: > "$root/lib/SymbolsNerdFont-Regular.ttf"
: > "$HOME/.zen-probe-test-$$/here"

printf '#!/bin/sh\n' > "$stub/awww"
printf '#!/bin/sh\n[ "$1" = list ] && printf "org.mozilla.firefox\\nmoe.nyarchlinux.catgirldownloader\\n"\n' > "$stub/flatpak"
chmod +x "$stub/awww" "$stub/flatpak"
PATH="$stub:$PATH"

printf 'setup.sh must not call an installed thing missing\n\n'

check "a file after one that is missing"       found  "/nope/a,$root/lib/xdg-desktop-portal"
check "a file in a directory that is absent"   found  "/usr/libexec-nope/x,$root/lib/xdg-desktop-portal"
check "nothing at all"                         absent "/nope/a,/nope/b"
check "a glob"                                 found  "$root/lib/SymbolsNerdFont*"
check "a glob one level down"                  found  "$root/*/polkit-*/polkit-*-authentication-agent-1"
check "a glob that matches nothing"            absent "$root/lib/NoSuchFont*"
check "a path under your home"                 found  "~/.zen-probe-test-$$/here"
check "a command"                              found  "awww"
check "the second of two names"                found  "swww-nope,awww"
check "neither of two names"                   absent "swww-nope,awww-nope"
check "a Flatpak by id"                        found  "flatpak:org.mozilla.firefox"
check "a Flatpak by pattern"                   found  "flatpak:*atgirl*"
check "a Flatpak that is not installed"        absent "flatpak:org.example.Nope"
check "spaces around names"                    found  " swww-nope , awww "

rm -rf "$HOME/.zen-probe-test-$$"

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
