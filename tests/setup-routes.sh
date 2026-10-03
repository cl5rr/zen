#!/usr/bin/env bash
#
#   bash tests/setup-routes.sh

set -u
cd "$(dirname "$0")/.." || exit 1

# shellcheck source=/dev/null
. tests/stubs.sh

fail=0
say_ok() { printf '  ok   %s\n' "$1"; }
say_no() { printf '  FAIL %s\n' "$1"; fail=1; }
expect() {
    local what="$1"
    shift
    if "$@"; then say_ok "$what"; else say_no "$what"; fi
}

# arch

printf 'setup.sh refuses anything that is not Arch\n\n'

out=$(
    ZEN_SETUP_LIB=1 NO_COLOR=1 bash -c '
        . ./setup.sh
        have() { [ "$1" = pacman ] && return 1; command -v "$1" >/dev/null 2>&1; }
        require_arch
        echo "still running"
    ' 2>&1
)
code=$?
expect "a machine with no pacman is turned away" [ "$code" -ne 0 ]
expect "and told why" grep -q "Arch Linux" <<<"$out"
expect "before anything else runs" bash -c '! grep -q "still running" <<<"$1"' _ "$out"
out=$(
    ZEN_SETUP_LIB=1 NO_COLOR=1 bash -c '
        . ./setup.sh
        have() { [ "$1" = pacman ] && return 0; command -v "$1" >/dev/null 2>&1; }
        require_arch && echo "let through"
    ' 2>&1
)
expect "a machine with pacman is let through" grep -q "let through" <<<"$out"

# routes

printf '\nevery package finds a way in\n\n'

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
PATH="$HOME/.local/bin:$STUB/bin:$PATH"
PKG_MGR=pacman
ASSUME_YES=1
UI_TTY=0
NO_COLOR=1

manifest="$STUB/packages.list"
cat > "$manifest" <<'EOF'
# id | category | profiles | probe | pacman | aur | flatpak | why
native   | apps   | rx | nat-cmd | nat-pkg | | | in the repos
fallback | apps   | rx | fb-cmd  | fb-pkg  | | | not in the repos, same name in the AUR
renamed  | apps   | rx | ren-cmd | -       | ren-aur | | AUR only, under another name
flat     | apps   | rx | flat-cmd,flatpak:org.test.Flat | - | | org.test.Flat | Flathub only
flatfail | apps   | rx | ff-cmd,flatpak:org.test.Fail | - | | org.test.Fail | Flathub refuses it
bundle   | fun    | x  | flatpak:*undle* | - | | gh:owner/repo/bundle.flatpak | a GitHub release bundle
nowhere  | apps   | rx | none-cmd | - | | | nowhere at all
stubborn | apps   | rx | stb-cmd | stb-pkg | | | in the repos but refuses to install
core     | core   | erx | core-cmd | core-pkg | | | always
EOF

stub_repo nat-pkg stb-pkg core-pkg base-devel git flatpak
stub_broken stb-pkg
stub_aur fb-pkg ren-aur
stub_flathub org.test.Flat
stub_provides nat-pkg nat-cmd
stub_provides fb-pkg fb-cmd
stub_provides ren-aur ren-cmd
stub_provides core-pkg core-cmd

load_manifest "$manifest"
FAILED_IDS=""
install_ids native fallback renamed flat flatfail bundle nowhere stubborn >/dev/null 2>&1

