# simple-config-provider

A small Rust service for distributing JSON, JSON5, YAML, and TOML configuration. A flavor is either a plain configuration file or an interpreted JavaScript ES module.

## Quick start

Build prerequisites: Rust 1.95+, Node.js 24 with npm, a C/C++ toolchain, CMake, pkg-config, Git, and the `zstd` command. Node.js is used only to build LLRT’s bundled libraries, not to run the service.

```sh
node scripts/prepare-llrt.mjs
cargo run --release --locked
```

Run the preparation step after a fresh checkout or a change to the LLRT revision. It locates LLRT through Cargo metadata and runs the upstream locked Yarn install and bundler in Cargo’s dependency checkout. No upstream code is patched.

The service reads config/config.json5 and flavor files from config/flavor/. Set HOST, PORT, and CONFIG_DIR to change the defaults: 0.0.0.0, 3000, and ./config.

## Endpoints

The URL extension selects output format and the preceding path selects a flavor:

```text
/base.json?token=secret
/pc.yaml?token=secret
/mobile.json5?token=secret
/long/path/one.toml?token=secret
```

Flavor lookup order is .mjs, .js, .json5, .json, .yaml, .yml, then .toml. Plain flavors ignore input. Script flavors export a default function:

```js
export default async function (input) {
  const config = await applyFlavor(input, 'base')
  return { ...config, platform: 'pc' }
}
```

applyFlavor(input, name) accepts one name or an array of names. Script modules can use global fetch, Buffer, atob, btoa, TextEncoder, and TextDecoder; buffer, node:buffer, url, and node:url are available as module imports.

Scripted requests use `llrt_core::vm::Vm` (LLRT 0.9.0-beta, rquickjs 0.14). LLRT initializes the context, loaders, globals, JSON behavior, console, timers, and `structuredClone`; the project does not implement or register replacements for these APIs. `applyFlavor` copies its input and each result using LLRT’s `structuredClone`.

Each scripted request creates its own VM. Configuration files and flavor source are loaded on demand without application-level content caching; module state is not shared between requests. LLRT’s own library bundle is embedded at build time, without the AWS SDK. This is separate from user flavor files, which are interpreted on demand. LLRT also owns process-global infrastructure, including HTTP clients and timer bookkeeping; apart from the per-VM memory limit below, this service does not override its memory-management behavior.

Flavors are trusted server-side code: the full VM exposes LLRT facilities such as filesystem, process, and networking. It is not a sandbox. Synchronous and returned-Promise failures become HTTP 500 responses. LLRT background-task errors are logged instead of using its default process-exit handler; a script that never settles its returned Promise can still leave a request pending.

Each script VM is limited by `vm_options` in config/config.json5. Sizes are strings such as `"64MiB"`, `"512KiB"` or `"50 MB"`; KiB/MiB are binary, KB/MB decimal.

- `memory_limit` (default `"64MiB"`, `"0"` disables it) caps JavaScript engine memory. Native allocations made by LLRT modules outside the engine are not counted.
- `max_stack_size` (default: LLRT's 512 KiB) caps the JavaScript stack. It must be greater than 0 and at most 1 MiB, because scripts run on threads with a 2 MiB native stack.

A script that exceeds either limit fails with HTTP 500. Flavors are not checked for cycles, so a flavor that applies itself directly or indirectly runs until it hits the memory limit.

```json5
{
  vm_options: { memory_limit: '64MiB', max_stack_size: '512KiB' },
  acls: [/* ... */],
}
```

## Authentication

Authentication is configured in config/config.json5. Tokens are plain text. An ACL can be one object, an array, or a map from token to filters:

```json5
{
  acls: [
    { token: 'secret', filter: ['base', { pattern: '^_', invert: true }] },
    { token: '', filter: ['public'] }
  ]
}
```

Filters are regular expressions and are alternatives within an ACL. An inverted filter allows names that do not match. An empty-token ACL enables anonymous access. Unknown or unauthorized tokens return 403; an unauthorized request without a token returns 401.

## Docker

Both build and runtime stages use Alpine 3.24. Build-only Node.js, npm, and zstd are absent from the final image. TLS and crypto use LLRT’s ring backends.

```sh
docker build -t simple-config-provider .
docker run --rm -p 3000:3000 -v "$PWD/config:/app/config:ro" simple-config-provider
```
