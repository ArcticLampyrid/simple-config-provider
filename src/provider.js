import { readFile, readdir, stat } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

import { isFlavorAllowed, normalizeAcls } from './acl.js'
import {
  getFormat,
  parseConfig,
  SCRIPT_EXTENSIONS,
  serializeConfig,
  SOURCE_EXTENSIONS,
} from './formats.js'
import { runInFlavorContext } from './runtime.js'

const SOURCE_PRIORITY = [...SCRIPT_EXTENSIONS, ...SOURCE_EXTENSIONS]

function cloneValue(value) {
  if (value === undefined || value === null) return value
  return structuredClone(value)
}

function normalizeFlavorName(value) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new TypeError('A flavor name must be a non-empty string')
  }

  const withoutExtension = value.replace(/\.(?:json5?|ya?ml|toml)$/i, '')
  const normalized = path.posix.normalize(withoutExtension)
  if (
    normalized === '.' ||
    normalized.startsWith('../') ||
    normalized.includes('/../') ||
    normalized.startsWith('/') ||
    normalized.includes('\\')
  ) {
    throw new Error(`Invalid flavor name: ${value}`)
  }
  return normalized
}

function sourcePathFor(flavorDirectory, flavorName, extension) {
  return path.join(flavorDirectory, `${flavorName}.${extension}`)
}

export class ConfigProvider {
  constructor({ configDir = path.resolve('config'), configFile } = {}) {
    this.configDir = path.resolve(configDir)
    this.flavorDir = path.join(this.configDir, 'flavor')
    this.configFile = path.resolve(configFile ?? path.join(this.configDir, 'config.json5'))
    this.acls = []
    this.moduleCache = new Map()
  }

  async load() {
    const text = await readFile(this.configFile, 'utf8')
    const extension = path.extname(this.configFile).slice(1)
    const config = parseConfig(text, extension)
    this.acls = normalizeAcls(config?.acls ?? config?.acl)
    return this
  }

  authorize(token, flavorName) {
    return isFlavorAllowed(this.acls, token, flavorName)
  }

  async findFlavor(flavorName) {
    const normalizedName = normalizeFlavorName(flavorName)
    const directory = path.dirname(sourcePathFor(this.flavorDir, normalizedName, 'mjs'))
    const basename = path.basename(normalizedName)

    // Checking the parent directory first gives a clearer error for malformed
    // nested flavors while still keeping all paths below the flavor directory.
    try {
      await stat(directory)
    } catch (error) {
      if (error?.code === 'ENOENT') throw new Error(`Flavor not found: ${normalizedName}`)
      throw error
    }
    const names = new Set(await readdir(directory))
    for (const extension of SOURCE_PRIORITY) {
      const filename = `${basename}.${extension}`
      if (names.has(filename)) {
        return {
          name: normalizedName,
          extension,
          path: sourcePathFor(this.flavorDir, normalizedName, extension),
        }
      }
    }

    throw new Error(`Flavor not found: ${normalizedName}`)
  }

  async loadPlainFlavor(source) {
    const text = await readFile(source.path, 'utf8')
    return parseConfig(text, source.extension)
  }

  async loadScript(source) {
    const sourceStat = await stat(source.path)
    const cacheKey = `${source.path}:${sourceStat.mtimeMs}:${sourceStat.size}`
    const cached = this.moduleCache.get(source.path)
    if (cached?.key === cacheKey) return cached.module

    const module = await import(`${pathToFileURL(source.path).href}?v=${encodeURIComponent(cacheKey)}`)
    this.moduleCache.set(source.path, { key: cacheKey, module })
    return module
  }

  async evaluateFlavor(flavorName, input, stack = []) {
    const source = await this.findFlavor(flavorName)
    if (stack.includes(source.name)) {
      throw new Error(`Circular flavor application: ${[...stack, source.name].join(' -> ')}`)
    }

    if (!SCRIPT_EXTENSIONS.includes(source.extension)) {
      return cloneValue(await this.loadPlainFlavor(source))
    }

    const module = await this.loadScript(source)
    if (typeof module.default !== 'function') {
      throw new TypeError(`Flavor module ${source.name} must export a default function`)
    }

    const applyFlavor = (nextInput, flavors) => this.applyFlavors(nextInput, flavors, [...stack, source.name])
    return runInFlavorContext(applyFlavor, () => module.default(input))
  }

  async applyFlavors(input, flavors, stack = []) {
    const names = Array.isArray(flavors) ? flavors : [flavors]
    let output = cloneValue(input)
    for (const name of names) {
      output = cloneValue(await this.evaluateFlavor(normalizeFlavorName(name), output, stack))
    }
    return output
  }

  async render(flavorName, format) {
    const output = await this.evaluateFlavor(normalizeFlavorName(flavorName), undefined)
    return serializeConfig(output, format)
  }

  async renderWithContentType(flavorName, format) {
    const definition = getFormat(format)
    if (!definition) throw new Error(`Unsupported output format: ${format}`)
    return {
      body: await this.render(flavorName, format),
      contentType: definition.contentType,
    }
  }
}

export async function createConfigProvider(options) {
  return new ConfigProvider(options).load()
}

export { normalizeFlavorName }
