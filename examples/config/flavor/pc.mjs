// An executable flavor receives the caller's input and may build on other
// flavors. `_platform/pc` starts with an underscore, so the ACL in
// config.json5 hides it from clients while scripts can still use it.
export default async function (input) {
  const base = await applyFlavor(input, 'base')
  const platform = await applyFlavor(undefined, '_platform/pc')
  return { ...base, ...platform }
}
