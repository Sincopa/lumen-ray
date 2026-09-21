<script lang="ts">
	import { onMount } from 'svelte';
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import BackgroundShader from '../lib/components/BackgroundShader.svelte';
	import WindowHeader from '../lib/components/WindowHeader.svelte';
	import BottomNav from '../lib/components/BottomNav.svelte';
	import ConnectionView from '../lib/features/connection/ConnectionView.svelte';
	import EmptyHomeView from '../lib/features/connection/EmptyHomeView.svelte';
	import SubscriptionsView from '../lib/features/subscriptions/SubscriptionsView.svelte';
	import ServersView from '../lib/features/servers/ServersView.svelte';
	import SettingsView from '../lib/features/settings/SettingsView.svelte';
	import RulesView from '../lib/features/rules/RulesView.svelte';
	import AddProfileView from '../lib/features/subscriptions/AddProfileView.svelte';
	import { catalog, connection } from '../lib/services';
	import { errorCode } from '../lib/services/errors';
	import { t } from '../lib/i18n';
	import { shaderSettings } from '../lib/stores/shaderSettings.svelte';
	import { runtime } from '../lib/runtime';
	import { APP_WIDTH, APP_HEIGHT } from '../lib/constants/app';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { LogicalSize } from '@tauri-apps/api/dpi';
	import { listen } from '@tauri-apps/api/event';
	import { nativeSettings } from '../lib/services/settings';

	type TabName = 'home' | 'subscriptions' | 'servers' | 'settings' | 'rules';

	const tabOrder: Record<TabName, number> = {
		home: 0,
		subscriptions: 1,
		servers: 2,
		rules: 3,
		settings: 4,
	};

	const connectionStore = connection.state;
	const catalogStore = catalog.state;
	const persistenceError = catalog.persistenceError;

	// Local state
	let currentTab = $state<TabName>('home');
	let prevTab = $state<TabName>('home');
	let importing = $state(false);
	let actionError = $state('');
	let ruleChrome = $state<'default' | 'selection' | 'picker'>('default');

	// Derived state
	let connectionState = $derived($connectionStore.status);
	let info = $derived($connectionStore.info);
	let hasSubscriptions = $derived($catalogStore.subscriptions.length > 0);
	let routingMode = $derived($catalogStore.routingMode);
	let selectedServer = $derived(
		$catalogStore.servers.find(
			(s) => s.id === (info?.serverId ?? $catalogStore.selectedServerId),
		),
	);
	let selectedCountry = $derived(selectedServer?.countryCode ?? 'fi');

	/**
	 * Applies the current uiScale:
	 * - Tauri: resizes the native window (390 × scale, 845 × scale).
	 *   The CSS width/height stay at 100vw/100vh so the UI fills the window.
	 *   CSS custom properties --app-width/--app-height are updated so
	 *   border-radius and clamp() calculations remain correct.
	 * - Browser dev-mode: CSS zoom on <html> as a visual approximation.
	 */
	async function applyScale(scale: number) {
		const w = Math.round(APP_WIDTH * scale);
		const h = Math.round(APP_HEIGHT * scale);

		// Set CSS zoom on .app-viewport — scales ALL UI elements proportionally
		// (text, padding, border-radius, icons — everything).
		// --app-width stays at 390px; zoom makes the 390px box visually larger.
		document.documentElement.style.setProperty('--ui-zoom', String(scale));

		if (runtime.isTauri) {
			// Resize the native window to match the zoomed dimensions so the
			// scaled content fits perfectly without clipping or empty space.
			try {
				console.log(`[scale] setSize → ${w}×${h} (scale=${scale})`);
				await getCurrentWindow().setSize(new LogicalSize(w, h));
				console.log(`[scale] setSize OK`);
			} catch (e) {
				console.error('[scale] Failed to resize Tauri window:', e);
			}
		} else {
			// Browser dev-mode: zoom the html element so the page shrinks/grows
			// without needing a native window resize.
			document.documentElement.style.zoom = scale === 1 ? '' : String(scale);
		}
	}

	// Reactively resize whenever uiScale changes
	$effect(() => {
		void applyScale(shaderSettings.uiScale);
	});

	async function refreshAndPingSubscriptions() {
		try {
			const sub = $catalogStore.subscriptions[0];
			if (sub) {
				await catalog.refreshSubscription(sub.id);
			}
			const servers = $catalogStore.servers;
			if (servers.length > 0) {
				await Promise.allSettled(
					servers.map((s) => catalog.pingServer(s.id).catch(() => 'timeout')),
				);
			}
		} catch (err) {
			console.error('[auto-refresh] Failed to refresh subscription or ping:', err);
		}
	}

	onMount(() => {
		void connection.initialize();
		void refreshAndPingSubscriptions();

		const handleContextMenu = (e: MouseEvent) => {
			e.preventDefault();
		};
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === 'Tab') {
				e.preventDefault();
			}
		};
		window.addEventListener('contextmenu', handleContextMenu);
		window.addEventListener('keydown', handleKeyDown);

		let unlistenNavigate: (() => void) | undefined;
		if (runtime.isTauri) {
			listen('navigate:settings', () => {
				currentTab = 'settings';
			}).then((unlisten) => {
				unlistenNavigate = unlisten;
			});
		}

		return () => {
			window.removeEventListener('contextmenu', handleContextMenu);
			window.removeEventListener('keydown', handleKeyDown);
			unlistenNavigate?.();
		};
	});

	$effect(() => {
		const hours = $nativeSettings.updateIntervalHours ?? 24;
		if (hours <= 0) return;
		const timer = setInterval(() => {
			void refreshAndPingSubscriptions();
		}, hours * 3600 * 1000);
		return () => {
			clearInterval(timer);
		};
	});

	// Action handlers
	const toggleConnection = () => {
		connection.toggle($catalogStore.selectedServerId);
	};

	async function chooseServer(server: { id: string }) {
		try {
			await catalog.selectServer(server.id);
			currentTab = 'home';
			if (connectionState === 'connected' || connectionState === 'connecting') {
				void connection.switchServer(server.id);
			}
		} catch (e) {
			actionError = t('errors.' + errorCode(e));
		}
	}

	async function changeMode(mode: 'global' | 'rules') {
		try {
			await catalog.setRoutingMode(mode);
		} catch (e) {
			actionError = t('errors.' + errorCode(e));
		}
	}

	function handleTabChange(tab: TabName) {
		prevTab = currentTab;
		currentTab = tab;
		if (tab !== 'rules') {
			ruleChrome = 'default';
		}
	}

	// Direction: +1 = slide right→left (going deeper), -1 = slide left→right (going back)
	function getDir(to: string, from: string) {
		return (tabOrder[to as TabName] ?? 0) > (tabOrder[from as TabName] ?? 0)
			? 1
			: -1;
	}
