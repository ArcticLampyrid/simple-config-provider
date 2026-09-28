#!/usr/bin/env node

import { startServer } from './server.js'

try {
  const server = await startServer({
    host: process.env.HOST ?? '0.0.0.0',
    port: Number(process.env.PORT ?? 3000),
  })
  const address = server.address()
  const shownAddress = typeof address === 'object' && address ? `${address.address}:${address.port}` : address
  console.log(`simple-config-provider listening on ${shownAddress}`)
} catch (error) {
  console.error(error instanceof Error ? error.stack : error)
  process.exitCode = 1
}
