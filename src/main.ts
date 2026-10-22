import { mount } from 'svelte';
import './assets/fonts.css';
import './styles/tokens.css';

import App from './app/App.svelte';
import { i18n } from './lib/i18n';
import { APP_WIDTH, APP_HEIGHT } from './lib/constants/app';
import { isMac } from './lib/platform';

// Keep CSS custom properties in sync with the TS constants.
const root = document.documentElement;
root.style.setProperty('--app-width', `${APP_WIDTH}px`);
root.style.setProperty('--app-height', `${APP_HEIGHT}px`);
// macOS draws the window frame, corners and traffic lights natively.
if (isMac) root.dataset.platform = 'macos';

document.documentElement.lang = i18n.currentLanguage;

const app = mount(App, {
	target: document.getElementById('app')!,
});

export default app;
