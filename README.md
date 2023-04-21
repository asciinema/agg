# agg - asciinema gif generator

agg is a command-line tool for generating animated GIF files from [asciicast v2
files](https://github.com/asciinema/asciinema/blob/master/doc/asciicast-v2.md)
produced by [asciinema terminal
recorder](https://github.com/asciinema/asciinema).

It uses Kornel Lesiński's excellent
[gifski](https://github.com/ImageOptim/gifski) library to produce optimized,
high quality GIF output with accurate frame timing.

agg is a successor to
[asciicast2gif](https://github.com/asciinema/asciicast2gif).

![Demo](demo.gif)

## Building

Building from source requires [Rust](https://www.rust-lang.org/) compiler
(1.56.0 or later) and [Cargo package manager](https://doc.rust-lang.org/cargo/).
You can install both with [rustup](https://rustup.rs/).

To download source code, build agg binary and install it in `$HOME/.cargo/bin`
run:

```bash
cargo install --git https://github.com/asciinema/agg
```

You need to ensure `$HOME/.cargo/bin` is in your shell's `$PATH`.

Alternatively, you can manually download source code and build agg binary with:

```bash
git clone https://github.com/asciinema/agg
cd agg
cargo build --release
```

This produces an executable file in _release mode_ (`--release`) at
`target/release/agg`. There are no other build artifacts so you can copy the
binary to a directory in your `$PATH`.

## Usage

Basic usage:

```bash
agg demo.cast demo.gif
```

The above command renders a GIF file with default theme (dracula), font size
14px.

Additional options are available for customization. For example, the following
command selects Monokai theme, larger font size, 2x playback speed:

```bash
agg --theme monokai --font-size 20 --speed 2 demo.cast demo.gif
```

Run `agg -h` to see all available options. Current options are:

```
    --cols <COLS>
        Override terminal width (number of columns)

    --font-dir <FONT_DIR>
        Use additional font directory

    --font-family <FONT_FAMILY>
        Specify font family [default: "JetBrains Mono,Fira Code,SF Mono,Menlo,Consolas,DejaVu
        Sans Mono,Liberation Mono"]

    --font-size <FONT_SIZE>
        Specify font size (in pixels) [default: 14]

    --fps-cap <FPS_CAP>
        Set FPS cap [default: 30]

-h, --help
        Print help information

    --idle-time-limit <IDLE_TIME_LIMIT>
        Limit idle time to max number of seconds [default: 5]

    --line-height <LINE_HEIGHT>
        Specify line height [default: 1.4]

    --no-loop
        Disable animation loop

    --renderer <RENDERER>
        Select frame rendering backend [default: fontdue] [possible values: fontdue, resvg]

    --rows <ROWS>
        Override terminal height (number of rows)

    --speed <SPEED>
        Adjust playback speed [default: 1]

    --theme <THEME>
        Select color theme [possible values: asciinema, dracula, monokai, solarized-dark,
        solarized-light, custom]

-v, --verbose
        Enable verbose logging

-V, --version
        Print version information
```

## Fonts

By default agg uses common monospaced font for a given platform, that can be
found on its default font family list. The list includes DejaVu Sans Mono and
Liberation Mono (found on most Linux distros), SF Mono and Menlo (found on
macOS), Consolas (found on Windows), with addition of my personal favourites
like JetBrains Mono and Fira Code. The fonts are not included in agg and must be
present on the system. To see the default value run `agg --help` and look for
`--font-family`. In addition there's implicit fallback to DejaVu Sans (not Mono)
which helps with rendering symbols like ⣽ or ✔ amongst others.

If you want to use another font family then pass a comma-separated list like
this:

```bash
agg --font-family "Source Code Pro,Fira Code" demo.cast demo.gif
```

As long as the fonts you want to use are installed in one of standard system
locations (e.g. /usr/share/fonts or ~/.local/share/fonts on Linux) agg will find
them. You can also use `--font-dir=/path/to/fonts` option to include extra
fonts. `--font-dir` can be specified multiple times.

To verify agg picks up your font run it with `-v` (verbose) flag:

```bash
agg -v --font-family "Source Code Pro,Fira Code" demo.cast demo.gif
```
It should print something similar to:

```
[INFO agg] selected font families: ["Source Code Pro", "Fira Code", "DejaVu Sans", "Noto Emoji"]
```

This list may also include implicit addition of DejaVu Sans fallback (mentioned
earlier), as well as Noto Emoji (see section below).

Here's how to use [Nerd Fonts](https://www.nerdfonts.com/) with agg:

1. Download one of the patched font sets from
   https://github.com/ryanoasis/nerd-fonts/releases/latest , e.g. JetBrainsMono.zip
2. Unzip them into `~/.local/share/fonts` (on Linux) or install with system font
   manager (macOS, Windows)
3. Specify font family like this:

```bash
agg --font-family "JetBrainsMono Nerd Font Mono" demo.cast demo.gif
```

## Emoji

Currently agg supports only monochrome emojis via [Noto Emoji
font](https://fonts.google.com/noto/specimen/Noto+Emoji).

Install Noto Emoji font on your system or, point agg to a folder containing
`NotoEmoji-*.ttf` files with `--font-dir`.

Note that [Noto Color Emoji
font](https://fonts.google.com/noto/specimen/Noto+Color+Emoji) is not supported.
Be aware that [some
distros](https://archlinux.org/packages/extra/any/noto-fonts-emoji/) ship this
color font by name like "noto-fonts-emoji". This is _not_ what you need.

## Color themes

There are several built-in color themes you can use with `--theme` option:

- asciinema
- dracula (default)
- monokai
- solarized-dark
- solarized-light

If your asciicast file includes [theme
definition](https://github.com/asciinema/asciinema/blob/develop/doc/asciicast-v2.md#theme)
then it's used automatically unless `--theme` option is explicitly specified.

A custom, ad-hoc theme can be used with `--theme` option by passing a series of
comma-separated hex triplets defining terminal background color, default text
color and a color palette:

```
--theme bbbbbb,ffffff,000000,111111,222222,333333,444444,555555,666666,777777
```

The above sets terminal background color to `bbbbbb`, default text color to `ffffff`,
and uses remaining 8 colors as [SGR color
palette](https://en.wikipedia.org/wiki/ANSI_escape_code#Colors).

Additional bright color variants can be specified by adding 8 more hex triplets
at the end. For example, the equivalent of the built-in Monokai theme is:

```
--theme 272822,f8f8f2,272822,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f8f8f2,75715e,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f9f8f5
```

## Additional GIF optimization

GIF encoder used by agg, [gifski](https://github.com/ImageOptim/gifski),
produces great looking GIF files, although this often comes at a cost - file
size.

[gifsicle](https://www.lcdf.org/gifsicle/) can be used to shrink the produced GIF file:

```bash
gifsicle --lossy=80 -k 128 -O2 -Okeep-empty demo.gif -o demo-opt.gif
```

Every recording is different so you may need to tweak the lossiness level
(`--lossy`), number of colors (`-k`) and other options to suit your needs.

## License

© 2022 Marcin Kulik.

All code is licensed under the Apache License, Version 2.0. See LICENSE file for details.
