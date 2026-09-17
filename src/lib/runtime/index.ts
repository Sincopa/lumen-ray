import { isTauri } from '@tauri-apps/api/core';
export const runtime = {
  isDev: import.meta.env.DEV,
  isTauri: isTauri(),
  isBrowser: !isTauri(),
  useMocks: !isTauri(),
};
