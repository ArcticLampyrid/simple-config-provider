import JSON5 from 'json5'
import { parse as parseToml, stringify as stringifyToml } from '@iarna/toml'
import YAML from 'yaml'

const FORMAT_DEFINITIONS = Object.freeze({
  json: Object.freeze({
    contentType: 'application/json; charset=utf-8',
    parse: JSON.parse,
    stringify: value => JSON.stringify(value, null, 2),
  }),
  json5: Object.freeze({
    contentType: 'application/json5; charset=utf-8',
    parse: JSON5.parse,
    stringify: value => JSON5.stringify(value, null, 2),
  }),
  yaml: Object.freeze({
    contentType: 'application/yaml; charset=utf-8',
    parse: YAML.parse,
    stringify: YAML.stringify,
  }),
  yml: Object.freeze({
    contentType: 'application/yaml; charset=utf-8',
    parse: YAML.parse,
    stringify: YAML.stringify,
  }),
  toml: Object.freeze({
    contentType: 'application/toml; charset=utf-8',
    parse: parseToml,
    stringify: stringifyToml,
  }),
})

export const SOURCE_EXTENSIONS = Object.freeze([
  'json5',
  'json',
  'yaml',
  'yml',
  'toml',
])

export const SCRIPT_EXTENSIONS = Object.freeze(['mjs', 'js'])

export const OUTPUT_FORMATS = Object.freeze(Object.keys(FORMAT_DEFINITIONS))

export function getFormat(extension) {
  return FORMAT_DEFINITIONS[extension.toLowerCase()]
}

export function parseConfig(text, extension) {
  if (text.trim() === '') return null

  const format = getFormat(extension)
  if (!format) throw new Error(`Unsupported configuration format: ${extension}`)
  return format.parse(text)
}

export function serializeConfig(value, extension) {
  const format = getFormat(extension)
  if (!format) throw new Error(`Unsupported output format: ${extension}`)

  // JSON.stringify is allowed to return undefined for an undefined root value.
  // HTTP responses still need a valid representation, so use JSON null there.
  const serialized = format.stringify(value === undefined ? null : value)
  return typeof serialized === 'string' ? serialized : String(serialized)
}
