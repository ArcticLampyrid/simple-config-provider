# simple-config-provider

一个小型配置服务，用于分发 JSON、JSON5、YAML 和 TOML。flavor 可以是普通配置文件，也可以是按需生成输出的 JavaScript 模块。

## 快速开始

依赖：Rust 1.95+、Node.js 24 与 npm、C/C++ 编译工具链、CMake、pkg-config、Git、`zstd`。Node.js 仅在构建时需要。

```sh
node scripts/prepare-llrt.mjs
cargo run --release --locked
```

首次检出后需运行准备脚本。服务默认读取 `config/config.json5`，并从 `config/flavor/` 读取 flavor。可通过 `HOST`、`PORT`、`CONFIG_DIR` 修改默认值（`0.0.0.0`、`3000`、`./config`）。

## Endpoint 和 flavor

URL 扩展名决定输出格式，扩展名之前的路径决定 flavor：

```text
/base.json?token=secret
/pc.yaml?token=secret
/mobile.json5?token=secret
/long/path/one.toml?token=secret
```

查找顺序：`.mjs`、`.js`、`.json5`、`.json`、`.yaml`、`.yml`、`.toml`。

普通 flavor 忽略输入，直接返回解析后的文件。脚本 flavor 导出一个默认函数，接收输入并返回输出（可为异步）：

```js
export default async function (input) {
  const config = await applyFlavor(input, 'base')
  return { ...config, platform: 'pc' }
}
```

`applyFlavor(input, name)` 应用一个 flavor；传入名称数组则按顺序依次应用。脚本运行在 [LLRT](https://github.com/awslabs/llrt) 上，可使用 `fetch`、`Buffer`、`TextEncoder`、`TextDecoder`、`atob`、`btoa`，并可导入 `node:buffer` 和 `node:url`。

每个请求使用独立 VM，请求之间不共享状态。脚本 flavor 是受信任的服务端代码，不是沙箱。出错时返回 HTTP 500。

脚本 VM 的资源限制可在 `config/config.json5` 中设置：

```json5
{
  vm_options: { memory_limit: '64MiB', max_stack_size: '512KiB' },
}
```

`memory_limit` 默认 `64MiB`（`"0"` 表示不限制），`max_stack_size` 不能超过 `1MiB`。超出限制时返回 HTTP 500。flavor 递归应用自身不会被检测，会一直运行到触及内存上限。

## 鉴权

在 `config/config.json5` 中配置访问权限，token 以明文保存。`acls` 可以是单个对象、对象数组，或 token 到 filter 的 map：

```json5
{
  acls: [
    { token: 'secret', filter: ['base', { pattern: '^_', invert: true }] },
    { token: '', filter: ['public'] }
  ]
}
```

filter 是匹配 flavor 名称的正则表达式，任一匹配即允许访问；`invert: true` 表示允许不匹配的名称。空 token 的 ACL 允许匿名访问。未知或无权限的 token 返回 403，缺少 token 返回 401。

## Docker

```sh
docker build -t simple-config-provider .
docker run --rm -p 3000:3000 -v "$PWD/config:/app/config:ro" simple-config-provider
```
