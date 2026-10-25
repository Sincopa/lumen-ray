<script lang="ts">
	import { tick } from 'svelte';
	import addIcon from '../../assets/plus.svg';
	import pasteIcon from '../../assets/copy.svg';
	import { catalog } from '../../lib/services';
	import { errorCode } from '../../lib/services/errors';
	import { t } from '../../lib/i18n';

	let editing = $state(false);
	let source = $state('');
	let busy = $state(false);
	let error = $state('');
	let input = $state<HTMLInputElement | null>(null);

	async function open() {
		editing = true;
		error = '';
		await tick();
		input?.focus();
	}

	/** Fills the field from the clipboard; typing or Ctrl+V still works if it is blocked. */
	async function paste() {
		if (busy) return;
		try {
			const text = (await navigator.clipboard.readText()).trim();
			if (text) source = text;
		} catch {
			/* Clipboard access refused. */
		}
		input?.focus();
	}

	function close() {
		if (busy) return;
		editing = false;
		source = '';
		error = '';
	}

	async function submit() {
		if (busy || !source.trim()) return;
		busy = true;
		error = '';
		try {
			await catalog.importSubscription(source.trim());
		} catch (cause) {
			error = t('errors.' + errorCode(cause));
			input?.focus();
		} finally {
			busy = false;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			void submit();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			close();
		}
	}
</script>

<div class="empty-home-view">
	<div
		class="profile-control"
		class:editing
		class:busy
		class:filled={!!source.trim()}
	>
		<button
			class="add-profile-btn"
			class:hidden={editing}
			onclick={open}
			type="button"
			aria-hidden={editing}
			aria-label={t('subscriptions.addProfile')}
			tabindex={editing ? -1 : 0}
		>
			<img src={addIcon} alt="" class="plus-icon" />
			<span class="btn-label">{t('subscriptions.addLink')}</span>
		</button>

		<div class="source-editor" aria-hidden={!editing}>
			<button
				class="paste-btn"
				type="button"
				onclick={paste}
				disabled={busy}
				tabindex={editing ? 0 : -1}
				aria-label={t('subscriptions.pasteLink')}
			>
				<img src={pasteIcon} alt="" width="34" height="34" />
			</button>
			<input
				bind:this={input}
				bind:value={source}
				onkeydown={handleKeydown}
				type="url"
				inputmode="url"
				autocomplete="off"
				spellcheck="false"
				disabled={busy}
				tabindex={editing ? 0 : -1}
				aria-label={t('subscriptions.domainLink')}
				placeholder={t('subscriptions.addLink')}
			/>
			<button
				class="submit-link"
				type="button"
				onclick={submit}
				disabled={busy || !source.trim()}
				aria-label={t('subscriptions.importLink')}
			>
				<span aria-hidden="true">→</span>
			</button>
		</div>
	</div>

	{#if error}
		<p class="import-error" role="alert">{error}</p>
	{/if}
</div>

<style>
	.empty-home-view {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		flex: 1;
		min-height: 0;
		transform: translateY(-41.5px);
		user-select: none;
	}

	.profile-control {
		position: relative;
		width: min(100%, 300px);
		height: 63px;
		border-radius: 32px;
		background: var(--design-glass);
		-webkit-backdrop-filter: blur(16px);
		backdrop-filter: blur(16px);
		overflow: hidden;
		transition:
			width 360ms var(--ease-spring),
			background-color 220ms ease,
			box-shadow 220ms ease;
	}

	.profile-control:not(.editing) {
		width: min(100%, 229px);
	}

	/* Figma "Frame 2147223317": a faint pill that grows once a link is in. */
	.profile-control.editing {
		width: min(100%, 236px);
		background: #ffffff05;
		box-shadow: var(--design-edge);
	}
	.profile-control.editing.filled {
		width: min(100%, 300px);
	}

	.add-profile-btn,
	.source-editor {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}

	.add-profile-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 22px;
		padding: 0 26px;
		border: 0;
		background: transparent;
		color: #fff;
		transition:
			opacity 160ms ease,
			transform 240ms var(--ease-spring);
	}

	.add-profile-btn.hidden {
		opacity: 0;
		transform: scale(0.88);
		pointer-events: none;
	}

	.profile-control:not(.editing):hover {
		background: rgba(255, 255, 255, 0.12);
		box-shadow: 0 4px 20px rgba(0, 0, 0, 0.35);
	}

	.profile-control:not(.editing):active {
		transform: scale(0.97);
	}

	.plus-icon {
		width: 21px;
		height: 21px;
		object-fit: contain;
		filter: brightness(0) invert(1);
	}

	.btn-label {
		font-family: 'Satoshi', var(--font-family-base);
		font-size: 18px;
		font-weight: 400;
		color: rgba(255, 255, 255, 0.25);
		letter-spacing: -0.36px;
		white-space: nowrap;
	}

	.source-editor {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 0 7px;
		opacity: 0;
		transform: scale(1.08);
		pointer-events: none;
		transition:
			opacity 180ms ease 60ms,
			transform 320ms var(--ease-spring);
	}

	.editing .source-editor {
		opacity: 1;
		transform: scale(1);
		pointer-events: auto;
	}

	.paste-btn {
		display: grid;
		place-items: center;
		width: 50px;
		height: 50px;
		flex: 0 0 50px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		/* 5% glass with a light falling from the top. */
		background: linear-gradient(#ffffff1a, #99999900), #ffffff0d;
		transition:
			background-color 160ms ease,
			transform 180ms var(--ease-spring);
	}

	.paste-btn img {
		display: block;
	}

	.paste-btn:hover:not(:disabled) {
		background: linear-gradient(#ffffff1a, #99999900), #ffffff1a;
	}

	.paste-btn:active:not(:disabled) {
		transform: scale(0.94);
	}

	.paste-btn:disabled {
		opacity: 1;
	}

	.busy .paste-btn img {
		animation: importing 900ms ease-in-out infinite alternate;
	}

	.source-editor input {
		min-width: 0;
		width: 100%;
		height: 100%;
		padding: 0 !important;
		border: 0 !important;
		border-radius: 0 !important;
		outline: 0 !important;
		background: transparent !important;
		box-shadow: none !important;
		color: rgba(255, 255, 255, 0.9);
		font-family: var(--font-family-base);
		font-size: 18px;
		letter-spacing: -0.04em;
		user-select: text;
	}

	.source-editor input::placeholder {
		color: rgba(255, 255, 255, 0.5);
		opacity: 1;
	}

	.submit-link {
		width: 36px;
		height: 36px;
		min-width: 36px;
		padding: 0;
		border: 0;
		border-radius: 50%;
		background: #fff;
		color: #111;
		font-size: 20px;
		line-height: 1;
		transition:
			opacity 160ms ease,
			transform 180ms var(--ease-spring);
	}

	.submit-link:hover:not(:disabled) {
		transform: scale(1.06);
	}

	.submit-link:disabled {
		opacity: 0;
		pointer-events: none;
	}

	/* Until a link is in, the placeholder gets the whole pill, as in the design. */
	.profile-control:not(.filled) .submit-link {
		display: none;
	}

	.import-error {
		position: absolute;
		margin-top: 96px;
		max-width: 280px;
		color: #ffbcbc;
		font-size: 12px;
		line-height: 1.35;
		text-align: center;
	}

	@keyframes importing {
		to {
			transform: scale(0.72);
			opacity: 0.5;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.profile-control,
		.add-profile-btn,
		.source-editor,
		.paste-btn img {
			animation: none;
			transition-duration: 0ms;
		}
	}
</style>
