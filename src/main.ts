import { mount } from 'svelte';
import './assets/satoshi.css';
import './styles/tokens.css';

import App from './app/App.svelte';
import { i18n } from './lib/i18n';

document.documentElement.lang = i18n.currentLanguage;

const app = mount(App, {
  target: document.getElementById('app')!,
});

export default app;
