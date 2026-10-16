<script lang="ts">
	import LumenFace from '../../components/LumenFace.svelte';
	import { shaderSettings } from '../../stores/shaderSettings.svelte';
	import { t } from '../../i18n';
	import { fade } from 'svelte/transition';
	import { pop, rise } from '../../motion';
	import type { VpnConnectionState } from '../../types/models';

	interface Props {
		connectionState: VpnConnectionState;
		onClose: () => void;
	}

	let { connectionState, onClose }: Props = $props();

	const MIN = 0.8;
	const MAX = 2.5;
	let progress = $derived((shaderSettings.exposure - MIN) / (MAX - MIN));
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') onClose();
	}}
/>

<!-- Figma 202:4917 — brightness of the background shader over the idle face. -->
<div
	class="brightness-view"
	data-tauri-drag-region
	in:fade={{ duration: 220 }}
	out:fade={{ duration: 160 }}
>
	<button
		type="button"
		class="back-btn"
		aria-label={t('common.back')}
		onclick={onClose}
	>
		<svg
			width="32"
			height="32"
			viewBox="0 0 32 32"
			fill="none"
			aria-hidden="true"
		>
			<path
				d="M18.6 9.2 12.4 15.4a.85.85 0 0 0 0 1.2l6.2 6.2"
				stroke="#fff"
				stroke-width="1.6"
				stroke-linecap="round"
			/>
		</svg>
	</button>

	<div
		class="face"
		aria-hidden="true"
		in:pop|global={{ start: 0.92, duration: 360 }}
	>
		<LumenFace status={connectionState} />
	</div>

	<div class="slider-card" in:rise|global={{ y: 16, delay: 80, duration: 300 }}>
		<svg
			class="sun"
			width="32"
			height="32"
			viewBox="0 0 32 32"
			fill="none"
			aria-hidden="true"
		>
			<circle cx="16" cy="16" r="4" stroke="#fff" stroke-width="2.67" />
			<path
				d="M16 4v1.33M16 26.67V28M4 16h1.33M26.67 16H28M7.5 7.5l.95.95M23.55 23.55l.95.95M7.5 24.5l.95-.95M23.55 8.45l.95-.95"
				stroke="#fff"
				stroke-width="2.67"
				stroke-linecap="round"
			/>
		</svg>
		<div class="track" style="--progress: {progress}">
			<span class="fill"></span>
			<span class="thumb"></span>
			<input
				type="range"
				min={MIN}
				max={MAX}
				step="0.01"
				aria-label={t('settings.brightness')}
				value={shaderSettings.exposure}
				oninput={(e) =>
					shaderSettings.setExposure(
						parseFloat((e.target as HTMLInputElement).value),
					)}
			/>
		</div>
		<svg
			class="sun"
			width="32"
			height="32"
			viewBox="0 0 32 32"
			fill="none"
			aria-hidden="true"
		>
			<circle cx="16" cy="16" r="6.67" fill="#fff" />
			<path
				d="M16 1.33v2.67M16 28v2.67M1.33 16H4M28 16h2.67M5.63 5.63l1.89 1.89M24.48 24.48l1.89 1.89M5.63 26.37l1.89-1.89M24.48 7.52l1.89-1.89"
				stroke="#fff"
				stroke-width="2.67"
				stroke-linecap="round"
			/>
		</svg>
	</div>
</div>

<style>
	.brightness-view {
		position: absolute;
		inset: 0;
		z-index: 30;
	}

	.back-btn {
		position: absolute;
		top: 30px;
		left: 30px;
		z-index: 2;
		display: grid;
		place-items: center;
		width: 64px;
		height: 64px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: var(--design-glass);
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.back-btn:active {
		transform: scale(0.94);
	}

	/* Same face geometry as the connection screen (Figma 202:4868). */
	.face {
		position: absolute;
		top: calc(72px + (100% - 72px) * 0.3764 - 87.5px);
		left: calc(50% - 128px);
		width: 256px;
		height: 175px;
		pointer-events: none;
	}

	.slider-card {
		position: absolute;
		left: 30px;
		right: 30px;
		bottom: 30px;
		display: flex;
		align-items: center;
		gap: 13px;
		height: 72px;
		padding: 16px;
		border-radius: 123px;
		background: #00000029;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}
	.sun {
		flex: 0 0 32px;
	}

	.track {
		position: relative;
		flex: 1;
		min-width: 0;
		height: 32px;
		border-radius: 123px;
		background: #00000040;
		box-shadow: 0 0 25px #00000040;
		/* The thumb travels inside the track, so keep 8px of slack at both ends. */
		--thumb-x: calc(8px + (100% - 16px) * var(--progress));
	}
	.fill {
		position: absolute;
		top: 7px;
		bottom: 7px;
		left: 0;
		width: calc(var(--thumb-x) + 8px);
		border-radius: 123px;
		background: #ffffffbf;
		pointer-events: none;
	}
	.thumb {
		position: absolute;
		top: -4px;
		left: calc(var(--thumb-x) - 8px);
		width: 16px;
		height: 40px;
		border-radius: 123px;
		background: #fff;
		pointer-events: none;
		transition: transform 160ms var(--ease-spring);
	}
	.track:active .thumb {
		transform: scaleY(1.06);
	}
	.track input {
		position: absolute;
		inset: -4px 0;
		width: 100%;
		height: calc(100% + 8px);
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}

	@media (hover: hover) and (pointer: fine) {
		.back-btn:hover {
			background: #00000040;
		}
	}
</style>
