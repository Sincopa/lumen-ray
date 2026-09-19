/**
 * App window dimensions — single source of truth.
 *
 * When you need to change the window size, update ONLY these two values.
 * They are automatically propagated to:
 *   - CSS custom properties (via tokens.css, which reads them via JS on mount)
 *   - App.svelte (.app-viewport / .app-ui styles use var(--app-width) / var(--app-height))
 *   - playwright.config.ts (imports APP_WIDTH / APP_HEIGHT)
 *
 * NOTE: src-tauri/tauri.conf.json must be updated manually — JSON does not
 * support variables. Keep the values there in sync with the constants below.
 */
export const APP_WIDTH = 400;
export const APP_HEIGHT = 700;

