import { mount } from 'svelte';
import './assets/fonts.css';
import './styles/tokens.css';

import App from './app/App.svelte';
import { i18n } from './lib/i18n';
import { APP_WIDTH, APP_HEIGHT } from './lib/constants/app';
import { isAndroid, isMac } from './lib/platform';
import { paceAnimationFrames } from './lib/platform/framePacing';

// Keep CSS custom properties in sync with the TS constants.
const root = document.documentElement;
// Android fills the screen; the desktop window keeps its designed size.
root.style.setProperty('--app-width', isAndroid ? '100vw' : `${APP_WIDTH}px`);
root.style.setProperty(
	'--app-height',
	isAndroid ? '100dvh' : `${APP_HEIGHT}px`,
);
// macOS draws the window frame, corners and traffic lights natively.
if (isMac) root.dataset.platform = 'macos';
if (isAndroid) {
	root.dataset.platform = 'android';
	paceAnimationFrames();
}

document.documentElement.lang = i18n.currentLanguage;

const app = mount(App, {
	target: document.getElementById('app')!,
});

export default app;
