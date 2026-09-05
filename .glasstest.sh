#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$HOME/zen-target"
SRC=/root/zen-src
OUT=/mnt/c/Users/heybu/AppData/Local/Temp/claude/c--Users-heybu-Downloads-ZEN/55c05ee9-f911-4ee0-b3a3-8ee748e23f63/scratchpad
Z="$CARGO_TARGET_DIR/release/zen"

cd "$SRC" || exit 1
rm -f /tmp/zen.log
rm -rf "$HOME/Pictures/Screenshots"

MODE="${1:-glass}"
REFR="${2:-14}"
if [ "$MODE" = blur ]; then EFFECT=""; else EFFECT="        glass true"; fi

cat > /tmp/zen-glass.kdl <<EOF
prefer-no-csd

hotkey-overlay {
    skip-at-startup
}

welcome {
    off
}

layout {
    gaps 24
    background-color "#101218"
    border {
        off
    }
    focus-ring {
        off
    }
}

glass {
    opacity 0.15
    tint "#ffffff"
    refraction $REFR
    falloff 20
    squircle 4.5
    saturation 1.4
    specular 0.16
}

window-rule {
    geometry-corner-radius 16
    clip-to-geometry true
    background-effect {
$EFFECT
        blur true
    }
}
EOF

echo "=== validate ==="
if ! "$Z" validate --config /tmp/zen-glass.kdl 2>&1 | head -20; then :; fi
"$Z" validate --config /tmp/zen-glass.kdl >/dev/null 2>&1 || { echo "CONFIG INVALID, stopping"; exit 1; }
echo "config ok"

"$Z" --config /tmp/zen-glass.kdl > /tmp/zen.log 2>&1 &
ZENPID=$!

S=""
for _ in $(seq 1 60); do
  S=$(grep -oE '/run/user/[0-9]+/zen[^ ]*\.sock' /tmp/zen.log | head -1)
  [ -n "$S" ] && break
  sleep 0.5
done
export ZEN_SOCKET="$S"
WD=$(grep -oE 'listening on Wayland socket: [a-z0-9-]+' /tmp/zen.log | awk '{print $NF}')
echo "socket=$S wayland=$WD mode=$MODE"

sleep 2
WAYLAND_DISPLAY="$WD" swaybg --mode fill -i "$SRC/resources/default-wallpaper.jpg" >/dev/null 2>&1 &
sleep 2
# A background effect only shows through translucent pixels, so the client has to
# actually be see-through or we are just photographing an opaque rectangle.
WAYLAND_DISPLAY="$WD" foot -o colors.alpha=0.55 -o colors.background=101218 >/dev/null 2>&1 &
sleep 4

"$Z" msg action screenshot-screen
sleep 1
find "$HOME/Pictures/Screenshots" -name '*.png' -exec cp {} "$OUT/glass-$MODE.png" \; 2>/dev/null
ls -la "$OUT/glass-$MODE.png" 2>/dev/null || echo "NO SCREENSHOT"

echo "=== log, shader lines ==="
grep -iE "error|glass|shader" /tmp/zen.log | grep -viE "ZINK|EGL|xwayland|MESA" | head -6

kill $ZENPID 2>/dev/null
pkill swaybg 2>/dev/null
sleep 1
