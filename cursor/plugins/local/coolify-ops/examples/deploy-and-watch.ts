import { CoolifyApiError, CoolifyClient } from './coolify-client'

interface CliOptions {
  appUuid: string
  intervalMs: number
  timeoutMs: number
  tailLines: number
}

function printUsage(): void {
  console.log(`Usage:
  tsx deploy-and-watch.ts --app <uuid> [--interval-ms 2000] [--timeout-ms 600000] [--tail-lines 200]

Required environment:
  COOLIFY_BASE_URL
  COOLIFY_TOKEN
`)
}

function parseArgs(argv: string[]): CliOptions | null {
  const args = [...argv]

  const getValue = (flag: string): string | undefined => {
    const idx = args.indexOf(flag)
    if (idx === -1)
      return undefined
    return args[idx + 1]
  }

  const appUuid = getValue('--app')
  if (!appUuid)
    return null

  const intervalMs = Number(getValue('--interval-ms') ?? 2000)
  const timeoutMs = Number(getValue('--timeout-ms') ?? 600000)
  const tailLines = Number(getValue('--tail-lines') ?? 200)

  if (!Number.isFinite(intervalMs) || intervalMs <= 0)
    throw new Error('`--interval-ms` must be a positive number.')
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0)
    throw new Error('`--timeout-ms` must be a positive number.')
  if (!Number.isFinite(tailLines) || tailLines <= 0)
    throw new Error('`--tail-lines` must be a positive number.')

  return {
    appUuid,
    intervalMs,
    timeoutMs,
    tailLines,
  }
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2))
  if (!options) {
    printUsage()
    process.exitCode = 1
    return
  }

  const baseUrl = process.env.COOLIFY_BASE_URL
  const token = process.env.COOLIFY_TOKEN

  if (!baseUrl || !token) {
    console.error(JSON.stringify({
      code: 'VALIDATION_ERROR',
      message: 'Missing COOLIFY_BASE_URL or COOLIFY_TOKEN.',
      retryable: false,
    }, null, 2))
    process.exitCode = 1
    return
  }

  const client = new CoolifyClient({
    baseUrl,
    token,
  })

  const startedAt = Date.now()

  try {
    const result = await client.deployAndWatch(options.appUuid, {
      intervalMs: options.intervalMs,
      timeoutMs: options.timeoutMs,
      logTailLines: options.tailLines,
    })

    const durationMs = Date.now() - startedAt

    console.log(JSON.stringify({
      ok: true,
      deploymentUuid: result.deploymentUuid,
      finalState: result.finalState,
      attempts: result.attempts,
      durationMs,
      logs: result.logs,
    }, null, 2))
  }
  catch (error) {
    const durationMs = Date.now() - startedAt

    if (error instanceof CoolifyApiError) {
      console.error(JSON.stringify({
        ok: false,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
        details: error.details,
        durationMs,
      }, null, 2))
      process.exitCode = 1
      return
    }

    console.error(JSON.stringify({
      ok: false,
      code: 'REMOTE_ERROR',
      message: String(error),
      retryable: true,
      durationMs,
    }, null, 2))
    process.exitCode = 1
  }
}

void main()
