# qmljsfmt

Formats JavaScript embedded in QML documents using [Oxfmt](https://oxc.rs/docs/guide/usage/formatter.html).

Reads one complete QML document from stdin and writes the result to stdout.

## Usage

```sh
cat input.qml | qmljsfmt > output.qml
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
    },
  },
})
```

## Development

```sh
nix develop
cargo test
```
