/**
 * App window dimensions — single source of truth.
 *
 * App.svelte resizes the native window to these dimensions after startup,
 * including the user's UI scale. playwright.config.ts uses the same values.
 * Keep the CSS fallback dimensions in styles/tokens.css in sync.
 */
export const APP_WIDTH = 390;
export const APP_HEIGHT = 700;
