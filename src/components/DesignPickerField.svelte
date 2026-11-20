<script lang="ts">
	import type { Snippet } from 'svelte';
	import chevronIcon from '../assets/icons/chevron-right.svg';

	interface Props {
		/** "Выбрать сервер": the prompt while empty, a small caption once chosen. */
		label: string;
		chosen: boolean;
		/** 32px round flag or badge in front of a chosen value. */
		media?: Snippet;
		/** The chosen value, shown under the caption. */
		children?: Snippet;
		/** sm: a 12px value (process count), md: a 14px value next to the media. */
		size?: 'sm' | 'md';
		ariaLabel?: string;
		onclick: () => void;
	}

	let {
		label,
		chosen,
		media,
		children,
		size = 'sm',
		ariaLabel,
		onclick,
	}: Props = $props();

	const withMedia = $derived(chosen && !!media);
</script>

<!-- Figma Component 58 selector: the prompt shrinks into a caption above the
     chosen value, 300 ms ease-out like the prototype's Smart Animate. -->
<button
	type="button"
	class="picker-field {size}"
	class:chosen
	class:with-media={withMedia}
	aria-label={ariaLabel}
	{onclick}
>
	{#if media}
		<span class="picker-media" aria-hidden={!withMedia}>{@render media()}</span>
	{/if}
	<span class="picker-label">{label}</span>
	<span class="picker-value" aria-hidden={!chosen}>
		{#if chosen}{@render children?.()}{/if}
	</span>
	<img src={chevronIcon} alt="" class="picker-chevron" />
</button>

<style>
	.picker-field {
		position: relative;
		display: block;
		flex-shrink: 0;
		width: 100%;
		height: 60px;
		padding: 0;
		border: 0;
		border-radius: 24px;
		background-color: var(--design-panel);
		color: #fff;
		-webkit-backdrop-filter: blur(25px);
		backdrop-filter: blur(25px);
		text-align: left;
		box-sizing: border-box;
		transition: background-color var(--field-duration) var(--ease-field);
	}
	/* A made choice lifts the pill from 2% to 5%. */
	.picker-field.chosen {
		background-color: var(--design-panel-open);
	}
	@media (hover: hover) and (pointer: fine) {
		.picker-field:hover {
			background-color: #ffffff14;
		}
	}

	/* Geist centers its caps in a 1.3em line, so each line box is placed by
	   the center of its Figma cap box. Empty prompt: caps centered (y 25-35). */
	.picker-label,
	.picker-value {
		position: absolute;
		left: 24px;
		right: 54px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.picker-label {
		top: 20.9px;
		font-size: 14px;
		line-height: 18.2px;
		letter-spacing: -0.28px;
		transform-origin: left center;
		transition:
			transform var(--field-duration) var(--ease-field),
			color var(--field-duration) var(--ease-field);
	}
	/* Chosen: 12px caption at 25%, caps at y 18-27 (15-24 beside media). */
	.chosen .picker-label {
		color: #ffffff40;
		transform: translateY(-7.5px) scale(0.8571);
	}
	.with-media .picker-label {
		transform: translate(32px, -10.5px) scale(0.8571);
	}
	.picker-value {
		display: flex;
		align-items: center;
		gap: 6px;
		color: #fff;
		opacity: 0;
		transition: opacity var(--field-duration) var(--ease-field);
	}
	.chosen .picker-value {
		opacity: 1;
	}
	/* "17 Выбрано": 12px, caps at y 33-42. */
	.sm .picker-value {
		top: 29.7px;
		font-size: 12px;
		line-height: 15.6px;
		letter-spacing: -0.24px;
	}
	/* A server name: 14px, caps at y 35-45, after the 32px media and a 10px gap. */
	.md .picker-value {
		top: 30.9px;
		font-size: 14px;
		line-height: 18.2px;
	}
	.with-media .picker-value {
		left: 56px;
	}

	.picker-media {
		position: absolute;
		top: 14px;
		left: 14px;
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		opacity: 0;
		transition: opacity var(--field-duration) var(--ease-field);
	}
	.with-media .picker-media {
		opacity: 1;
	}

	.picker-chevron {
		position: absolute;
		top: 18px;
		right: 18px;
		width: 24px;
		height: 24px;
	}
</style>
