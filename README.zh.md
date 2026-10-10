# simple-config-provider

一个使用 Rust 编写的小型配置服务，用于分发 JSON、JSON5、YAML 和 TOML。flavor 可以是普通配置文件，也可以是解释执行的 JavaScript ES 模块。

## 快速开始

构建依赖：Rust 1.95+、Node.js 24 与 npm、C/C++ 编译工具链、CMake、pkg-config、Git、`zstd` 命令。Node.js 只用于生成 LLRT 自带库的 bundle，运行服务不需要 Node.js。

```sh
node scripts/prepare-llrt.mjs
cargo run --release --locked
```

首次构建或修改 LLRT revision 后运行准备脚本。脚本通过 Cargo metadata 定位 LLRT，在 Cargo 的依赖 checkout 中使用上游 Yarn lockfile 安装构建依赖并运行原有 bundler，不修改上游源码。

服务默认读取 config/config.json5，并按需读取 config/flavor/ 下的 flavor。可以通过 HOST、PORT、CONFIG_DIR 修改默认值：0.0.0.0、3000 和 ./config。

## Endpoint 和 flavor

URL 扩展名决定输出格式，扩展名之前的路径决定 flavor：

```text
/base.json?token=secret
/pc.yaml?token=secret
/mobile.json5?token=secret
/long/path/one.toml?token=secret
```

查找顺序为 .mjs、.js、.json5、.json、.yaml、.yml、.toml。普通 flavor 忽略输入；脚本 flavor 导出默认函数：

```js
export default async function (input) {
  const config = await applyFlavor(input, 'base')
  return { ...config, platform: 'pc' }
}
```

applyFlavor(input, name) 支持一个名称或名称数组，并按顺序应用。脚本可以使用全局的 fetch、Buffer、atob、btoa、TextEncoder、TextDecoder，也可以导入 buffer、node:buffer、url 和 node:url。

脚本请求整体使用 `llrt_core::vm::Vm`（LLRT 0.9.0-beta、rquickjs 0.14）。LLRT 负责 context、模块加载、全局接口、JSON 行为、console、timers 和 `structuredClone`；项目不实现或注册这些 API 的替代品。`applyFlavor` 使用 LLRT 的 `structuredClone` 复制输入和每一步结果。

每个脚本请求创建独立 VM。配置文件和 flavor 源码按需读取，不缓存文件内容或解析结果，模块状态不跨请求共享。LLRT 自带库在构建阶段生成并嵌入 bundle，不包含 AWS SDK；用户 flavor 仍按需解释执行。HTTP client、timer 管理等进程级基础设施由 LLRT 持有；除下文的单 VM 内存上限外，本服务不改写其内存管理行为。

flavor 是受信任的服务端代码：完整 VM 提供文件系统、process、网络等 LLRT 能力，不是沙箱。同步异常和返回 Promise 的失败转换为 HTTP 500；LLRT 后台任务错误会记录日志，而不是使用默认的进程退出处理器。脚本若不结束返回的 Promise，请求仍可能一直等待。

每个脚本 VM 的限制由 config/config.json5 中的 `vm_options` 设置。大小使用字符串，例如 `"64MiB"`、`"512KiB"` 或 `"50 MB"`；KiB/MiB 为二进制单位，KB/MB 为十进制单位。

- `memory_limit`（默认 `"64MiB"`，`"0"` 表示不限制）限制 JavaScript 引擎内存。LLRT 模块在引擎之外的原生内存分配不计入。
- `max_stack_size`（默认使用 LLRT 的 512 KiB）限制 JavaScript 栈。取值必须大于 0 且不超过 1 MiB，因为脚本运行在原生栈为 2 MiB 的线程上。

超出任一限制时脚本失败并返回 HTTP 500。flavor 之间不做循环检测，直接或间接应用自身的 flavor 会一直运行到触及内存上限。

```json5
{
  vm_options: { memory_limit: '64MiB', max_stack_size: '512KiB' },
  acls: [/* ... */],
}
```

## 鉴权和过滤器

鉴权配置在 config/config.json5 中，token 以明文保存。ACL 可以是单个对象、对象数组，或 token 到 filter 的 map：

```json5
{
  acls: [
    { token: 'secret', filter: ['base', { pattern: '^_', invert: true }] },
    { token: '', filter: ['public'] }
  ]
}
```

filter 使用正则表达式，同一 ACL 内为“或”关系。invert: true 表示允许不匹配正则的 flavor。空 token 的 ACL 允许匿名访问；未知或无权限 token 返回 403，没有 token 且不允许匿名时返回 401。

## Docker

构建和运行阶段均使用 Alpine 3.24。最终镜像不包含构建时使用的 Node.js、npm 和 zstd。TLS 与 crypto 使用 LLRT 自带的 ring 后端。

```sh
docker build -t simple-config-provider .
docker run --rm -p 3000:3000 -v "$PWD/config:/app/config:ro" simple-config-provider
```
