<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { rise, swap } from '../../lib/motion';
	import RollingNumber from '../../components/RollingNumber.svelte';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import LumenFace from '../../components/LumenFace.svelte';
	import HomeNotice from '../../components/HomeNotice.svelte';
	import { t } from '../../lib/i18n';
	import { serverDisplayName } from '../../lib/utils/serverDisplayName';
	import { tooltip } from '../../lib/actions/tooltip';
	import type {
		VpnConnectionState,
		VpnConnectionInfo,
	} from '../../lib/types/models';
	import type { Server } from '../../lib/services/catalog';
	import type { ErrorCode } from '../../lib/services/errors';
	import chevronIcon from '../../assets/chevron-up.svg';

	interface Props {
		server?: Server;
		busy: boolean;
		error: ErrorCode | null;
		onRetry?: () => void;
		connectionState: VpnConnectionState;
		info: VpnConnectionInfo | null;
		selectedCountry?: string;
		onToggle: () => void;
		onSelectServer: () => void;
		updateVersion?: string | null;
		updating?: boolean;
		onInstallUpdate?: () => void;
		subscriptionExpired?: boolean;
	}

	let {
		connectionState,
		info,
		server,
		busy,
		error,
		selectedCountry = 'fi',
		onToggle,
		onSelectServer,
		updateVersion = null,
		updating = false,
		onInstallUpdate,
		subscriptionExpired = false,
	}: Props = $props();

	let isConnected = $derived(connectionState === 'connected');
	let isConnecting = $derived(connectionState === 'connecting');

	let now = $state(Date.now());
	let elapsed = $derived(
		info ? Math.max(0, Math.floor((now - info.connectedAt) / 1000)) : 0,
	);
	let hours = $derived(Math.floor(elapsed / 3600));
	let minutes = $derived(Math.floor(elapsed / 60) % 60);
	let seconds = $derived(elapsed % 60);

	let visible = $state(true);
	let hovered = $state(false);
	let pressed = $state(false);
	// A button that turns busy swallows its own pointerup, so never stay "pressed" through it.
	$effect(() => {
		if (busy) pressed = false;
	});
	onMount(() => {
		const updateVisibility = () => {
			visible = !document.hidden;
		};
		updateVisibility();
		document.addEventListener('visibilitychange', updateVisibility);
		return () =>
			document.removeEventListener('visibilitychange', updateVisibility);
	});

	$effect(() => {
		if (!info || !visible) return;
		now = Date.now();
		const timer = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(timer);
	});
</script>

