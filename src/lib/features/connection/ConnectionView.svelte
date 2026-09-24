<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import NumberFlow from '@number-flow/svelte';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import { t } from '../../i18n';
	import { serverDisplayName } from '../../utils/serverDisplayName';
	import { tooltip } from '../../actions/tooltip';
	import type {
		VpnConnectionState,
		VpnConnectionInfo,
	} from '../../types/models';
	import type { Server } from '../../services/catalog';
	import type { ErrorCode } from '../../services/errors';
	import logoIcon from '../../../assets/logo.svg';
	import chevronIcon from '../../../assets/chevron.svg';

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
	<div class="center-container">
		<!-- Central Lumen Logo Button -->
		<button
			class="central-logo-btn"
			class:connected={isConnected}
			class:connecting={isConnecting}
			onclick={onToggle}
			disabled={busy || (!server && !isConnected)}
			aria-busy={busy}
			aria-label={isConnected
				? t('home.clickToDisconnect')
				: t('home.clickToConnect')}
			use:tooltip={isConnected
				? t('home.clickToDisconnect')
				: t('home.clickToConnect')}
		>
			<img src={logoIcon} alt="Lumen" class="giant-logo-img" />
		</button>

		<!-- Accessible live status for assistive tech -->
		<span class="sr-only" role="status">{t('home.' + connectionState)}</span>

		<!-- Status and Timer in an absolute slot below button to prevent button displacement -->
		<div class="status-slot">
			{#if isConnecting || connectionState === 'disconnecting'}
				<p
					class="connection-status"
					aria-hidden="true"
					transition:fade={{ duration: 240 }}
				>
					{t('home.' + connectionState)}
				</p>
			{:else if isConnected}
				<div class="timer" transition:fade={{ duration: 240 }}>
					<span class="time-segment">
						<NumberFlow value={hours} format={{ minimumIntegerDigits: 1 }} />
						<span class="time-label">{t('home.hoursUnit')}</span>
					</span>
					<span class="time-separator"></span>
					<span class="time-segment">
						<NumberFlow value={minutes} format={{ minimumIntegerDigits: 2 }} />
						<span class="time-label">{t('home.minutesUnit')}</span>
					</span>
					<span class="time-separator"></span>
					<span class="time-segment">
						<NumberFlow value={seconds} format={{ minimumIntegerDigits: 2 }} />
						<span class="time-label">{t('home.secondsUnit')}</span>
					</span>
				</div>
			{/if}

			{#if error}
				<div
					class="connection-error"
					role="alert"
					transition:fade={{ duration: 200 }}
				>
					{t('errors.' + error)}
				</div>
			{/if}
		</div>
	</div>

	<!-- Server Selector Card -->
	<button
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
						<NumberFlow value={server.latency} />
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
</div>

<style>
	.status-slot {
		position: absolute;
		top: calc(50% + 94px);
		left: 0;
		right: 0;
		display: flex;
		justify-content: center;
		align-items: center;
		pointer-events: none;
		height: 32px;
	}
	.connection-status {
		position: absolute;
		color: rgba(24, 22, 21, 0.72);
		font-size: 16px;
		margin: 0;
		min-height: 22px;
		text-align: center;
		white-space: nowrap;
	}
	.connection-error {
		position: absolute;
		color: #ffb9b9;
		text-align: center;
		font-size: 13px;
		white-space: nowrap;
	}
	.connection-view {
		display: flex;
		flex-direction: column;
		width: 100%;
		flex: 1;
		position: relative;
		justify-content: space-between;
		align-items: center;
		padding-top: 0;
	}

	.center-container {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		position: relative;
		width: 100%;
		transform: translateY(7px);
	}

	.central-logo-btn {
		width: 164px;
		height: 164px;
		flex: 0 0 164px;
		background: transparent;
		border: none;
		cursor: pointer;
		padding: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: filter 0.24s ease;
		border-radius: 50%;
	}

	.giant-logo-img {
		width: 164px;
		height: 164px;
		object-fit: contain;
		filter: brightness(0.15);
		transform: scale(0.7561);
		will-change: transform, filter, opacity;
		transition:
			filter 0.35s ease,
			transform 420ms var(--ease-spring),
			opacity 240ms ease;
	}

	.central-logo-btn:hover .giant-logo-img {
		filter: brightness(0.1);
		transform: scale(0.79);
	}

	.central-logo-btn:active .giant-logo-img {
		transform: scale(0.73);
		transition-duration: 120ms;
	}

	.central-logo-btn.connected .giant-logo-img {
		transform: scale(1);
		filter: brightness(0.12);
		animation: logoPulse 3s infinite ease-in-out;
	}

	.central-logo-btn.connecting .giant-logo-img {
		transform: scale(1);
		animation: connectingPulse 1.6s infinite ease-in-out;
	}

	.central-logo-btn:is(.connected, .connecting):hover .giant-logo-img {
		transform: scale(1.02);
	}

	.central-logo-btn:is(.connected, .connecting):active .giant-logo-img {
		transform: scale(0.98);
	}

	@keyframes connectingPulse {
		0%,
		100% {
			opacity: 0.7;
		}
		50% {
			opacity: 1;
		}
	}

	@keyframes logoPulse {
		0%,
		100% {
			filter: brightness(0.16);
		}
		50% {
			filter: brightness(0.08);
		}
	}

	/* Timer with NumberFlow */
	.timer {
		position: absolute;
		margin-top: 0;
		display: flex;
		justify-content: center;
		align-items: baseline;
		gap: 2px;
		font-family: var(--font-family-base);
		font-size: 18px;
		font-weight: 500;
		color: rgba(24, 22, 21, 0.84);
		letter-spacing: -0.2px;
		white-space: nowrap;
	}

	.time-segment {
		display: inline-flex;
		align-items: baseline;
		gap: 1px;
	}

	.time-label {
		font-size: 14px;
		opacity: 0.8;
		margin-left: 1px;
	}

	.time-separator {
		width: 6px;
		display: inline-block;
	}

	/* Server Selector Card */
	.server-selector {
		width: 100%;
		max-width: 326px;
		height: 64px;
		background: rgba(0, 0, 0, 0.2);
		border: none;
		outline: none;
		box-shadow: none;
		border-radius: 124px;
		padding: 0 20px 0 12px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		box-sizing: border-box;
		backdrop-filter: blur(14px);
		cursor: pointer;
		transition:
			background 0.22s ease,
			border-color 0.22s ease,
			transform 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
		margin-top: auto;
		margin-bottom: 124px;
	}

	.server-selector:hover {
		background: rgba(255, 255, 255, 0.09);
		transform: scale(1.005);
	}

	.server-selector:active {
		transform: scale(0.98);
		transition-duration: 0.1s;
	}

	.server-info {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.flag-circle {
		width: 36px;
		height: 36px;
		border-radius: 50%;
		overflow: hidden;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.server-details {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
	}

	.protocol {
		font-family: var(--font-family-base);
		font-size: 11px;
		font-weight: 700;
		color: rgba(255, 255, 255, 0.65);
		letter-spacing: 0.5px;
		text-transform: uppercase;
	}

	.latency {
		font-family: var(--font-family-base);
		font-size: 16px;
		font-weight: 500;
		color: #ffffff;
		display: inline-flex;
		align-items: baseline;
	}

	.latency-unit {
		font-size: 14px;
		opacity: 0.85;
	}

	.server-arrow {
		display: flex;
		align-items: center;
	}

	.chevron-icon {
		font-size: 20px !important;
		color: rgba(255, 255, 255, 0.6);
		transition:
			color 0.2s ease,
			transform 0.2s ease;
		font-variation-settings:
			'FILL' 0,
			'wght' 300,
			'GRAD' 0,
			'opsz' 24;
		opacity: 0.6;
		transform: rotate(-90deg);
		filter: brightness(0) invert(1);
		transition:
			opacity 0.2s ease,
			transform 0.2s ease;
	}

	.server-selector:hover .chevron-icon {
		color: rgba(255, 255, 255, 1);
		transform: translateX(2px);
		opacity: 1;
		transform: rotate(-90deg) translateX(-2px);
	}

	@media (prefers-reduced-motion: reduce) {
		.central-logo-btn,
		.server-selector,
		.giant-logo-img {
			animation: none;
			transition-duration: 0ms;
		}
	}
</style>
