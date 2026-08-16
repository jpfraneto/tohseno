export interface Clock {
  now(): number;
}

export const systemClock: Clock = { now: () => Date.now() };

export class WindowRateLimiter {
  private readonly windows = new Map<string, { startedAt: number; count: number }>();

  constructor(
    private readonly clock: Clock = systemClock,
    private readonly maximumKeys = 20_000,
  ) {}

  take(key: string, maximum: number): boolean {
    const now = this.clock.now();
    if (this.windows.size >= this.maximumKeys) this.prune(now);
    if (this.windows.size >= this.maximumKeys && !this.windows.has(key)) return false;

    const current = this.windows.get(key);
    if (!current || now - current.startedAt >= 60_000) {
      this.windows.set(key, { startedAt: now, count: 1 });
      return true;
    }
    if (current.count >= maximum) return false;
    current.count += 1;
    return true;
  }

  private prune(now: number): void {
    for (const [key, window] of this.windows) {
      if (now - window.startedAt >= 60_000) this.windows.delete(key);
    }
  }
}
