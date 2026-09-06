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

## Keybinds

`Mod` is the Super/Windows key. Everything here is in
[`resources/default-config.kdl`](resources/default-config.kdl) and is yours to change.

### Camera

The camera is the point of ZEN, so these matter most.

| Keybind | Action |
|---|---|
| `Mod+Shift+M` | Camera-maximize: frame the window without resizing it |
| `Mod+Home`, `Mod+Ctrl+0` | Go home: back to origin (0, 0) at 1:1 |
| `Mod+Alt+0` | Frame every window. The way out of being lost |
| `Mod+Alt+arrows` | Pan the camera |
| `Mod+middle-drag` | Pan with the mouse |
| `Mod+Alt+wheel`, `Mod+Ctrl+±` | Zoom about the pointer |
| `Mod+O` | Overview |

### Windows

| Keybind | Action |
|---|---|
| `Mod+arrows`, `Mod+HJKL` | Move focus, nearest window in that direction |
| `Mod+Ctrl+arrows` | Move the window |
| `Mod+left-drag` | Move the window with the mouse |
| `Mod+right-drag` | Resize. Grab anywhere in the window, not just an edge |
| `Mod+R`, `Mod+Shift+R` | Cycle preset widths |
| `Mod+Ctrl+Shift+R` | Cycle preset heights |
| `Mod+F` | Fullscreen |
| `Mod+Q` | Close |
| `Mod+Shift+arrows` | Move focus to another monitor |

### Islands

Clusters that tile internally and move as one.

| Keybind | Action |
|---|---|
| `Mod+[`, `Mod+]` | Merge the window into the island left/right of it |
| `Mod+Shift+[`, `Mod+Shift+]` | Merge into the island above/below |
| `Mod+\` | Pull the window out into its own island |
| `Mod+Alt+HJKL` | Jump between islands, skipping their members |

### Apps and the desktop

| Keybind | Action |
|---|---|
| `Mod+Space`, or tap `Mod` | App launcher |
| `Mod+,` | Settings |
| `Mod+T` | Terminal |
| `Mod+W` | Browser |
| `Mod+Shift+W` | Pick a wallpaper |
| `Mod+Ctrl+W` | Next wallpaper |
| `Print` | Screenshot |
| `Mod+Shift+/` | Show every binding, live from your config |

## Settings

`Mod+,` opens a settings app: the glass material, window spacing, edges and
depth, the camera, the welcome animation, and the wallpaper reel. It edits
`~/.config/zen/config.kdl` directly and ZEN reloads as you drag a slider, so
you see the change on the windows behind it.

It refuses to write a config the compositor would reject, and it leaves the
rest of the file, comments included, exactly as you had it.

It needs `gtk4` and `libadwaita`. Without them ZEN builds and runs the same;
`./setup.sh` just skips the app.

## The mouse

Windows that draw their own decorations, like a terminal, let you drag their edges.
Windows that do not, like Firefox under `prefer-no-csd`, have no edges to grab, so
resizing them is the compositor's job:

| Gesture | Action |
|---|---|
| `Mod+left-drag` | Move a window |
| `Mod+right-drag` | Resize a window, from anywhere inside it |
| `Mod+middle-drag` | Pan the camera |
| `Mod+Alt+wheel` | Zoom about the pointer |

## Wallpapers

Drop images into `~/.config/zen/wallpapers`. That folder is the entire
configuration: anything in it shows up in the picker on `Mod+Shift+W`, and
`Mod+Ctrl+W` cycles. The choice survives a reboot.

`zen-wallpaper` also works from a shell:

```sh
zen-wallpaper            # pick one
zen-wallpaper next       # cycle
zen-wallpaper random
zen-wallpaper set ~/pictures/thing.jpg
```

Needs `swaybg`, which `./setup.sh` offers to install. A wallpaper ships with ZEN
and is seeded into that folder on first install.

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
