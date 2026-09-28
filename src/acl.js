function asToken(value) {
  return value === undefined || value === null ? '' : String(value)
}

function asFilterList(value) {
  if (value === undefined || value === null) return []
  return Array.isArray(value) ? value : [value]
}

function compileFilter(value) {
  if (typeof value === 'string') {
    return { pattern: new RegExp(value), invert: false }
  }

  if (!value || typeof value !== 'object' || typeof value.pattern !== 'string') {
    throw new TypeError('An ACL filter must be a regular-expression string or an object with pattern')
  }

  return {
    pattern: new RegExp(value.pattern),
    invert: value.invert === true,
  }
}

function normalizeEntry(entry) {
  return {
    token: asToken(entry?.token),
    filters: asFilterList(entry?.filter).map(compileFilter),
  }
}

/**
 * Accept the documented single ACL object as well as an array of ACL objects.
 * The map form is useful for larger configurations: { token: ["pattern"] }.
 */
export function normalizeAcls(value) {
  if (value === undefined || value === null) return []
  if (Array.isArray(value)) return value.map(normalizeEntry)

  if (typeof value !== 'object') {
    throw new TypeError('acls must be an object or an array')
  }

  if ('token' in value || 'filter' in value) return [normalizeEntry(value)]

  return Object.entries(value).map(([token, filter]) => normalizeEntry({ token, filter }))
}

function filtersAllow(filters, flavor) {
  if (filters.length === 0) return true

  // Filters are alternatives. An inverted filter is useful for expressing a
  // broad public rule next to a specific exception, for example
  // ["base", { pattern: "^_", invert: true }].
  return filters.some(filter => filter.invert !== filter.pattern.test(flavor))
}

export function isFlavorAllowed(acls, token, flavor) {
  const requestedToken = token ?? ''
  return acls
    .filter(acl => acl.token === requestedToken)
    .some(acl => filtersAllow(acl.filters, flavor))
}
