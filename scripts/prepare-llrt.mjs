import { execFileSync } from 'node:child_process'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--format-version', '1'], {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 32 * 1024 * 1024,
}))
const core = metadata.packages.find(pkg => pkg.name === 'llrt_core')
if (!core) throw new Error('llrt_core is missing from Cargo.lock')
const upstream = dirname(dirname(core.manifest_path))

// Use LLRT's own lockfile and bundler; do not copy or patch its implementation.
execFileSync('npm', ['exec', '--yes', '--package=yarn@1.22.22', '--', 'yarn', 'install', '--frozen-lockfile', '--non-interactive'], {
  cwd: upstream,
  stdio: 'inherit',
})
execFileSync(process.execPath, [join(upstream, 'build.mjs')], {
  cwd: upstream,
  env: { ...process.env, SDK_BUNDLE_MODE: 'NONE' },
  stdio: 'inherit',
})
