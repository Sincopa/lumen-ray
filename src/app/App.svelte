<script lang="ts">
	import { fade } from 'svelte/transition';
	import { screen, rise, leaveInPlace } from '../lib/motion';
	import BackgroundShader from './chrome/BackgroundShader.svelte';
	import WindowHeader from './chrome/WindowHeader.svelte';
	import BottomNav from './chrome/BottomNav.svelte';
	import TabHeader from './chrome/TabHeader.svelte';
	import ConnectionView from '../features/connection/ConnectionView.svelte';
	import EmptyHomeView from '../features/connection/EmptyHomeView.svelte';
	import BrightnessView from '../features/connection/BrightnessView.svelte';
	import SubscriptionsView from '../features/subscriptions/SubscriptionsView.svelte';
	import ServersView from '../features/servers/ServersView.svelte';
	import SettingsView from '../features/settings/SettingsView.svelte';
	import RulesView from '../features/rules/RulesView.svelte';
	import AddProfileView from '../features/subscriptions/AddProfileView.svelte';
	import { catalog, connection } from '../lib/services';
	import { t } from '../lib/i18n';
	import { runtime } from '../lib/runtime';
	import { applicationUpdate } from '../lib/services/applicationUpdate';
	import { shell } from './shell.svelte';
	import { tabHeader } from './tabHeader.svelte';
	import { installLifecycle } from './lifecycle.svelte';

	installLifecycle();

	const connectionStore = connection.state;
	const catalogStore = catalog.state;
	const persistenceError = catalog.persistenceError;
	const applicationUpdateState = applicationUpdate.state;

	let connectionState = $derived($connectionStore.status);
	let backdropTone = $state<'light' | 'dark'>('dark');
	let info = $derived($connectionStore.info);
	let hasSubscriptions = $derived($catalogStore.subscriptions.length > 0);
	let routingMode = $derived($catalogStore.routingMode);
	let selectedServer = $derived(
		$catalogStore.servers.find(
			(s) => s.id === (info?.serverId ?? $catalogStore.selectedServerId),
		),
	);
	let selectedCountry = $derived(selectedServer?.countryCode ?? 'fi');
	let selectedSubscription = $derived(
		$catalogStore.subscriptions.find(
			(s) => s.id === selectedServer?.subscriptionId,
		),
	);
	// Subscriptions, servers and rules share one header that stays put between them.
	let showTabHeader = $derived(
		!shell.importing &&
			(shell.currentTab === 'subscriptions' ||
				shell.currentTab === 'servers' ||
				(shell.currentTab === 'rules' && shell.ruleChrome === 'default')) &&
			!!tabHeader.slots &&
			!tabHeader.slots.hidden,
	);

	let subscriptionExpired = $derived(
		selectedSubscription?.status === 'expired' ||
			(typeof selectedSubscription?.daysRemaining === 'number' &&
				selectedSubscription.daysRemaining <= 0),
	);
</script>