<div class="connection-view">
	{#if subscriptionExpired || updateVersion}
		<div class="notice-slot" in:rise={{ y: -8 }} out:fade={{ duration: 140 }}>
			{#if subscriptionExpired}
				<HomeNotice icon="bell" text={t('home.subscriptionExpired')} />
			{:else}
				<HomeNotice
					icon="update"
					text={t('home.updateAvailable')}
					actionLabel={t('home.updateAction')}
					actionDisabled={updating}
					onAction={onInstallUpdate}
				/>
			{/if}
		</div>
	{/if}

	<div class="center-container">
		<!-- Central Lumen Logo Button -->
		<button
			class="central-logo-btn"
			class:connected={isConnected}
			class:connecting={isConnecting}
			onclick={() => {
				hovered = false;
				onToggle();
			}}
			onpointerenter={(event) => {
				if (event.pointerType === 'mouse') hovered = true;
			}}
			onpointerdown={() => (pressed = true)}
			onpointerup={() => (pressed = false)}
			onpointercancel={() => (pressed = false)}
			onpointerleave={() => {
				hovered = false;
				pressed = false;
			}}
			disabled={busy || (!server && !isConnected)}
			aria-busy={busy}
			aria-label={isConnected
				? t('home.clickToDisconnect')
				: t('home.clickToConnect')}
			use:tooltip={isConnected
				? t('home.clickToDisconnect')
				: t('home.clickToConnect')}
		>
			<LumenFace status={connectionState} {hovered} {pressed} error={!!error} />
		</button>

		<!-- Accessible live status for assistive tech -->
		<span class="sr-only" role="status">{t('home.' + connectionState)}</span>

		<!-- Status and Timer in an absolute slot below button to prevent button displacement -->
		<div class="status-slot">
			{#if isConnecting || connectionState === 'disconnecting'}
				<p
					class="connection-status"
					aria-hidden="true"
					in:swap
					out:fade={{ duration: 120 }}
				>
					{t('home.' + connectionState)}
				</p>
			{:else if isConnected}
				<div
					class="timer"
					in:swap={{ y: 8, duration: 260 }}
					out:fade={{ duration: 120 }}
				>
					<span class="time-segment">
						<RollingNumber value={hours} />
						<span class="time-label">{t('home.hoursUnit')}</span>
					</span>
					<span class="time-separator"></span>
					<span class="time-segment">
						<RollingNumber value={minutes} minDigits={2} />
						<span class="time-label">{t('home.minutesUnit')}</span>
					</span>
					<span class="time-separator"></span>
					<span class="time-segment">
						<RollingNumber value={seconds} minDigits={2} />
						<span class="time-label">{t('home.secondsUnit')}</span>
					</span>
				</div>
			{/if}

			{#if error}
				<div
					class="connection-error"
					role="alert"
					in:rise={{ y: 6 }}
					out:fade={{ duration: 140 }}
				>
					{t('errors.' + error)}
				</div>
			{/if}
		</div>
	</div>

	<!-- Server card. Hidden while the subscription is expired. -->
	{#if !subscriptionExpired}
		<button
			transition:rise={{ y: 12 }}
			class="server-selector"
			aria-label={server ? serverDisplayName(server) : t('servers.empty')}
			onclick={onSelectServer}
		>
			<div class="server-info">
				<div class="flag-circle">
					<CountryFlag countryCode={selectedCountry} size={32} />
				</div>
				<div class="server-details">
					<span class="protocol">{info?.protocol || 'VLESS'}</span>
					<span class="latency">
						{#if typeof server?.latency === 'number'}
							<RollingNumber value={server.latency} />
							<span class="latency-unit">ms</span>
						{:else if server?.latency === 'timeout'}
							<span>{t('servers.timeout')}</span>
						{:else}
							<span>{server?.latency ?? '—'}</span>
						{/if}
					</span>
				</div>
			</div>

			<div class="server-arrow">
				<img
					src={chevronIcon}
					alt=""
					width="24"
					height="24"
					class="chevron-icon"
				/>
			</div>
		</button>
	{/if}
</div>

<style>
	.connection-view {
		position: relative;
		width: 100%;
		height: 100%;
		min-height: 0;
	}

	/* Notices sit under the header, centred. */
	.notice-slot {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		z-index: 2;
		display: flex;
		justify-content: center;
		padding: 0 26.5px;
	}

	/* The face starts 149px into the 628px content area. */
	.center-container {
		position: absolute;
		top: calc(37.64% - 87.5px);
		left: 0;
		right: 0;
		display: flex;
		justify-content: center;
		height: 175px;
	}

	.central-logo-btn {
		width: 256px;
		height: 175px;
		flex: 0 0 256px;
		padding: 0;
		border: 0;
		border-radius: 48px;
		background: transparent;
		transition: transform 0.16s var(--ease-spring);
	}

	.central-logo-btn:disabled {
		opacity: 1;
	}

	.central-logo-btn:active:not(:disabled) {
		transform: scale(0.97);
	}

	.status-slot {
		position: absolute;
		top: 197px;
		left: 0;
		right: 0;
		display: flex;
		justify-content: center;
		align-items: center;
		height: 32px;
		pointer-events: none;
	}

	.connection-status {
		margin: 0;
		color: rgba(0, 0, 0, 0.66);
		font-size: 14px;
		text-align: center;
	}

	.connection-error {
		max-width: 100%;
		padding: 0 16px;
		color: #601919;
		font-size: 13px;
		text-align: center;
	}

	/* Connection timer */
	.timer {
		display: flex;
		justify-content: center;
		align-items: baseline;
		gap: 2px;
		color: rgba(24, 22, 21, 0.84);
		font-size: 18px;
		font-weight: 500;
		white-space: nowrap;
	}

	.time-segment {
		display: inline-flex;
		align-items: baseline;
		gap: 1px;
	}

	.time-label {
		margin-left: 1px;
		font-size: 14px;
		opacity: 0.8;
	}

	.time-separator {
		width: 6px;
	}

	/* Server Selector Card */
	.server-selector {
		position: absolute;
		left: 16px;
		right: 16px;
		bottom: 114px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: calc(100% - 32px);
		height: 64px;
		padding: 16px;
		border: 0;
		border-radius: 124px;
		background: var(--design-glass);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			background-color 0.16s ease,
			transform 0.16s var(--ease-spring);
	}

	.server-selector:active {
		transform: scale(0.98);
	}

	.server-info {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
	}

	.flag-circle {
		flex: 0 0 32px;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		overflow: hidden;
	}

	.server-details {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 0;
	}

	.protocol {
		color: rgba(255, 255, 255, 0.75);
		font-size: 10px;
		font-weight: 400;
		line-height: 12px;
		text-transform: uppercase;
	}

	.latency {
		display: inline-flex;
		align-items: baseline;
		gap: 3px;
		color: #fff;
		font-size: 16px;
		font-weight: 500;
		line-height: 18px;
	}

	.latency-unit {
		font-size: 16px;
	}

	.server-arrow {
		display: flex;
		align-items: center;
	}

	.chevron-icon {
		width: 28px;
		height: 28px;
		transform: rotate(90deg);
	}

	@media (hover: hover) and (pointer: fine) {
		.server-selector:hover {
			background: rgba(0, 0, 0, 0.25);
		}
	}

	@media (max-height: 580px) {
		.center-container {
			top: 0;
			transform: scale(0.72);
			transform-origin: top;
		}

		.status-slot {
			top: 175px;
		}

		.server-selector {
			bottom: 100px;
		}
	}

	@media (max-height: 430px) {
		.center-container {
			transform: scale(0.55);
		}

		.status-slot {
			top: 145px;
		}

		.server-selector {
			height: 52px;
			bottom: 92px;
		}
	}
</style>
