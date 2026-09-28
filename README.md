# simple-config-provider

`simple-config-provider` is a small Node.js service for distributing JSON,
JSON5, YAML, and TOML configuration. A flavor is either a plain configuration
file or an executable JavaScript module.

## Quick start

```sh
pnpm install
pnpm start
```

The service reads `config/config.json5` and flavor files from
`config/flavor/`. `HOST`, `PORT`, and `CONFIG_DIR` can be used to change the
runtime defaults (`0.0.0.0`, `3000`, and `./config`).

## Endpoints and flavors

The URL extension selects the output format, while the name before the
extension selects a flavor:

```text
/base.json?token=secret
/pc.yaml?token=secret
/mobile.json5?token=secret
/long/path/one.toml?token=secret
```

The service looks for the flavor in this order:

1. `<name>.mjs`
2. `<name>.js`
3. `<name>.json5`, `<name>.json`, `<name>.yaml`, `<name>.yml`, or
   `<name>.toml`

A plain flavor ignores its input and always returns the parsed file. An
executable flavor must export a default function with the shape
`(input) => value | Promise<value>`:

```js
export default async function (input) {
  const config = await applyFlavor(input, 'base')
  return { ...config, platform: 'pc' }
}
```

Every flavor therefore behaves like an `(input: any): any | Promise<any>`
function. A plain configuration file is equivalent to `map(_): any` and
always returns the same output regardless of its input.

`applyFlavor(input, name)` applies one flavor. Passing an array applies them
in order. Flavor modules can use Node's global `fetch` and `Buffer`, while
`buffer` and `url` can be imported normally:

```js
import { Buffer } from 'node:buffer'
import * as url from 'node:url'
```

## Authentication and filters

Authentication is configured in `config/config.json5`. Tokens are deliberately
stored as plain text. The configuration may contain one ACL object or an array
of ACL objects:

```json5
{
  acls: [
    { token: 'secret', filter: ['base', { pattern: '^_', invert: true }] },
    { token: '', filter: ['public'] }
  ]
}
```

An ACL with an empty token enables anonymous access. Its `token` query
parameter may therefore be omitted. String filters are regular-expression
allow rules. An object with `invert: true` allows names that do not match its
regular expression. Filters in one ACL are alternatives, so the example
allows `base` and every flavor that does not start with `_`.

An unknown token receives `403`, while a request without a token when no
anonymous ACL matches receives `401`.

## Docker

```sh
docker build -t simple-config-provider .
docker run --rm -p 3000:3000 -v "$PWD/config:/app/config:ro" \
  simple-config-provider
```

## Technology and localization

The project uses Node.js, PNPM, and an Alpine-based Docker image. Source code
comments are written in English, and the documentation is available in both
English and Chinese.
