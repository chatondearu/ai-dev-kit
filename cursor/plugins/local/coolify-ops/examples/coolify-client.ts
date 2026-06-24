type HttpMethod = 'GET' | 'POST' | 'PATCH' | 'DELETE'

type CoolifyErrorCode =
  | 'AUTH_ERROR'
  | 'NOT_FOUND'
  | 'RATE_LIMIT'
  | 'VALIDATION_ERROR'
  | 'REMOTE_ERROR'

export class CoolifyApiError extends Error {
  constructor(
    public readonly code: CoolifyErrorCode,
    message: string,
    public readonly retryable: boolean,
    public readonly details?: unknown,
  ) {
    super(message)
  }
}

export interface CoolifyClientOptions {
  baseUrl: string
  token: string
  timeoutMs?: number
  maxRetries?: number
}

export interface RetryOptions {
  retries?: number
  initialDelayMs?: number
  maxDelayMs?: number
}

export interface DeploymentWatchOptions {
  intervalMs?: number
  timeoutMs?: number
  logTailLines?: number
}

export interface DeploymentWatchResult {
  deploymentUuid: string
  finalState: string
  attempts: number
  logs?: unknown
}

export class CoolifyClient {
  private readonly baseUrl: string
  private readonly token: string
  private readonly timeoutMs: number
  private readonly maxRetries: number

  constructor(options: CoolifyClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, '')
    this.token = options.token
    this.timeoutMs = options.timeoutMs ?? 10_000
    this.maxRetries = options.maxRetries ?? 3
  }

  async health(): Promise<unknown> {
    return this.request('GET', '/api/v1/version')
  }

  async listApplications(): Promise<unknown> {
    return this.request('GET', '/api/v1/applications')
  }

  async deployApplication(uuid: string): Promise<unknown> {
    return this.request('POST', `/api/v1/deploy?uuid=${encodeURIComponent(uuid)}`)
  }

  async getDeployment(deploymentUuid: string): Promise<unknown> {
    return this.request('GET', `/api/v1/deployments/${encodeURIComponent(deploymentUuid)}`)
  }

  async getDeploymentLogs(deploymentUuid: string, tailLines = 200): Promise<unknown> {
    return this.request('GET', `/api/v1/deployments/${encodeURIComponent(deploymentUuid)}/logs?tail=${tailLines}`)
  }

  async deployAndWatch(appUuid: string, options: DeploymentWatchOptions = {}): Promise<DeploymentWatchResult> {
    const intervalMs = options.intervalMs ?? 2_000
    const timeoutMs = options.timeoutMs ?? 10 * 60_000
    const logTailLines = options.logTailLines ?? 200

    const deployResponse = await this.withRetry(() => this.deployApplication(appUuid))
    const deploymentUuid = this.extractDeploymentUuid(deployResponse)

    if (!deploymentUuid) {
      throw new CoolifyApiError(
        'VALIDATION_ERROR',
        'Deployment was triggered but deployment UUID was missing in response.',
        false,
        { response: deployResponse },
      )
    }

    const start = Date.now()
    let attempts = 0
    let finalState = 'unknown'

    while (Date.now() - start < timeoutMs) {
      attempts += 1
      const deployment = await this.withRetry(() => this.getDeployment(deploymentUuid))
      finalState = this.extractDeploymentState(deployment)

      if (this.isTerminalState(finalState)) {
        const logs = await this.withRetry(() => this.getDeploymentLogs(deploymentUuid, logTailLines))
        return {
          deploymentUuid,
          finalState,
          attempts,
          logs,
        }
      }

      await this.sleep(intervalMs)
    }

    throw new CoolifyApiError(
      'REMOTE_ERROR',
      'Deployment polling timed out before reaching a terminal state.',
      true,
      { deploymentUuid, timeoutMs },
    )
  }

  private async request(method: HttpMethod, path: string): Promise<unknown> {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs)

    try {
      const res = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers: {
          Authorization: `Bearer ${this.token}`,
          'Content-Type': 'application/json',
        },
        signal: controller.signal,
      })

      const text = await res.text()
      const data = this.tryParseJson(text)

      if (!res.ok) {
        throw this.toApiError(res.status, data ?? text)
      }

      return data ?? text
    }
    catch (error) {
      if (error instanceof CoolifyApiError)
        throw error

      throw new CoolifyApiError(
        'REMOTE_ERROR',
        'Coolify request failed before receiving a valid response.',
        true,
        { cause: String(error) },
      )
    }
    finally {
      clearTimeout(timeout)
    }
  }

  private async withRetry<T>(fn: () => Promise<T>, retryOptions: RetryOptions = {}): Promise<T> {
    const retries = retryOptions.retries ?? this.maxRetries
    const initialDelayMs = retryOptions.initialDelayMs ?? 300
    const maxDelayMs = retryOptions.maxDelayMs ?? 4_000

    let attempt = 0
    let delay = initialDelayMs

    while (true) {
      try {
        return await fn()
      }
      catch (error) {
        attempt += 1
        if (!(error instanceof CoolifyApiError) || !error.retryable || attempt > retries) {
          throw error
        }

        await this.sleep(delay)
        delay = Math.min(delay * 2, maxDelayMs)
      }
    }
  }

  private toApiError(status: number, details: unknown): CoolifyApiError {
    if (status === 401 || status === 403) {
      return new CoolifyApiError('AUTH_ERROR', 'Authentication failed with Coolify API.', false, details)
    }

    if (status === 404) {
      return new CoolifyApiError('NOT_FOUND', 'Requested Coolify resource was not found.', false, details)
    }

    if (status === 422) {
      return new CoolifyApiError('VALIDATION_ERROR', 'Coolify rejected the request payload.', false, details)
    }

    if (status === 429) {
      return new CoolifyApiError('RATE_LIMIT', 'Coolify API rate limit reached.', true, details)
    }

    return new CoolifyApiError('REMOTE_ERROR', `Coolify API returned status ${status}.`, status >= 500, details)
  }

  private tryParseJson(text: string): unknown | null {
    if (!text)
      return null

    try {
      return JSON.parse(text)
    }
    catch {
      return null
    }
  }

  private extractDeploymentUuid(payload: unknown): string | null {
    if (!payload || typeof payload !== 'object')
      return null

    const obj = payload as Record<string, unknown>
    const direct = obj.deployment_uuid ?? obj.uuid

    if (typeof direct === 'string')
      return direct

    if (obj.data && typeof obj.data === 'object') {
      const data = obj.data as Record<string, unknown>
      const nested = data.deployment_uuid ?? data.uuid
      if (typeof nested === 'string')
        return nested
    }

    return null
  }

  private extractDeploymentState(payload: unknown): string {
    if (!payload || typeof payload !== 'object')
      return 'unknown'

    const obj = payload as Record<string, unknown>
    const direct = obj.status ?? obj.state
    if (typeof direct === 'string')
      return direct.toLowerCase()

    if (obj.data && typeof obj.data === 'object') {
      const data = obj.data as Record<string, unknown>
      const nested = data.status ?? data.state
      if (typeof nested === 'string')
        return nested.toLowerCase()
    }

    return 'unknown'
  }

  private isTerminalState(state: string): boolean {
    return ['finished', 'success', 'failed', 'error', 'canceled', 'cancelled'].includes(state)
  }

  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms))
  }
}
