/**
 * Contract polling service with exponential backoff retry logic
 * Handles network delays and graceful recovery
 */

export interface PollConfig {
  minInterval: number;
  maxInterval: number;
  backoffMultiplier: number;
  maxRetries: number;
}

const defaultConfig: PollConfig = {
  minInterval: 5000,     // 5 seconds
  maxInterval: 30000,    // 30 seconds
  backoffMultiplier: 1.5,
  maxRetries: 3,
};

interface PollJob<T> {
  id: string;
  fn: () => Promise<T>;
  onSuccess: (data: T) => void;
  onError: (error: Error) => void;
  config: PollConfig;
  interval: number;
  retries: number;
  timeout: ReturnType<typeof setTimeout> | null;
}

export class ContractPollingService {
  private jobs = new Map<string, PollJob<any>>();
  private isActive = true;

  constructor(private config = defaultConfig) {}

  /**
   * Start polling a contract function
   */
  poll<T>(
    id: string,
    fn: () => Promise<T>,
    onSuccess: (data: T) => void,
    onError: (error: Error) => void
  ) {
    if (this.jobs.has(id)) {
      this.stop(id);
    }

    const job: PollJob<T> = {
      id,
      fn,
      onSuccess,
      onError,
      config: this.config,
      interval: this.config.minInterval,
      retries: 0,
      timeout: null,
    };

    this.jobs.set(id, job);
    this.executePoll(job);
  }

  /**
   * Stop polling a specific job
   */
  stop(id: string) {
    const job = this.jobs.get(id);
    if (job?.timeout) {
      clearTimeout(job.timeout);
    }
    this.jobs.delete(id);
  }

  /**
   * Stop all polling
   */
  stopAll() {
    this.isActive = false;
    for (const job of this.jobs.values()) {
      if (job.timeout) {
        clearTimeout(job.timeout);
      }
    }
    this.jobs.clear();
  }

  /**
   * Resume all polling
   */
  resume() {
    this.isActive = true;
    for (const job of this.jobs.values()) {
      this.executePoll(job);
    }
  }

  private async executePoll<T>(job: PollJob<T>) {
    if (!this.isActive) return;

    try {
      const data = await job.fn();
      job.retries = 0;
      job.interval = job.config.minInterval;
      job.onSuccess(data);
    } catch (error) {
      job.retries++;
      if (job.retries >= job.config.maxRetries) {
        job.onError(error instanceof Error ? error : new Error(String(error)));
        job.retries = 0;
      }

      // Exponential backoff
      job.interval = Math.min(
        job.interval * job.config.backoffMultiplier,
        job.config.maxInterval
      );
    }

    job.timeout = setTimeout(() => this.executePoll(job), job.interval);
  }
}

export const pollingService = new ContractPollingService();
