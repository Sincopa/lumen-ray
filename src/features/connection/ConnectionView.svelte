<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { pop, rise } from '../../lib/motion';
	import RollingNumber from '../../components/RollingNumber.svelte';
	import Morph from '../../components/Morph.svelte';
	import CountryFlag from '../../components/CountryFlag.svelte';
	import LumenFace from '../../components/LumenFace.svelte';
	import HomeNotice from '../../components/HomeNotice.svelte';
	import { t } from '../../lib/i18n';
	import { serverDisplayName } from '../../lib/utils/serverDisplayName';
	import { tooltip } from '../../lib/actions/tooltip';
	import { canCancel } from '../../lib/services/connection';
	import type {
		VpnConnectionState,
		VpnConnectionInfo,
	} from '../../lib/types/models';
	import type { Server } from '../../lib/services/catalog';
	import type { ErrorCode } from '../../lib/services/errors';
	import chevronIcon from '../../assets/icons/chevron-right.svg';
	import infoIcon from '../../assets/icons/info.svg';
	import closeIcon from '../../assets/icons/close.svg';

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
		/** Share of the update downloaded (0-1), null while unknown. */
		updateProgress?: number | null;
		onInstallUpdate?: () => void;
		/** Hides the error notice before its timer does. */
		onDismissError?: () => void;
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
		updateProgress = null,
		onInstallUpdate,
		onDismissError,
		subscriptionExpired = false,
	}: Props = $props();

	// Figma error notice: the first sentence is the title, the rest explains it.
	// A short message is a single pill; a longer one a card with a countdown
	// to its own dismissal (the controller clears errors after 5 s) and a cross.
	const errorText = $derived(error ? t('errors.' + error) : '');
	const errorParts = $derived.by(() => {
		const match = /^(.+?[.:!?])\s+(.+)$/s.exec(errorText);
		return match
			? {
					title: match[1].replace(/:$/, '.'),
					detail: match[2].replace(/^./, (c) => c.toUpperCase()),
				}
			: { title: errorText, detail: '' };
	});
	const errorAutoHides = $derived(connectionState !== 'error');
	let errorSeconds = $state(5);
	$effect(() => {
		if (!error || !errorAutoHides) return;
		const shownAt = Date.now();
		errorSeconds = 5;
		const timer = setInterval(() => {
			errorSeconds = Math.max(1, 5 - Math.floor((Date.now() - shownAt) / 1000));
		}, 250);
		return () => clearInterval(timer);
	});

	let isConnected = $derived(connectionState === 'connected');
	let isConnecting = $derived(connectionState === 'connecting');
	// While a connection is on its way up, the button cancels it.
	let cancellable = $derived(canCancel(connectionState));
	let actionLabel = $derived(
		cancellable
			? t('home.clickToCancel')
			: isConnected
				? t('home.clickToDisconnect')
				: t('home.clickToConnect'),
	);
	let statusKey = $derived(
		isConnecting || connectionState === 'disconnecting'
			? connectionState
			: isConnected
				? 'timer'
				: 'idle',
	);

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
	let disabled = $derived(
		cancellable ? false : busy || (!server && !isConnected),
	);
	// A button that turns disabled swallows its own pointerup, so never stay "pressed" through it.
	$effect(() => {
		if (disabled) pressed = false;
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
			<Morph
				key={subscriptionExpired ? 'expired' : updating ? 'progress' : 'notice'}
			>
				{#if subscriptionExpired}
					<HomeNotice icon="bell" text={t('home.subscriptionExpired')} />
				{:else if updating}
					<!-- Figma: the notice turns into a progress bar while it downloads. -->
					<div
						class="update-progress"
						class:indeterminate={updateProgress === null}
						role="progressbar"
						aria-label={t('home.updating')}
						aria-valuemin={0}
						aria-valuemax={100}
						aria-valuenow={updateProgress === null
							? undefined
							: Math.round(updateProgress * 100)}
					>
						<span class="update-track">
							<span
								class="update-fill"
								style:width="{Math.max(8, (updateProgress ?? 0.35) * 100)}%"
							></span>
						</span>
					</div>
				{:else}
					<HomeNotice
						icon="update"
						text={t('home.updateAvailable')}
						actionLabel={t('home.updateAction')}
						actionDisabled={updating}
						onAction={onInstallUpdate}
					/>
				{/if}
			</Morph>
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
			{disabled}
			aria-busy={busy}
			aria-label={actionLabel}
			use:tooltip={actionLabel}
		>
			<LumenFace status={connectionState} {hovered} {pressed} error={!!error} />
		</button>

		<!-- Accessible live status for assistive tech -->
		<span class="sr-only" role="status">{t('home.' + connectionState)}</span>

		<!-- Status and Timer in an absolute slot below button to prevent button displacement -->
		<div class="status-slot">
			<!-- "Connecting…", the timer and "Disconnecting…" morph into each other. -->
			<div class="status-capsule" class:empty={statusKey === 'idle'}>
				<Morph key={statusKey}>
					{#if statusKey === 'connecting' || statusKey === 'disconnecting'}
						<p class="connection-status" aria-hidden="true">
							{t('home.' + statusKey)}
						</p>
					{:else if statusKey === 'timer'}
						<div class="timer">
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
				</Morph>
			</div>
		</div>
	</div>

	{#if error}
		<div
			class="error-notice"
			class:detailed={!!errorParts.detail}
			role="alert"
			in:pop={{ start: 0.94, duration: 320 }}
			out:pop={{ start: 0.96, duration: 160 }}
		>
			<img
				class="error-icon"
				src={infoIcon}
				alt=""
				width={errorParts.detail ? 24 : 16}
				height={errorParts.detail ? 24 : 16}
			/>
			{#if errorParts.detail}
				<div class="error-copy">
					<span class="error-title">{errorParts.title}</span>
					<span class="error-detail">{errorParts.detail}</span>
				</div>
				<div class="error-controls">
					{#if errorAutoHides}
						<span class="error-countdown">
							<RollingNumber value={errorSeconds} />{t('home.secondsShort')}
						</span>
					{/if}
					<button
						type="button"
						class="error-close"
						aria-label={t('common.close')}
						onclick={() => onDismissError?.()}
					>
						<img src={closeIcon} alt="" width="18" height="18" />
					</button>
				</div>
			{:else}
				<span class="error-title">{errorParts.title}</span>
			{/if}
		</div>
	{/if}

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
				<img src={chevronIcon} alt="" width="24" height="24" />
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

	/* Plain text: only the words dissolve into each other. */
	.status-capsule {
		display: flex;
		align-items: center;
		transition: opacity 240ms ease;
	}
	.status-capsule.empty {
		opacity: 0;
	}

	.connection-status {
		margin: 0;
		white-space: nowrap;
		color: rgba(0, 0, 0, 0.66);
		font-size: 14px;
		text-align: center;
	}

	/* Figma error notice: 50% black under a 20% red, blurred. A single line is
	   a 40px pill; with an explanation it becomes a 24px card. */
	.error-notice {
		position: absolute;
		left: 50%;
		bottom: 206px;
		z-index: 3;
		display: flex;
		align-items: center;
		gap: 10px;
		max-width: calc(100% - 32px);
		height: 40px;
		padding: 12px 16px;
		border-radius: 24px;
		background: linear-gradient(#ff4b5a33, #ff4b5a33), #00000080;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transform: translateX(-50%);
		color: #fff;
	}
	.error-notice.detailed {
		align-items: flex-start;
		gap: 8px;
		width: calc(100% - 32px);
		height: auto;
		padding: 16px 12px;
	}
	.error-icon {
		flex-shrink: 0;
		display: block;
	}
	.error-title {
		font-size: 14px;
		line-height: 14px;
		letter-spacing: -0.28px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.error-copy {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
		flex: 1;
		padding-top: 5px;
	}
	.detailed .error-title {
		font-weight: 500;
		white-space: normal;
	}
	.error-detail {
		color: #ffffffbf;
		font-size: 12px;
		line-height: 1.2;
		user-select: text;
	}
	/* "5с" and the cross share a 5% chip. */
	.error-controls {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-shrink: 0;
		height: 22px;
		margin-top: -4px;
		padding: 2px 2px 2px 8px;
		border-radius: 123px;
		background: #ffffff0d;
	}
	.error-countdown {
		display: inline-flex;
		color: #ffffffbf;
		font-size: 12px;
		line-height: 1.2;
		letter-spacing: -0.24px;
		font-variant-numeric: tabular-nums;
	}
	.error-close {
		display: grid;
		place-items: center;
		width: 18px;
		height: 18px;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: #ffffff26;
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.error-close img {
		display: block;
	}
	.error-close:active {
		transform: scale(0.9);
	}
	@media (hover: hover) and (pointer: fine) {
		.error-close:hover {
			background: #ffffff40;
		}
	}

	/* Figma: a 141x16 capsule holding an 8px track that fills white. */
	.update-progress {
		width: 141px;
		height: 16px;
		padding: 4px;
		border-radius: 18px;
		background: #00000026;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}
	.update-track {
		display: block;
		height: 8px;
		border-radius: 18px;
		background: #00000026;
		overflow: hidden;
	}
	.update-fill {
		display: block;
		height: 100%;
		border-radius: 18px;
		background: #fff;
		transition: width 300ms var(--ease-out);
	}
	.indeterminate .update-fill {
		animation: update-wait 1.4s ease-in-out infinite alternate;
	}
	@keyframes update-wait {
		from {
			transform: translateX(-60%);
		}
		to {
			transform: translateX(220%);
		}
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
		height: 66px;
		padding: 16px 20px 16px 16px;
		border: 1px solid rgba(0, 0, 0, 0.1);
		border-radius: 24px;
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
		/* Figma's inside stroke, drawn over the flag. */
		outline: 1px solid rgba(0, 0, 0, 0.25);
		outline-offset: -1px;
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

		.error-notice {
			bottom: 160px;
		}
	}
</style>