expect "from the repos"                         present native
expect "from the AUR, under the same name"      present fallback
expect "from the AUR, under its AUR name"       present renamed
expect "from Flathub"                           present flat
expect "from a GitHub release bundle"           present bundle
expect "paru was installed first"               stub_logged "git clone --depth 1 https://aur.archlinux.org/paru-bin.git"
expect "and built as the user with makepkg"     stub_logged "makepkg -si"
expect "one bad package did not sink the batch" grep -qx "pacman -S --needed --noconfirm nat-pkg" "$STUB/log"
expect "Flathub was added before installing"    stub_logged "flatpak remote-add --user --if-not-exists flathub"
expect "Arch was brought up to date first" [ "$(grep -m1 '^pacman -S' "$STUB/log")" = "pacman -Syu --noconfirm" ]
expect "a Flatpak that installed gets a command" [ -x "$HOME/.local/bin/flat-cmd" ]
expect "a Flatpak that failed gets no command" [ ! -e "$HOME/.local/bin/ff-cmd" ]
case " $FAILED_IDS " in *" flatfail "*) ff=1 ;; *) ff=0 ;; esac
expect "and is reported as failed" [ "$ff" = 1 ]

case " $FAILED_IDS " in *" nowhere "*) has_nowhere=1 ;; *) has_nowhere=0 ;; esac
case " $FAILED_IDS " in *" stubborn "*) has_stubborn=1 ;; *) has_stubborn=0 ;; esac
expect "the two that cannot install are named" [ "$has_nowhere$has_stubborn" = 11 ]
case " $FAILED_IDS " in
    *" native "*|*" flat "*|*" renamed "*) say_no "something that installed is reported as failed: '$FAILED_IDS'" ;;
    *) say_ok "nothing that installed is reported as failed" ;;
esac

report=$(report_failed 2>&1)
expect "the report fails when something is missing" [ $? -ne 0 ]
expect "and lists it with what it was for" grep -q "nowhere at all" <<<"$report"

# profiles

printf '\nprofiles and choices\n\n'

PROFILE=essentials; apply_profile
expect "essentials leaves the apps off" [ "${M_ON[native]}" = 0 ]
expect "essentials keeps core on" [ "${M_ON[core]}" = 1 ]

PROFILE=recommended; apply_profile
expect "recommended turns the apps on" [ "${M_ON[native]}" = 1 ]
expect "recommended leaves the fun pack off" [ "${M_ON[bundle]}" = 0 ]

PROFILE=everything; apply_profile
expect "everything turns the fun pack on" [ "${M_ON[bundle]}" = 1 ]

PROFILE=recommended; apply_profile
M_ON[native]=0
choices="$STUB/choices"
save_choices "$choices"

printf 'later    | apps | rx | later-cmd | later-pkg | | | added in a later version\n' >> "$manifest"
load_manifest "$manifest"
load_choices "$choices"

expect "an untick survives the next run" [ "${M_ON[native]}" = 0 ]
expect "the profile comes back too" [ "$PROFILE" = recommended ]
expect "something new arrives on by its profile" [ "${M_ON[later]}" = 1 ]
expect "and is marked new" [ "${M_NEW[later]:-0}" = 1 ]
expect "what was already chosen is not marked new" [ "${M_NEW[fallback]:-0}" = 0 ]

# shipped

printf '\nthe shipped manifest\n\n'

load_manifest
expect "it parses" [ "${#M_IDS[@]}" -gt 20 ]
bad=""
for id in "${M_IDS[@]}"; do
    case "${M_CAT[$id]}" in
        core|desktop|system|apps|media|office|creative|gaming|dev|fun|greeter|gpu) ;;
        *) bad="$bad $id" ;;
    esac
done
expect "every row has a known category${bad:+:$bad}" [ -z "$bad" ]
nowhere=""
for id in "${M_IDS[@]}"; do
    [ -n "${M_PAC[$id]}${M_AUR[$id]}${M_FLAT[$id]}" ] || nowhere="$nowhere $id"
done
expect "every row has somewhere to come from${nowhere:+:$nowhere}" [ -z "$nowhere" ]
expect "swww is asked for under its Arch name" [ "${M_PAC[awww]}" = awww ]
expect "and found under either name" grep -q 'awww,swww' <<<"${M_PROBE[awww]}"

stub_done
printf '\n'
if [ "$fail" -eq 0 ]; then printf 'all good\n'; else printf 'a route is broken\n'; fi
exit "$fail"
