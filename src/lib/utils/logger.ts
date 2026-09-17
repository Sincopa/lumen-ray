import { runtime } from '../runtime';

export const logger = {
  debug(...args: unknown[]) {
    if (runtime.isDev) {
      console.debug('[DEBUG]', ...args);
    }
  },
  info(...args: unknown[]) {
    console.info('[INFO]', ...args);
  },
  warn(...args: unknown[]) {
    console.warn('[WARN]', ...args);
  },
  error(...args: unknown[]) {
    console.error('[ERROR]', ...args);
  },
};