</script>

<main class="app-viewport">
	<!-- Native WebGL2 Background Fluid Light Shader -->
	<BackgroundShader active={connectionState === 'connected'} />

	<div
		class="app-ui"
		class:dim-backdrop={currentTab !== 'home' || !hasSubscriptions || importing}
	>
		<!-- Top Desktop Window Bar -->
		<WindowHeader
			title="lumen"
			darkSurface={currentTab !== 'home' || !hasSubscriptions || importing}
			isSettingsOpen={currentTab === 'settings'}
			onOpenSettings={() => {
				currentTab = currentTab === 'settings' ? 'home' : 'settings';
			}}
		/>

		<p class="demo-notice sr-only">{runtime.isTauri ? '' : t('demo.notice')}</p>

		{#if $persistenceError}
			<p role="alert">{t('errors.storage')}</p>
		{/if}

		{#if actionError}
			<p role="alert">{actionError}</p>
		{/if}

		<!-- Tab Views with directional fly transitions -->
		<div class="view-content">
			{#if importing}
				<AddProfileView
					onClose={() => (importing = false)}
					onSuccess={() => {
						importing = false;
						currentTab = 'home';
					}}
				/>
			{:else if currentTab === 'rules'}
				<RulesView onChromeChange={(mode) => (ruleChrome = mode)} />
			{:else}
				{#key currentTab}
					<div
						class="tab-view"
						in:fly={{
							x: getDir(currentTab, prevTab) * 24,
							duration: 260,
							easing: cubicOut,
							opacity: 0,
						}}
					>
						{#if currentTab === 'home'}
							{#if !hasSubscriptions}
								<EmptyHomeView onAddProfile={() => (importing = true)} />
							{:else}
								<ConnectionView
									{connectionState}
									{info}
									{selectedCountry}
									server={selectedServer}
									busy={$connectionStore.busy}
									error={$connectionStore.error}
									onRetry={() => connection.initialize()}
									onToggle={toggleConnection}
									onSelectServer={() => (currentTab = 'servers')}
								/>
							{/if}
						{:else if currentTab === 'subscriptions'}
							<SubscriptionsView />
						{:else if currentTab === 'servers'}
							<ServersView
								selectedId={$catalogStore.selectedServerId ?? ''}
								disabled={$connectionStore.busy}
								onSelect={chooseServer}
								onManageSubscription={() => handleTabChange('subscriptions')}
							/>
						{:else if currentTab === 'settings'}
							<SettingsView />
						{/if}
					</div>
				{/key}
			{/if}
		</div>

		<!-- Bottom Navigation Bar -->
		{#if !importing && (currentTab !== 'rules' || ruleChrome === 'default')}
			<BottomNav
				{currentTab}
				{routingMode}
				{hasSubscriptions}
				onTabChange={handleTabChange}
				onModeChange={changeMode}
				onActionClick={() => (importing = true)}
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
		height: min(var(--app-height), 100vh);
		display: flex;
		justify-content: center;
		align-items: center;
		overflow: hidden;
		background-color: #08080b;
		border-radius: var(--radius-window) var(--radius-window)
			var(--radius-window-bottom) var(--radius-window-bottom);
		/* Scales the entire UI (text, spacing, icons) proportionally.
		   --app-width/height stay at base 390×845; the native window is resized
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

	.view-content {
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
		overflow: hidden;
		position: relative;
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
