#!/bin/sh
for dir in /usr/share/wayland-sessions /usr/share/xsessions /usr/local/share/wayland-sessions; do
    for f in "$dir"/*.desktop; do
        [ -f "$f" ] || continue
        grep -qiE '^(NoDisplay|Hidden)=true' "$f" && continue
        name=$(grep -m1 '^Name=' "$f" | cut -d= -f2-)
        exec=$(grep -m1 '^Exec=' "$f" | cut -d= -f2-)
        desk=$(grep -m1 '^DesktopNames=' "$f" | cut -d= -f2- | cut -d';' -f1)
        [ -n "$name" ] && [ -n "$exec" ] || continue
        case "$dir" in *xsessions) kind=x11 ;; *) kind=wayland ;; esac
        printf '%s\t%s\t%s\t%s\n' "$kind" "$name" "$exec" "$desk"
    done
done
