# qmljsfmt

Formats JavaScript embedded in QML documents using [Oxfmt](https://oxc.rs/docs/guide/usage/formatter.html).

## Usage

```
Formats JavaScript embedded in QML documents

Usage: qmljsfmt [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Files and directories to format, or `-` for stdin [default: .]

Options:
  -c, --check    Check if files are formatted
  -w, --write    Rewrite files in place (default)
  -h, --help     Print help
  -V, --version  Print version
```

## Installation

With Nix:

```sh
nix profile install .
```

With Cargo:

```sh
cargo install --path .
```

When installed with Cargo, an `oxfmt` with stdin support must be available on `PATH`. The Nix package includes it.

## Neovim integration with [conform.nvim](https://github.com/stevearc/conform.nvim)

```lua
require("conform").setup({
  formatters_by_ft = {
    qml = { "qmlformat", "qmljsfmt" },
  },
  formatters = {
    qmljsfmt = {
      command = "qmljsfmt",
      args = { "-" },
    },
  },
})
```

## Development

```sh
nix develop
cargo test
```
