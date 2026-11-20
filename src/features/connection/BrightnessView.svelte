<script lang="ts">
	import backIcon from '../../assets/icons/back.svg';
	import sunLowIcon from '../../assets/icons/sun.svg';
	import sunHighIcon from '../../assets/icons/sun-fill.svg';
	import { shaderSettings } from '../../lib/stores/shaderSettings.svelte';
	import { t } from '../../lib/i18n';
	import { fade } from 'svelte/transition';
	import { rise } from '../../lib/motion';

	interface Props {
		/** The shader under the title is bright, so the title turns dark. */
		lightBackdrop?: boolean;
		onClose: () => void;
	}

	let { lightBackdrop = false, onClose }: Props = $props();

	const MIN = 0.8;
	const MAX = 2.5;
	let progress = $derived((shaderSettings.exposure - MIN) / (MAX - MIN));
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') onClose();
	}}
/>

<!-- Figma "Яркость": the live shader with the brightness bar under the title. -->
<div
	class="brightness-view"
	class:light-backdrop={lightBackdrop}
	data-tauri-drag-region
	in:fade={{ duration: 220 }}
	out:fade={{ duration: 160 }}
>
	<div class="page-header" data-tauri-drag-region>
		<button
			type="button"
			class="back-btn"
			aria-label={t('common.back')}
			onclick={onClose}
		>
			<img src={backIcon} alt="" width="24" height="24" />
		</button>
		<span class="title">{t('settings.brightness')}</span>
	</div>

	<div class="slider-card" in:rise|global={{ y: -8, delay: 60, duration: 300 }}>
		<img src={sunLowIcon} alt="" class="sun" width="32" height="32" />
		<div class="track" style:--progress={progress}>
			<span class="track-fill"></span>
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
		padding: 0 16px;
	}

	/* The back button and the centred title, under the window header. */
	.page-header {
		position: absolute;
		top: 77px;
		left: 16px;
		right: 16px;
		display: flex;
		align-items: center;
		justify-content: center;
		height: 50px;
	}
	.back-btn {
		position: absolute;
		left: 0;
		top: 0;
		display: grid;
		place-items: center;
		width: 50px;
		height: 50px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: linear-gradient(#ffffff0d, #ffffff0d), #00000033;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		transition:
			background-color 160ms ease,
			transform 160ms var(--ease-spring);
	}
	.back-btn:active {
		transform: scale(0.94);
	}
	.title {
		color: #ffffff80;
		font-size: 14px;
		font-weight: 500;
		letter-spacing: -0.28px;
		transition: color 600ms ease;
		pointer-events: none;
	}
	.light-backdrop .title {
		color: #00000080;
	}

	/* 340x72, 24px corners: 2% white over 20% black, 18px padding. */
	.slider-card {
		position: absolute;
		top: 152px;
		left: 25px;
		right: 25px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		height: 72px;
		padding: 18px;
		border-radius: 24px;
		background: linear-gradient(#ffffff05, #ffffff05), #00000033;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
	}
	.sun {
		flex: 0 0 32px;
	}

	/* A 16px bar (6px corners on the left, round on the right) filling with
	   75% white; the 4x26 thumb rides its end. */
	.track {
		position: relative;
		flex: 1;
		min-width: 0;
		height: 26px;
		--fill: calc(4px + (100% - 4px) * var(--progress));
	}
	.track::before,
	.track-fill {
		content: '';
		position: absolute;
		top: 5px;
		left: 0;
		height: 16px;
		border-radius: 6px 8px 8px 6px;
	}
	.track::before {
		right: 0;
		background: #00000080;
	}
	.track-fill {
		width: var(--fill);
		border-radius: 6px 0 0 6px;
		background: linear-gradient(#ffffffbf, #ffffffbf), #00000040;
	}
	.thumb {
		position: absolute;
		top: 0;
		left: calc(var(--fill) - 4px);
		width: 4px;
		height: 26px;
		border-radius: 123px;
		background: #fff;
		pointer-events: none;
		transition: transform 160ms var(--ease-spring);
	}
	.track:active .thumb {
		transform: scaleY(1.08);
	}
	.track input {
		position: absolute;
		inset: -6px 0;
		width: 100%;
		height: calc(100% + 12px);
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}

	@media (hover: hover) and (pointer: fine) {
		.back-btn:hover {
			background: linear-gradient(#ffffff1a, #ffffff1a), #00000033;
		}
	}
</style>
