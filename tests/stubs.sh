# stubs

stub_init() {
    STUB="$(mktemp -d)" || exit 1
    mkdir -p "$STUB/bin"
    : > "$STUB/log"
    : > "$STUB/repo"
    : > "$STUB/aur"
    : > "$STUB/broken"
    : > "$STUB/installed"
    : > "$STUB/provides"
    : > "$STUB/flathub"
    : > "$STUB/flatpaks"

    cat > "$STUB/bin/pacman" <<'EOF'
#!/bin/sh
S="${STUB:?}"
echo "pacman $*" >> "$S/log"
op="$1"; shift
case "$op" in
    -Si)
        [ "$1" = "--" ] && shift
        grep -qx -- "$1" "$S/repo" ;;
    -Qq)
        [ "$1" = "--" ] && shift
        if [ $# -eq 0 ]; then cat "$S/installed"; else grep -qx -- "$1" "$S/installed"; fi ;;
    -S)
        pkgs=""
        for a in "$@"; do
            case "$a" in -*) ;; *) pkgs="$pkgs $a" ;; esac
        done
        for p in $pkgs; do
            grep -qx -- "$p" "$S/repo" || exit 1
            grep -qx -- "$p" "$S/broken" && exit 1
        done
        for p in $pkgs; do
            echo "$p" >> "$S/installed"
            awk -v p="$p" '$1 == p { print $2 }' "$S/provides" | while read -r c; do
                printf '#!/bin/sh\n' > "$S/bin/$c"; chmod +x "$S/bin/$c"
            done
        done ;;
    *) exit 0 ;;
esac
EOF

    cat > "$STUB/bin/aurhelper" <<'EOF'
#!/bin/sh
S="${STUB:?}"
echo "$(basename "$0") $*" >> "$S/log"
[ "$1" = "-S" ] || exit 0
shift
for a in "$@"; do
    case "$a" in -*) continue ;; esac
    grep -qx -- "$a" "$S/aur" || exit 1
    echo "$a" >> "$S/installed"
    awk -v p="$a" '$1 == p { print $2 }' "$S/provides" | while read -r c; do
        printf '#!/bin/sh\n' > "$S/bin/$c"; chmod +x "$S/bin/$c"
    done
done
EOF

    cat > "$STUB/bin/flatpak" <<'EOF'
#!/bin/sh
S="${STUB:?}"
echo "flatpak $*" >> "$S/log"
case "$1" in
    list) cat "$S/flatpaks" ;;
    remote-add) exit 0 ;;
    install)
        last=""
        for a in "$@"; do last="$a"; done
        case "$last" in
            *.flatpak) echo "org.bundle.$(basename "$last" .flatpak)" >> "$S/flatpaks" ;;
            *) grep -qx -- "$last" "$S/flathub" || exit 1
               echo "$last" >> "$S/flatpaks" ;;
        esac ;;
    *) exit 0 ;;
esac
EOF

    cat > "$STUB/bin/sudo" <<'EOF'
#!/bin/sh
while [ $# -gt 0 ]; do
    case "$1" in -*) shift ;; *) break ;; esac
done
[ $# -gt 0 ] || exit 0
exec "$@"
EOF

    cat > "$STUB/bin/git" <<'EOF'
#!/bin/sh
echo "git $*" >> "${STUB:?}/log"
[ "$1" = clone ] || exit 0
for a in "$@"; do last="$a"; done
mkdir -p "$last"
EOF

    cat > "$STUB/bin/makepkg" <<'EOF'
#!/bin/sh
S="${STUB:?}"
echo "makepkg $*" >> "$S/log"
case "$(pwd)" in
    *paru-bin) cp "$S/bin/aurhelper" "$S/bin/paru"; chmod +x "$S/bin/paru" ;;
esac
EOF

    cat > "$STUB/bin/curl" <<'EOF'
#!/bin/sh
echo "curl $*" >> "${STUB:?}/log"
out=""
while [ $# -gt 0 ]; do
    case "$1" in -o) out="$2"; shift ;; esac
    shift
done
[ -n "$out" ] && : > "$out"
EOF

    chmod +x "$STUB/bin/"*
    export STUB
    PATH="$STUB/bin:$PATH"
    export PATH
}

stub_repo()     { printf '%s\n' "$@" >> "$STUB/repo"; }
stub_aur()      { printf '%s\n' "$@" >> "$STUB/aur"; }
stub_broken()   { printf '%s\n' "$@" >> "$STUB/broken"; }
stub_flathub()  { printf '%s\n' "$@" >> "$STUB/flathub"; }
stub_provides() { printf '%s %s\n' "$1" "$2" >> "$STUB/provides"; }
stub_logged()   { grep -q -- "$1" "$STUB/log"; }
stub_done()     { rm -rf "$STUB"; }
