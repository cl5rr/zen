<h1 align="center">
  <img alt="ZEN" src="resources/zen.png" width="140">
  <br>
  ZEN
</h1>
<p align="center">A spatial Wayland compositor built around an infinite 2D canvas.</p>

---

> [!WARNING]
> **ZEN is in early development.** The canvas, the camera and the island layout
> work and are tested, but this has only ever run nested under WSLg with software
> rendering - never on real hardware, a real GPU, or DRM/KMS. There are no
> releases. Do not expect to daily-drive it yet.

## The idea

Most window managers ask where a window should *go*. ZEN asks where you should
be *looking*.

Windows live at absolute positions on an unbounded 2D canvas, grouped into
**islands** - free-floating clusters that tile internally like a normal tiling
WM. You navigate by panning and zooming a camera over that space rather than by
switching workspaces.

The idea the rest of the design hangs on: **maximize is a camera operation, not
a window operation.**

When you maximize a window, ZEN does not resize it. The viewport animates until
that window fills the screen, and the window is never told anything happened -
no `xdg_toplevel.configure`, no relayout, no reflow, and nothing to "restore",
because nothing was disturbed. A maximized terminal's `stty size` is unchanged.
You can pan away from it mid-animation and it is exactly where it always was.

That one primitive also expresses focus, overview, zoom-to-island and - later -
per-output virtual monitors.

## What works today

- **The infinite canvas.** Absolute coordinates, bounded only at ±1e6 logical
  pixels. Drag a window off the edge of the screen and it stays there; pan and
  it is still there. `Mod+Alt+0` frames everything if you get lost.
- **The camera.** Per-output, so two monitors can sit at different zoom levels
  simultaneously. Zoom is applied about the pointer, springs rather than eases,
  hands velocity over when you interrupt it, and rubber-bands at its limits.
- **Camera-maximize.** `Mod+Shift+M`. As described above.
- **Crisp magnification.** At zoom > 1 clients are asked for more pixels via
  fractional scale, quantized to quarter steps and only once the camera settles,
  so magnified text is sharp rather than an upscaled 1x buffer.
- **Islands.** Clusters that tile internally and move as one. New windows open
  where you are looking; merge with `Mod+BracketLeft`/`Mod+BracketRight`, split
  out with `Mod+Backslash`, jump between clusters with `Mod+Alt+HJKL`.
- **Spatial navigation.** Direction keys do a real 2D nearest-neighbour search
  that prefers the window actually beside you over a nearer diagonal one.
- **A canvas clock.** Compositor-drawn, positioned in canvas space, re-rasterized
  at the camera's zoom so it stays crisp when magnified. Off by default.

## What does not work yet

- Real hardware. DRM/KMS, multi-monitor and GPU performance are unexercised.
- Virtual monitors - runtime headless outputs with a per-window visibility mask.
- Canvas widgets: relocating layer-shell surfaces into canvas space so any
  existing Wayland widget toolkit works unmodified.
- The glass material. The shader compiles and its configuration is settled, but
  nothing feeds it a backdrop yet, so it has never actually been seen.
- Tabbed islands. The geometry and hit-testing are correct, but nothing hides
  the members underneath the active one, so no action exposes them.

## Building

```sh
./setup.sh --check          # non-destructive: what is installed vs missing
./setup.sh -y               # install dependencies and build
```

Or by hand, on Arch:

```sh
sudo pacman -S --needed rust gcc clang pkgconf systemd-libs mesa libxkbcommon \
                        wayland libinput dbus seatd pipewire pango libdisplay-info
cargo build --release
```

`--no-default-features` drops dbus/systemd/screencast if you do not want them.

## Running

ZEN picks the winit backend automatically when `WAYLAND_DISPLAY` is set, so it
opens as a window inside an existing session - which is the right way to develop
against it:

```sh
zen --config resources/default-config.kdl
```

On a TTY it takes the DRM/KMS backend. `zen msg` drives it over IPC:

```sh
zen msg windows
zen msg action camera-maximize
```

Configuration is KDL with live reload, at `$XDG_CONFIG_HOME/zen/config.kdl`.
[`resources/default-config.kdl`](resources/default-config.kdl) is the annotated
reference and is written out on first run.

## Development

```sh
cargo test                  # layout invariants, IPC, client-facing regressions
cargo run -p zen-visual-tests   # shaders, against a real GLES renderer, no compositor
```

Two `window_opening` snapshot tests fail and have done since before ZEN
existed, so a run is green at *"N passed; 2 failed"*.

The highest-value way to check a change here is to look at it: run ZEN nested,
open a client, and take a screenshot with `zen msg action screenshot-screen`.
That has caught a render z-order bug, an early-return bug, a contrast problem
and two window-placement bugs, none of which any test would have found, because
the code was self-consistent and simply wrong.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE), and [NOTICE](NOTICE) for the
derivation and the copyright it carries.
