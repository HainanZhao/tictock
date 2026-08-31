# tictock

`tictock` is a polished, configurable clock for your terminal with 18 distinctive faces. It is
written in Rust, ships as a single lightweight `tictock` binary, and idles at ~0% CPU.

```
      ██       ██████                    ██       ██                      ██     ██████
      ██       ██████                    ██       ██                      ██     ██████
    ████     ██      ██     ██         ████     ████         ██         ████   ██      ██
    ████     ██      ██     ██         ████     ████         ██         ████   ██      ██
      ██             ██     ██       ██  ██       ██         ██       ██  ██   ██      ██
      ██             ██     ██       ██  ██       ██         ██       ██  ██   ██      ██
      ██           ██              ██    ██       ██                ██    ██     ██████
      ██           ██              ██    ██       ██                ██    ██     ██████
      ██         ██         ██     ██████████     ██         ██     ██████████ ██      ██
      ██         ██         ██     ██████████     ██         ██     ██████████ ██      ██
      ██       ██           ██           ██       ██         ██           ██   ██      ██
      ██       ██           ██           ██       ██         ██           ██   ██      ██
    ██████   ██████████                  ██     ██████                    ██     ██████
    ██████   ██████████                  ██     ██████                    ██     ██████
                                   Friday, August 14 2026
```

## Faces

| Face      | Look                                                       |
|-----------|-------------------------------------------------------------|
| `digital` | Big blocky LED-style digits                                 |
| `analog`  | Round clock face with hands, drawn in braille sub-pixels     |
| `binary`  | Binary-coded-decimal dot grid, one column per digit          |
| `word`    | Natural language — "TWENTY PAST FOUR" in big letters          |
| `matrix`  | Sharp 7-segment digits drawn in braille sub-pixels           |
| `flip`    | Retro split-flap board — each digit on a card with a seam     |
| `waves`   | Sci-fi oscilloscope — flowing sine waves with a central time card |
| `rings`   | Concentric progress arcs, time in the middle                 |
| `roman`   | Roman numerals, stacked and oversized                        |
| `lcd`     | Thick seven-segment bars with solid corners                   |
| `hourglass` | Sand draining through a glass, once per hour                |
| `blocks`  | The whole day as a grid of blocks, one lit per interval passed |
| `cuckoo`  | Ornate cuckoo clock chalet silhouette with a ticking swinging pendulum |
| `radar`   | Aviation/marine radar screen with rotating sweep and target blips |
| `ship`    | Maritime ship steering wheel with an elegant analog dial in the center |
| `grid`    | Retro 3x5 block matrix digits rendered using solid square blocks ■ |
| `warp`    | Star Trek warp-speed time travel effect, zooming in to the center |
| `snake`   | A self-playing arcade board driven deterministically by wall-clock time |

Switch faces live with the Left/Right arrow keys, or press `Tab` for a
picker grid showing a live preview of every face at once.

Every face auto-scales to fill your terminal — the bigger the window, the
bigger the clock. Press `+`/`-` to override the size, `0` to go back to auto.

## Install

**Homebrew (macOS/Linux):**

```sh
brew install hainanzhao/tap/tictock
```

**Linux/macOS (prebuilt binary):**

```sh
curl -fsSL https://raw.githubusercontent.com/HainanZhao/tictock/main/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/HainanZhao/tictock/main/install.ps1 | iex
```

**From crates.io (any platform, requires Rust):**

```sh
cargo install tictock
```

**From the latest source:**

```sh
cargo install --git https://github.com/HainanZhao/tictock
```

## Usage

```sh
tictock                       # show the clock using your saved config
tictock --face analog         # try a face for this run only (doesn't save)
tictock --face digital --color green --no-date
```

While running:

| Key         | Action                                  |
|-------------|------------------------------------------|
| `q` / Esc   | Quit                                     |
| `←` / `→`   | Cycle to the previous / next face        |
| `Tab`       | Open a grid picker with a live preview of every face |
| `c`         | Cycle through beautiful, solid color presets |
| `t`         | Toggle 12h / 24h                         |
| `s`         | Toggle seconds                           |
| `+` / `-`   | Grow / shrink the clock                  |
| `0`         | Back to auto-fill sizing                 |

In the picker: arrow keys move the selection, `Enter` confirms, `Esc`
cancels.

Whatever you switch to during a session is remembered — face, 12/24h, seconds
and size are written back to the config on exit, so restarting picks up where
you left off. One-off `--flag` overrides are *not* saved.

## Configuring

Settings persist in a small TOML file so `tictock` always starts the way you
like it, without needing flags every time.

```sh
tictock config path              # print the config file location
tictock config show              # print the current config
tictock config set face analog   # persist a setting
tictock config set color "#33ccff"
tictock config colors            # list built-in color names
tictock config reset             # back to defaults
```

Config file (created on first `config set`, edit by hand too):

```toml
face = "digital"          # digital, analog, binary, word, matrix, flip, waves, rings, roman, lcd, hourglass, blocks, cuckoo, radar, ship, grid, warp, snake
hour12 = true             # 12h with am/pm, or false for 24h
show_seconds = true
show_date = true
blink_colon = true        # digital/matrix/flip: blink the ':' once a second
tick_marks = true         # analog: hour tick marks around the rim
second_step = 1           # 5 shows :00, :05, :10 ... instead of every second
ghost_segments = false    # lcd: show the unlit segments faintly, panel-style
scale = 0                 # 0 = auto-fill the terminal; 1-9 to pin a size
color = "#38d9e8"         # primary color
accent_color = "none"     # "none" for solid colors, or hex/color for gradients
```

Faces are drawn with a controlled gradient running from `color` to
`accent_color`. With the default `accent_color = "none"`, every face stays in
one coherent color family; set an explicit accent to enable a two-color theme.

Colors accept the standard ANSI names (`red`, `green`, `blue`, ...) or
`#rrggbb` for truecolor terminals. Run `tictock config colors` for the full
built-in list.

Every setting also has a matching `--flag` for one-off overrides — see
`tictock --help`.

## Rendering


Glyphs are not a scaled-up pixel grid. Each digit and letter is described as
vector strokes — straight segments and elliptical arcs of constant width with
round caps, in the spirit of a light geometric sans — so the letterforms stay
true at any size.

Strokes are rasterized at sub-cell resolution and drawn with quadrant block
characters. All sixteen combinations of a 2x2 split exist in Unicode, so every
terminal cell carries four sub-pixels; half-blocks alone would subdivide only
vertically and leave curves visibly stepped along the horizontal axis.

Edges are hard rather than anti-aliased: blending partial coverage toward the
background reads as a grey halo around the strokes rather than as smoothing.

The `lcd` face does not use that pipeline at all. A seven-segment number is
already discrete — three horizontal bars and four vertical ones on a small
integer grid — so there is nothing to rasterize. Its bars are laid out
directly in cells from integer thicknesses, which is exact at every size: no
partial coverage, no stray single-cell spikes, no gaps that round away to
nothing. Rasterizing outlines onto a grid ten cells wide is what produces
those artifacts, and no amount of snapping or chamfering fixes the mismatch.

## Why it's lightweight

`tictock` does no polling or busy-waiting. It sleeps until the display would
actually change — every 500ms to blink the digital colon, every second for a
seconds readout, or once a minute with seconds hidden — and otherwise sits
at 0% CPU, backed by your OS's native event notification (kqueue/epoll/IOCP)
via [crossterm](https://github.com/crossterm-rs/crossterm).

## Building from source

```sh
git clone https://github.com/HainanZhao/tictock
cd tictock
cargo build --release
./target/release/tictock
```

## License

MIT
