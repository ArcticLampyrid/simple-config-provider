# simple-config-provider

`simple-config-provider` 是一个小型 Node.js 服务，用于分发 JSON、JSON5、
YAML 和 TOML 配置。一个 flavor 可以是普通配置文件，也可以是可执行的
JavaScript 模块。

## 快速开始

```sh
pnpm install
pnpm start
```

服务默认读取 `config/config.json5`，并从 `config/flavor/` 读取 flavor 文件。
可以通过 `HOST`、`PORT` 和 `CONFIG_DIR` 修改默认值（分别为
`0.0.0.0`、`3000` 和 `./config`）。

## 鉴权和过滤器

通过 query param `token` 进行鉴权。token 刻意以明文存储在配置文件中。
配置可以使用单个 ACL 对象，也可以使用 ACL 对象数组：

```json5
{
  "acls": [
    { "token": "secret", "filter": ["base", { "pattern": "^_", "invert": true }] },
    { "token": "", "filter": ["public"] }
  ]
}
```

每个 token 可以关联一个或多个 filter，filter 使用正则表达式进行匹配。
同一个 ACL 中的 filter 是“或”关系：字符串 filter 是普通的允许规则；
`{ "invert": true }` 表示允许不匹配该正则表达式的 flavor。因此上例允许
`base`，以及所有不以 `_` 开头的 flavor。

特别地，token 为空的 ACL 表项表示匿名访问，此时 `token` query param 可以
不存在。未知 token 返回 `403`；没有 token 且没有匹配的匿名 ACL 时返回 `401`。

## Endpoint 和 flavor

通过不同 endpoint 提供不同 flavor 的配置，URL 扩展名决定输出格式，扩展名
之前的名称决定 flavor：

```text
/base.json?token=secret
/pc.yaml?token=secret
/mobile.json5?token=secret
/long/path/one.toml?token=secret
```

服务会按以下顺序查找 flavor 文件：

1. `<name>.mjs`
2. `<name>.js`
3. `<name>.json5`、`<name>.json`、`<name>.yaml`、`<name>.yml` 或 `<name>.toml`

因此，`pc.yaml` 可能会执行 `pc.mjs`。普通 flavor 会忽略输入并始终返回
解析后的文件内容；可执行 flavor 必须导出形如
`(input) => value | Promise<value>` 的默认函数：

```js
export default async function (input) {
  const config = await applyFlavor(input, 'base')
  return { ...config, platform: 'pc' }
}
```

每种 flavor 都等价于 `(input: any): any | Promise<any>` 函数。普通配置文件
等价于 `map(_): any` 函数，无论输入什么都返回唯一输出。

`applyFlavor(input, name)` 用于应用一个 flavor；传入数组时会按数组顺序依次
应用多个 flavor。Flavor 模块可以使用 Node.js 全局的 `fetch` 和 `Buffer`，
`buffer` 和 `url` 则通过常规方式导入：

```js
import { Buffer } from 'node:buffer'
import * as url from 'node:url'
```

## Docker

```sh
docker build -t simple-config-provider .
docker run --rm -p 3000:3000 -v "$PWD/config:/app/config:ro" \
  simple-config-provider
```

## 技术栈和国际化

项目使用 Node.js、PNPM 和基于 Alpine 的 Docker 镜像。代码注释均使用英文，
文档同时提供中英文版本。
