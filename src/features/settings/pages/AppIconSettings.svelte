<script lang="ts">
	import { t } from '../../../lib/i18n';
	import { nativeSettings } from '../../../lib/services/settings';
	import {
		APP_ICONS,
		DEFAULT_APP_ICON,
		isAppIcon,
		setAppIcon,
		type AppIconId,
	} from '../../../lib/services/appIcon';
	import SettingHint from '../ui/SettingHint.svelte';

	const current = $derived(
		isAppIcon($nativeSettings.appIcon)
			? $nativeSettings.appIcon
			: DEFAULT_APP_ICON,
	);
	let busy = $state(false);
	let error = $state('');
	/** The tile the user just chose springs into place once. */
	let picked = $state<AppIconId | null>(null);

	async function pick(icon: AppIconId) {
		if (busy || icon === current) return;
		picked = icon;
		busy = true;
		const code = await setAppIcon(icon);
		error = code ? t('errors.' + code) : '';
		busy = false;
	}
</script>

<!-- Figma "Внешний вид приложения": three 102px tiles a row; the current
     icon at full strength, the others at half. -->
<div class="settings-group app-icon-card">
	<div class="info-row">
		<span class="info-label">{t('settings.appIcon')}</span>
	</div>
	<div class="icon-grid" role="radiogroup" aria-label={t('settings.appIcon')}>
		{#each APP_ICONS as icon (icon.id)}
			<button
				type="button"
				class="icon-tile"
				class:current={icon.id === current}
				class:picked={icon.id === picked && icon.id === current}
				role="radio"
				aria-checked={icon.id === current}
				aria-label={t('settings.appIcon_' + icon.id)}
				disabled={busy}
				onclick={() => pick(icon.id)}
			>
				<img src={icon.preview} alt="" draggable="false" />
			</button>
		{/each}
	</div>
</div>
<SettingHint text={error} />

<style>
	.app-icon-card {
		gap: 16px;
		padding: 18px;
	}
	.app-icon-card > :global(* + *) {
		border-top: none;
	}
	.icon-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 8px;
	}
	/* 18px tiles with a 15% white rim drawn over the artwork (Figma: a 1px
	   inside stroke). The rim is a border, which the browser snaps to a whole
	   device pixel at any UI scale; a 1px inset shadow thinned out to nothing
	   under the window zoom. */
	.icon-tile {
		position: relative;
		aspect-ratio: 1;
		min-height: 0;
		padding: 0;
		border: 0;
		border-radius: 18px;
		overflow: hidden;
		background: #ffffff0d;
		opacity: 0.5;
		cursor: pointer;
		transition:
			opacity 200ms ease,
			transform 200ms var(--ease-spring);
	}
	.icon-tile::after {
		content: '';
		position: absolute;
		inset: 0;
		border: 1px solid #ffffff26;
		border-radius: inherit;
		pointer-events: none;
	}
	.icon-tile img {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: cover;
		user-select: none;
	}
	.icon-tile.current {
		opacity: 1;
	}
	/* A small press-and-settle on the morph spring, like picking an icon on iOS. */
	.icon-tile.picked {
		animation: tile-pick 560ms var(--ease-morph);
	}
	@keyframes tile-pick {
		0% {
			transform: scale(0.92);
		}
		100% {
			transform: scale(1);
		}
	}
	.icon-tile:disabled {
		cursor: progress;
	}
	.icon-tile:active:not(:disabled) {
		transform: scale(0.96);
	}
	@media (hover: hover) and (pointer: fine) {
		.icon-tile:not(.current):hover:not(:disabled) {
			opacity: 0.8;
		}
	}
</style>
