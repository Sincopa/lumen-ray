<script lang="ts">
	import backIcon from '../../assets/chevron-left.svg';
	import sunLowIcon from '../../assets/sun-low.svg';
	import sunHighIcon from '../../assets/sun-high.svg';
	import LumenFace from '../../components/LumenFace.svelte';
	import { shaderSettings } from '../../lib/stores/shaderSettings.svelte';
	import { t } from '../../lib/i18n';
	import { fade } from 'svelte/transition';
	import { pop, rise } from '../../lib/motion';
	import type { VpnConnectionState } from '../../lib/types/models';
	import reloadIcon from '../../assets/reload.svg';

	interface Props {
		connectionState: VpnConnectionState;
		onClose: () => void;
	}

	let { connectionState, onClose }: Props = $props();

	const MIN = 0.8;
	const MAX = 2.5;
	const DEFAULT_EXPOSURE = 1.6;
	const THUMB_HALF = 8;
	let progress = $derived((shaderSettings.exposure - MIN) / (MAX - MIN));

	// The track is a wedge that grows from 16px on the left to 32px on the right.
	let trackWidth = $state(208);
	const CY = 16;
	const halfHeight = (x: number) => 8 + (8 * x) / trackWidth;
	let thumbX = $derived(THUMB_HALF + (trackWidth - THUMB_HALF * 2) * progress);

	/** A wedge from 0 to `end` with round caps, shrunk by `inset` on every side. */
	function wedge(end: number, inset: number) {
		const left = halfHeight(0) - inset;
		const right = Math.max(left, halfHeight(end) - inset);
		const leftCenter = left + inset;
		const rightCenter = Math.max(leftCenter, end - right);
		return [
			`M ${leftCenter} ${CY - left}`,
			`L ${rightCenter} ${CY - right}`,
			`A ${right} ${right} 0 0 1 ${rightCenter} ${CY + right}`,
			`L ${leftCenter} ${CY + left}`,
			`A ${left} ${left} 0 0 1 ${leftCenter} ${CY - left}`,
			'Z',
		].join(' ');
	}

	let trackPath = $derived(wedge(trackWidth, 0));
	let fillPath = $derived(wedge(thumbX, 0.6));
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') onClose();
	}}
/>

<!-- Brightness of the background shader over the idle face. -->
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
		<img src={backIcon} alt="" width="32" height="32" />
	</button>

	<button
		type="button"
		class="reset-btn"
		aria-label={t('settings.brightnessReset')}
		disabled={shaderSettings.exposure === DEFAULT_EXPOSURE}
		onclick={() => shaderSettings.setExposure(DEFAULT_EXPOSURE)}
	>
		<img src={reloadIcon} alt="" width="32" height="32" />
	</button>

	<div
		class="face"
		aria-hidden="true"
		in:pop|global={{ start: 0.92, duration: 360 }}
	>
		<LumenFace status={connectionState} />
	</div>

	<div class="slider-card" in:rise|global={{ y: 16, delay: 80, duration: 300 }}>
		<img src={sunLowIcon} alt="" class="sun" width="32" height="32" />
		<div
			class="track"
			bind:clientWidth={trackWidth}
			style="--thumb-x: {thumbX}px"
		>
			<svg
				class="wedge"
				width={trackWidth}
				height="32"
				viewBox="0 0 {trackWidth} 32"
				aria-hidden="true"
			>
				<path class="wedge-track" d={trackPath} />
				<path d={fillPath} fill="#000" fill-opacity="0.25" />
				<path d={fillPath} fill="#fff" fill-opacity="0.75" />
			</svg>
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
		<img src={sunHighIcon} alt="" class="sun" width="32" height="32" />
	</div>
</div>

<style>
	.brightness-view {
		position: absolute;
		inset: 0;
		z-index: 30;
	}

	.back-btn,
	.reset-btn {
		position: absolute;
		/* Sits under the window header, which stays visible on this screen. */
		top: 72px;
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
	/* Lines up with the window controls above it, as in the design. */
	.reset-btn {
		left: auto;
		right: 16px;
	}
	.back-btn:active,
	.reset-btn:active:not(:disabled) {
		transform: scale(0.94);
	}
	.reset-btn:disabled {
		cursor: default;
	}

	/* Same face geometry as the connection screen. */
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
	}
	.wedge {
		position: absolute;
		inset: 0;
		overflow: visible;
		pointer-events: none;
	}
	.wedge-track {
		fill: #00000040;
		filter: drop-shadow(0 0 12.5px #00000040);
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
		.back-btn:hover,
		.reset-btn:hover:not(:disabled) {
			background: #00000040;
		}
	}
</style>
