# agg - asciinema gif generator

__agg__ is a command-line tool for generating animated GIF files from terminal
session recordings.

It supports conversion from
[asciicast](https://github.com/asciinema/asciinema/blob/master/doc/asciicast-v3.md)
files produced by [asciinema recorder](https://github.com/asciinema/asciinema).
It uses Kornel Lesiński's excellent
[gifski](https://github.com/ImageOptim/gifski) library to produce optimized,
high quality GIF output with accurate frame timing.

Example GIF file generated with agg:

![Example GIF file generated with agg](demo.gif)

Notable features:

- conversion of [asciicast](https://docs.asciinema.org/manual/asciicast/v3/)
  recordings (v1, v2, v3) to animated GIF files,
- input from local files, stdin, or HTTP(S) URLs (e.g.
  [asciinema.org](https://asciinema.org) recording links),
- high-quality, optimized GIF output with accurate frame timing via the
  [gifski](https://github.com/ImageOptim/gifski) encoder,
- multiple built-in color themes (asciinema, dracula, monokai, github-dark,
  github-light, kanagawa, nord, solarized-dark, solarized-light, gruvbox-dark,
  and more),
- custom ad-hoc themes specified as hex color triplets,
- automatic use of the recording's embedded theme when present,
- configurable [font families](https://docs.asciinema.org/manual/agg/usage/#fonts)
  with sensible cross-platform defaults and implicit fallbacks for symbols,
  including automatic Nerd Font symbols rendering,
- configurable font size and line height,
- additional font directory support via `--font-dir` for fonts outside standard
  system locations,
- color emoji rendering with support for Apple Color Emoji, Noto Color Emoji,
  and other common emoji fonts,
- two selectable rendering backends: `swash` (default) and `resvg`,
- adjustable playback speed,
- idle time limiting to skip periods of inactivity,
- frame selection by time ranges, discrete positions, markers, percentages, and
  event indexes,
- looped or single-pass playback,
- configurable FPS cap and last-frame duration,
- terminal size override (cols/rows) for re-rendering at a different geometry.

Check out the [agg docs](https://docs.asciinema.org/manual/agg/) for
installation and usage overview.

## Building

To download source code, build agg binary and install it in `$HOME/.cargo/bin`
run:

```sh
cargo install --git https://github.com/asciinema/agg
```

This requires the Rust toolchain (1.86 or later) with Cargo, installed via your
system package manager or [rustup](https://rustup.rs/).

Once installed, ensure `$HOME/.cargo/bin` is in your shell's `$PATH`.

Alternatively, you can build from a local checkout. First download the source
code:

```sh
git clone https://github.com/asciinema/agg
cd agg
```

The recommended way to get the toolchain is the Nix dev shell, which provides
everything needed and just works:

```sh
nix develop
```

If you don't use Nix, you need Rust with Cargo, as above.

Then build the binary with:

```sh
cargo build --release
```

This produces an executable file in _release mode_ (`--release`) at
`target/release/agg`. There are no other build artifacts so you can copy the
binary to a directory in your `$PATH`.

### Building with Docker

Alternatively, if you have Docker, Podman or another Docker-compatible tool
installed you can use it to build agg container image. This doesn't require Rust
toolchain installed on your machine.

Build the image with the following command:

```sh
docker build -t agg .
```

Then run agg like this:

```sh
docker run --rm -u $(id -u):$(id -g) -v $PWD:/data agg demo.cast demo.gif
```

If you use Podman in root-less mode:

```sh
podman run --rm -v $PWD:/data agg demo.cast demo.gif
```

## Development

For toolchain setup and build instructions, see [Building](#building). Run the
test suite with `cargo test`.

If you'd like to propose or submit any changes, please read the
[contribution guidelines](CONTRIBUTING.md) first.

## Consulting

If you're interested in customization of agg or any other asciinema component to
for your corporate needs, check [asciinema consulting
services](https://docs.asciinema.org/consulting/).

## License

© 2022 Marcin Kulik.


All code is licensed under the GPL, v3 or later. See [LICENSE](./LICENSE) file
for details.