<main class="app-viewport">
	<!-- Native WebGL2 Background Fluid Light Shader -->
	<BackgroundShader
		active={connectionState === 'connected'}
		onBackdropTone={(tone) => (backdropTone = tone)}
	/>

	{#if shell.brightnessOpen}
		<BrightnessView
			{connectionState}
			onClose={() => {
				// Brightness is opened from settings, so leaving it goes back there.
				shell.brightnessOpen = false;
				shell.goToTab('settings');
			}}
		/>
	{/if}

	<div
		class="app-ui design-screen"
		class:brightness-hidden={shell.brightnessOpen}
		class:dim-backdrop={shell.currentTab !== 'home' ||
			!hasSubscriptions ||
			shell.importing}
	>
		<!-- Top Desktop Window Bar -->
		<WindowHeader
			title="Lumen"
			showQuickActions={shell.currentTab !== 'settings' && !shell.importing}
			darkSurface={shell.currentTab !== 'home' ||
				!hasSubscriptions ||
				shell.importing}
			lightBackdrop={backdropTone === 'light'}
			isSettingsOpen={shell.currentTab === 'settings'}
			onOpenSettings={() => {
				shell.brightnessOpen = false;
				shell.goToTab(shell.currentTab === 'settings' ? 'home' : 'settings');
			}}
		/>

		<p class="demo-notice sr-only">{runtime.isTauri ? '' : t('demo.notice')}</p>

		{#if $persistenceError}
			<p role="alert" transition:rise>{t('errors.storage')}</p>
		{/if}

		{#if shell.actionError}
			<p role="alert" transition:rise>{shell.actionError}</p>
		{/if}

		{#if showTabHeader}
			<div
				class="tab-header-host"
				in:screen={{ dir: shell.direction(shell.currentTab, shell.prevTab) }}
				out:leaveInPlace
			>
				<TabHeader />
			</div>
		{/if}

		<!-- Tab Views with directional fly transitions -->
		<div class="view-content">
			{#if shell.importing}
				<div
					class="tab-view"
					in:screen={{ dir: 1 }}
					out:fade={{ duration: 120 }}
				>
					<AddProfileView
						initialSource={shell.pendingImport}
						onClose={() => {
							shell.importing = false;
							shell.pendingImport = '';
						}}
						onSuccess={() => {
							shell.importing = false;
							shell.pendingImport = '';
							shell.goToTab('home');
						}}
					/>
				</div>
			{:else}
				{#key shell.currentTab}
					<div
						class="tab-view"
						in:screen|global={{
							dir: shell.direction(shell.currentTab, shell.prevTab),
						}}
						out:fade|global={{ duration: 120 }}
					>
						{#if shell.currentTab === 'home'}
							{#if !hasSubscriptions}
								<EmptyHomeView />
							{:else}
								<ConnectionView
									{connectionState}
									{info}
									{selectedCountry}
									server={selectedServer}
									busy={$connectionStore.busy}
									error={$connectionStore.error}
									onRetry={() => connection.initialize()}
									onToggle={shell.toggleConnection}
									onSelectServer={() => shell.goToTab('servers')}
									updateVersion={$applicationUpdateState.version}
									updating={$applicationUpdateState.status === 'downloading'}
									onInstallUpdate={() => void applicationUpdate.install()}
									{subscriptionExpired}
								/>
							{/if}
						{:else if shell.currentTab === 'subscriptions'}
							<SubscriptionsView />
						{:else if shell.currentTab === 'servers'}
							<ServersView
								selectedId={$catalogStore.selectedServerId ?? ''}
								disabled={$connectionStore.busy}
								onSelect={shell.chooseServer}
								onManageSubscription={() => shell.goToTab('subscriptions')}
							/>
						{:else if shell.currentTab === 'settings'}
							<SettingsView
								onClose={() =>
									shell.goToTab(
										shell.prevTab === 'settings' ? 'home' : shell.prevTab,
									)}
								onOpenBrightness={() => {
									shell.brightnessOpen = true;
									shell.goToTab('home');
								}}
							/>
						{:else if shell.currentTab === 'rules'}
							<RulesView
								onChromeChange={(mode) => (shell.ruleChrome = mode)}
								onSelectionChange={(bar) => (shell.selectionBar = bar)}
							/>
						{/if}
					</div>
				{/key}
			{/if}
		</div>

		<!-- Bottom Navigation Bar -->
		{#if !shell.importing && shell.currentTab !== 'settings' && (shell.currentTab !== 'rules' || shell.ruleChrome !== 'picker')}
			<BottomNav
				currentTab={shell.currentTab}
				{routingMode}
				{hasSubscriptions}
				showAdd={shell.currentTab === 'subscriptions' ||
					(shell.currentTab === 'home' && subscriptionExpired)}
				onTabChange={shell.goToTab}
				onModeChange={shell.changeMode}
				selection={shell.currentTab === 'rules' &&
				shell.ruleChrome === 'selection'
					? shell.selectionBar
					: null}
				onActionClick={() => (shell.importing = true)}
			/>
		{/if}
	</div>
</main>

<style>
	.demo-notice {
		font-size: 12px;
		color: #d6c4ed;
		margin: 0 0 8px;
		text-align: center;
	}
	.app-viewport {
		position: relative;
		width: min(var(--app-width), 100vw);
		height: min(var(--app-height), 100dvh);
		display: flex;
		justify-content: center;
		align-items: center;
		overflow: hidden;
		background-color: #08080b;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		/* Scales the entire UI (text, spacing, icons) proportionally.
		   --app-width/height stay at their base values; the native window is resized
		   separately to accommodate the zoomed content. */
		zoom: var(--ui-zoom, 1);
	}

	.app-ui {
		position: relative;
		z-index: 10;
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: var(--app-width);
		min-width: 0;
		height: 100%;
		min-height: 0;
		padding: 16px 16px 0;
		box-sizing: border-box;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		transition: background-color 0.3s ease;
	}
	/* Trim text to cap height. Edge and WebView2 support the same geometry.
	   Ellipsis titles (.title-text, .sub-name) keep their line box: the clip
	   would cut ascenders and descenders. */
	.design-screen :global(:is(.logo, .metric-label, .metric-val, .updated-time)),
	.design-screen :global(.server-text > span),
	.design-screen :global(.action-pill > span) {
		text-box-trim: trim-both;
		text-box-edge: cap alphabetic;
	}

	.app-ui::before {
		content: '';
		position: absolute;
		inset: 0;
		background-color: rgba(54, 54, 54, 0.85);
		/* background-image: linear-gradient(
      to top,
      rgba(0, 0, 0, 0.25) 0px,
      rgba(0, 0, 0, 0) 260px
    ); */
		backdrop-filter: blur(12px);
		background-repeat: no-repeat;
		background-position: bottom;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		pointer-events: none;
		z-index: -1;
		opacity: 0;
		transition: opacity 0.35s cubic-bezier(0.4, 0, 0.2, 1);
	}

	.app-ui.dim-backdrop::before {
		opacity: 1;
	}
	.app-ui.design-screen::before {
		background: rgba(15, 15, 15, 0.95);
		backdrop-filter: none;
	}

	/* The brightness screen keeps the window header; everything else hides. */
	.app-ui.brightness-hidden {
		z-index: 40;
		pointer-events: none;
	}
	.app-ui.brightness-hidden > :global(:not(.window-header)) {
		visibility: hidden;
	}
	.app-ui.brightness-hidden > :global(.window-header) {
		pointer-events: auto;
	}

	.view-content {
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		overflow: hidden;
		position: relative;
		min-height: 0;
	}

	.tab-header-host {
		position: relative;
		z-index: 20;
		flex-shrink: 0;
		width: 100%;
	}

	.tab-view {
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		overflow: hidden;
		position: absolute;
		inset: 0;
	}
</style>
