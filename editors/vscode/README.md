# Gale CSS Linter — VSCode Extension

An extremely fast CSS linter, written in Rust. This extension integrates the Gale LSP server into VSCode to provide real-time linting diagnostics for CSS, SCSS, Less, and Sass files.

## Features

- Real-time linting diagnostics as you type (or on save)
- Supports CSS, SCSS, Less, and Sass
- Auto-discovers Gale configuration files (`gale.json`, `gale.toml`, `.stylelintrc`, etc.)
- Minimal footprint — the linter runs as a native binary

## Requirements

Install the `gale-lint` npm package in your project:

```sh
npm install -D gale-lint
```

Or install `gale` globally so it is available on your PATH.

## Configuration

| Setting           | Type                    | Default  | Description                                      |
| ----------------- | ----------------------- | -------- | ------------------------------------------------ |
| `gale.enable`     | `boolean`               | `true`   | Enable or disable the linter.                    |
| `gale.configPath` | `string`                | `""`     | Explicit path to a config file (auto-discovered if empty). |
| `gale.run`        | `"onType"` \| `"onSave"` | `"onType"` | When to run the linter.                          |

## Commands

- **Gale: Restart** — Restart the Gale LSP server (`gale.restart`).
