#!/usr/bin/env bash
#
# setup.sh runs under `set -euo pipefail`, where a command substitution whose pipeline
# fails takes the whole script down. That is not a hypothetical: `strings | grep -q`,
# `diff | grep -c` and `busctl | awk` each locked a user out of installing and updating,
# and each looked like a missing file rather than a dead script.
#
# Every probe here is one that fails on a machine that does not have the thing being
# probed for, which is exactly when it runs. The check is simply that setup.sh still
# reaches its own last line.
#
#   bash tests/setup-survives.sh

set -u
cd "$(dirname "$0")/.." || exit 1

fail=0

run_check() {
    local what="$1"
    shift
    local out
    if out=$(env "$@" NO_COLOR=1 bash setup.sh --check 2>&1); then
        if printf '%s' "$out" | grep -q "run ./setup.sh"; then
            printf '  ok   %s\n' "$what"
            return 0
        fi
        printf '  FAIL %s: exited 0 but stopped early\n' "$what"
    else
        printf '  FAIL %s: setup.sh exited %s\n' "$what" "$?"
    fi
    printf '%s\n' "$out" | tail -3 | sed 's/^/       /'
    fail=1
}

printf 'setup.sh --check must survive a machine missing everything\n\n'

run_check "no session bus at all" DBUS_SESSION_BUS_ADDRESS=
run_check "a bus address that goes nowhere" \
    DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent-zen-test
run_check "no PREFIX binary to inspect" PREFIX=/nonexistent-zen-prefix
run_check "an unrecognised desktop" XDG_CURRENT_DESKTOP=SomethingElse
run_check "a PATH with almost nothing on it" PATH=/usr/bin:/bin

printf '\n'
if [ "$fail" -eq 0 ]; then
    printf 'all good\n'
else
    printf 'setup.sh aborts early under one of these, which locks out install and update\n'
fi
exit "$fail"
