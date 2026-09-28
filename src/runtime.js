import { AsyncLocalStorage } from 'node:async_hooks'

const flavorContext = new AsyncLocalStorage()

// Flavor modules are ordinary ESM files. Node provides fetch and Buffer as
// globals; buffer and url are available through normal node: imports.
if (!globalThis.applyFlavor) {
  globalThis.applyFlavor = (input, flavors) => {
    const context = flavorContext.getStore()
    if (!context) {
      return Promise.reject(new Error('applyFlavor can only be called while a flavor is running'))
    }
    return context.applyFlavor(input, flavors)
  }
}

export function runInFlavorContext(applyFlavor, callback) {
  return flavorContext.run({ applyFlavor }, callback)
}
