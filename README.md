# simple-config-provider

A small service for distributing JSON, JSON5, YAML, and TOML configuration. A flavor is either a plain configuration file or a JavaScript module that builds its output on demand.

## Quick start

Requirements: Rust 1.95+, Node.js 24 with npm, a C/C++ toolchain, CMake, pkg-config, Git, and `zstd`. Node.js is only needed at build time.

```sh
node scripts/prepare-llrt.mjs
cargo run --release --locked
```

Run the preparation step after a fresh checkout. The service reads `config/config.json5` and flavor files from `config/flavor/`. `HOST`, `PORT`, and `CONFIG_DIR` change the defaults (`0.0.0.0`, `3000`, and `./config`).

## Endpoints and flavors

The URL extension selects the output format, and the path before it selects the flavor:

```text
/base.json?token=secret
/pc.yaml?token=secret
/mobile.json5?token=secret
/long/path/one.toml?token=secret
```

Flavors are looked up in this order: `.mjs`, `.js`, `.json5`, `.json`, `.yaml`, `.yml`, `.toml`.

A plain flavor ignores its input and returns the parsed file. A script flavor exports a default function that receives the input and returns the output (it may be async):

```js
export default async function (input) {
  const config = await applyFlavor(input, 'base')
  return { ...config, platform: 'pc' }
}
```

`applyFlavor(input, name)` applies one flavor; pass an array of names to apply several in order. Scripts run on [LLRT](https://github.com/awslabs/llrt) and can use `fetch`, `Buffer`, `TextEncoder`, `TextDecoder`, `atob`, `btoa`, and import `node:buffer` and `node:url`.

Each request runs in a fresh VM, with no state shared between requests. Script flavors are trusted server-side code, not sandboxed. Errors result in HTTP 500.

Resource limits for script VMs can be set in `config/config.json5`:

```json5
{
  vm_options: { memory_limit: '64MiB', max_stack_size: '512KiB' },
}
```

`memory_limit` defaults to `64MiB` (`"0"` disables it), and `max_stack_size` must be at most `1MiB`. A script exceeding a limit fails with HTTP 500. Flavors that apply themselves recursively are not detected and will run until they hit the memory limit.

## Authentication

Configure access in `config/config.json5`. Tokens are stored as plain text. `acls` can be one object, an array, or a map from token to filters:

```json5
{
  acls: [
    { token: 'secret', filter: ['base', { pattern: '^_', invert: true }] },
    { token: '', filter: ['public'] }
  ]
}
```

Filters are regular expressions matched against flavor names, and any match allows access. `invert: true` allows names that do not match. An ACL with an empty token enables anonymous access. Unknown or unauthorized tokens get 403; a missing token gets 401.

## Docker

```sh
docker build -t simple-config-provider .
docker run --rm -p 3000:3000 -v "$PWD/config:/app/config:ro" simple-config-provider
```
