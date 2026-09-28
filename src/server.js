import http from 'node:http'
import path from 'node:path'
import { URL } from 'node:url'

import { OUTPUT_FORMATS } from './formats.js'
import { createConfigProvider } from './provider.js'

function jsonError(message) {
  return JSON.stringify({ error: message })
}

function requestPath(request) {
  try {
    return new URL(request.url ?? '/', 'http://localhost').pathname
  } catch {
    return request.url ?? '<unknown>'
  }
}

function logRequestError(request, statusCode, error) {
  const detail = error instanceof Error ? error.stack ?? error.message : error
  console.error(
    `[simple-config-provider] ${request.method} ${requestPath(request)} -> ${statusCode}`,
    detail,
  )
}

function requestTarget(requestUrl) {
  const url = new URL(requestUrl ?? '/', 'http://localhost')
  let pathname
  try {
    pathname = decodeURIComponent(url.pathname)
  } catch {
    throw Object.assign(new Error('Invalid URL encoding'), { statusCode: 400 })
  }
  if (!pathname.startsWith('/') || pathname.includes('\\')) {
    throw Object.assign(new Error('Invalid request path'), { statusCode: 400 })
  }

  const extension = path.posix.extname(pathname).slice(1).toLowerCase()
  if (!OUTPUT_FORMATS.includes(extension)) {
    throw Object.assign(new Error('The URL must end in a supported output format'), { statusCode: 404 })
  }

  const flavorName = pathname.slice(1, -(extension.length + 1))
  if (!flavorName || flavorName.split('/').some(part => part === '' || part === '.' || part === '..')) {
    throw Object.assign(new Error('Invalid flavor path'), { statusCode: 400 })
  }

  return { url, flavorName, extension }
}

function send(response, statusCode, body, contentType = 'application/json; charset=utf-8') {
  response.statusCode = statusCode
  response.setHeader('content-type', contentType)
  response.setHeader('content-length', Buffer.byteLength(body))
  if (response.req.method !== 'HEAD') response.end(body)
  else response.end()
}

export function createServer(provider) {
  return http.createServer(async (request, response) => {
    try {
      if (request.method !== 'GET' && request.method !== 'HEAD') {
        response.setHeader('allow', 'GET, HEAD')
        send(response, 405, jsonError('Method not allowed'))
        return
      }

      const { url, flavorName, extension } = requestTarget(request.url)
      const token = url.searchParams.get('token') ?? ''
      if (!provider.authorize(token, flavorName)) {
        const hasToken = token.length > 0
        const statusCode = hasToken ? 403 : 401
        console.warn(`[simple-config-provider] ${request.method} ${url.pathname} -> ${statusCode} authentication denied`)
        send(response, statusCode, jsonError(hasToken ? 'Forbidden' : 'Authentication required'))
        return
      }

      const result = await provider.renderWithContentType(flavorName, extension)
      send(response, 200, result.body, result.contentType)
    } catch (error) {
      const statusCode = Number.isInteger(error?.statusCode)
        ? error.statusCode
        : /not found/i.test(error?.message ?? '')
          ? 404
          : 500
      logRequestError(request, statusCode, error)
      send(response, statusCode, jsonError(statusCode >= 500 ? 'Internal server error' : error.message))
    }
  })
}

export async function startServer({ provider, host = process.env.HOST ?? '0.0.0.0', port = Number(process.env.PORT ?? 3000) } = {}) {
  const serverProvider = provider ?? await createConfigProvider({
    configDir: process.env.CONFIG_DIR ?? path.resolve('config'),
  })
  const server = createServer(serverProvider)
  await new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(port, host, resolve)
  })
  return server
}
