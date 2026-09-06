<p align="center">
  <img alt="ZEN" src="resources/zen-thumbnail.png" width="560">
</p>

<p align="center">
  A spatial Wayland compositor built around an infinite 2D canvas,<br>
  where maximizing a window moves the camera instead of the window.
</p>

---

> [!WARNING]
> **ZEN is in early development.** It runs on real hardware and is being used
> daily on Arch, but it is young: expect rough edges, expect to read the config,
> and keep a TTY (`Ctrl+Alt+F2`) in reach. There are no releases yet.

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

- **The camera.** Pan, zoom about the pointer, frame a window, frame everything,
  return to origin. Panning is a velocity: hold longer and it winds up. Zoom is a
  spring that holds the point under your cursor still while it animates.
- **The canvas.** Windows live at absolute coordinates and stay where you put
  them. Drag one off the edge, pan away, come back, it is there.
- **Islands.** Windows group into islands that tile internally and move together.
  Navigation is a spatial search for the nearest island in a direction, not index
  arithmetic.
- **Camera-maximize.** `Mod+Shift+M` frames a window without resizing it. No
  configure is sent, so `stty size` does not change and you can pan away instantly.
- **The map.** Pull the camera back past `map-zoom` and islands draw as bubbles.
  It is a zoom level, not a mode, so there is no state to get stuck in.
- **Glass.** A real material: blurred and saturation-lifted backdrop, squircle
  corners, a specular rim, and refraction that bends the backdrop at the edge and
  leaves the middle alone.
- **A settings app** on `Mod+,` covering the material, windows, camera, input,
  monitors, keybinds, startup commands and wallpapers.
- **Wallpapers.** Drop images in a folder; that folder is the whole configuration.

## What does not work yet

- **Virtual monitors are experimental.** They can be created and destroyed at
  runtime and they render offscreen, but nothing consumes that texture yet and
  the per-window visibility mask is not built. Treat them as a foundation.
- **Canvas widgets** are not started.
- **The workspace grid** is still underneath the canvas. It is invisible now, but
  it has not been retired, and it is why some layout code has more cases than the
  model needs.
- **Only tested on one machine.** Multi-GPU, fractional scaling and unusual
  hardware are unexplored.

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
depth, the camera, focus behaviour, monitor resolution and arrangement, the
welcome animation, your keybinds, the commands that run at startup, and the
wallpaper reel. It edits
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

## Virtual monitors

A display with nothing behind it: no connector, no cable. It renders into a
texture, so it exists for the layout, the IPC and the settings app without
anything showing it.

```sh
zen msg output stream create --width 1920 --height 1080
zen msg outputs                      # it is there, like any other monitor
zen msg output stream destroy
```

Experimental, and honestly so: creating, destroying and rendering work, but
nothing consumes the texture yet, so there is no screencast of it and no way to
hide a window from your real screen while keeping it on this one. That mask is
the next piece.

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
