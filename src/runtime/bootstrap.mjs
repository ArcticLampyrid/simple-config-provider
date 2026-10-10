// Rust flavor loader, provided by the entry point below.
let loadFlavor

async function evaluate(source, input) {
  if (!source.module) return source.value
  const module = await import(source.module)
  if (typeof module.default !== 'function') {
    throw new TypeError('Flavor module ' + source.name + ' must export a default function')
  }
  return module.default(input)
}

globalThis.applyFlavor = async (input, flavors) => {
  let output = structuredClone(input)
  for (const name of Array.isArray(flavors) ? flavors : [flavors]) {
    if (typeof name !== 'string') throw new TypeError('A flavor name must be a string')
    output = structuredClone(await evaluate(loadFlavor(name), output))
  }
  return output
}

// Called once per VM by runtime.rs.
export default (load, source) => {
  loadFlavor = load
  return evaluate(source, undefined)
}
