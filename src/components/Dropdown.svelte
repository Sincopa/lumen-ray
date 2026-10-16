<script lang="ts">
	import { tick } from 'svelte';
	import chevronIcon from '../assets/chevron-down.svg';

	type Option = { value: string; label: string };
	let {
		label,
		options,
		value = $bindable(''),
		disabled = false,
		compact = false,
		onChange,
	}: {
		label: string;
		options: Option[];
		value?: string;
		disabled?: boolean;
		compact?: boolean;
		onChange?: (value: string) => void;
	} = $props();

	const listId = `dropdown-${crypto.randomUUID()}`;
	let trigger: HTMLButtonElement;
	let list: HTMLDivElement;
	let open = $state(false);
	let wasOpenOnPointerDown = false;
	let selected = $derived(options.find((option) => option.value === value));

	function positionList() {
		if (!trigger || !list) return;
		const rect = trigger.getBoundingClientRect();
		const width = Math.min(
			Math.max(
				rect.width,
				compact && options.length > 2 ? 280 : Math.max(rect.width, 110),
			),
			window.innerWidth - 24,
		);
		const preferredHeight = Math.min(248, options.length * 40 + 8);
		const below = window.innerHeight - rect.bottom - 12;
		const above = rect.top - 12;
		const placeAbove = below < preferredHeight && above > below;
		const available = Math.max(80, placeAbove ? above : below);
		const height = Math.min(preferredHeight, available);
		const left = Math.max(
			12,
			Math.min(rect.right - width, window.innerWidth - width - 12),
		);
		list.style.left = `${left}px`;
		list.style.width = `${width}px`;
		list.style.top = `${placeAbove ? rect.top - height - 6 : rect.bottom + 6}px`;
		list.style.maxHeight = `${height}px`;
	}

	async function show(focusIndex?: number) {
		if (disabled || !options.length || !list) return;
		positionList();
		if (!list.matches(':popover-open')) {
			try {
				list.showPopover();
			} catch {
				// ignore if already shown
			}
		}
		if (focusIndex !== undefined) {
			await tick();
			const items = list.querySelectorAll<HTMLElement>('[role="option"]');
			items[focusIndex]?.focus();
		}
	}

	function choose(option: Option) {
		value = option.value;
		onChange?.(option.value);
		try {
			list?.hidePopover();
		} catch {
			/* The popover may already be closed or detached. */
		}
		trigger?.focus({ preventScroll: true });
	}

	function handleTriggerPointerDown() {
		if (disabled) return;
		wasOpenOnPointerDown = open;
	}

	function handleTriggerClick() {
		if (disabled) return;
		if (wasOpenOnPointerDown) {
			wasOpenOnPointerDown = false;
			return;
		}
		if (open) {
			try {
				list?.hidePopover();
			} catch {
				/* The popover may already be closed or detached. */
			}
		} else {
			void show();
		}
	}

	function onTriggerKeydown(event: KeyboardEvent) {
		if (disabled) return;
		const selectedIndex = Math.max(
			0,
			options.findIndex((option) => option.value === value),
		);
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			void show(
				event.key === 'ArrowDown'
					? Math.min(selectedIndex + 1, options.length - 1)
					: Math.max(selectedIndex - 1, 0),
			);
		} else if (event.key === 'Home' || event.key === 'End') {
			event.preventDefault();
			void show(event.key === 'Home' ? 0 : options.length - 1);
		}
	}

	function onOptionKeydown(event: KeyboardEvent, index: number) {
		const items = list.querySelectorAll<HTMLElement>('[role="option"]');
		let next: number;
		if (event.key === 'ArrowDown') next = Math.min(index + 1, items.length - 1);
		else if (event.key === 'ArrowUp') next = Math.max(index - 1, 0);
		else if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = items.length - 1;
		else if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			choose(options[index]);
			return;
		} else if (event.key === 'Escape') {
			event.preventDefault();
			try {
				list?.hidePopover();
			} catch {
				/* The popover may already be closed or detached. */
			}
			trigger?.focus({ preventScroll: true });
			return;
		} else return;
		event.preventDefault();
		items[next]?.focus();
	}

	$effect(() => {
		if (!open) return;

		function handleScroll(event: Event) {
			if (event.target !== list && !list?.contains(event.target as Node)) {
				try {
					list?.hidePopover();
				} catch {
					/* The popover may already be closed or detached. */
				}
			}
		}

		function handleResize() {
			try {
				list?.hidePopover();
			} catch {
				/* The popover may already be closed or detached. */
			}
		}

		window.addEventListener('scroll', handleScroll, {
			capture: true,
			passive: true,
		});
		window.addEventListener('resize', handleResize, { passive: true });

		return () => {
			window.removeEventListener('scroll', handleScroll, { capture: true });
			window.removeEventListener('resize', handleResize);
		};
	});
</script>

<button
	bind:this={trigger}
	class="dropdown-trigger"
	class:compact
	type="button"
	role="combobox"
	aria-label={label}
	aria-haspopup="listbox"
	aria-controls={listId}
	aria-expanded={open}
	{disabled}
	onpointerdown={handleTriggerPointerDown}
	onclick={handleTriggerClick}
	onkeydown={onTriggerKeydown}
>
	<span class="selected-label">{selected?.label ?? label}</span>
	<img
		src={chevronIcon}
		alt=""
		width="13"
		height="13"
		class="chevron-icon"
		aria-hidden="true"
	/>
</button>

<div
	bind:this={list}
	id={listId}
	class="dropdown-list"
	popover="auto"
	role="listbox"
	aria-label={label}
	ontoggle={(event) => (open = event.newState === 'open')}
>
	{#each options as option, index (option.value)}
		<div
			role="option"
			tabindex="-1"
			aria-selected={option.value === value}
			class:selected={option.value === value}
			onclick={() => choose(option)}
			onkeydown={(event) => onOptionKeydown(event, index)}
		>
			<span>{option.label}</span>
			{#if option.value === value}
				<span class="check" aria-hidden="true">✓</span>
			{/if}
		</div>
	{/each}
</div>

<style>
	.dropdown-trigger {
		width: 100%;
		min-height: 48px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 0 16px;
		border: 1px solid transparent;
		border-radius: 16px;
		background: rgba(0, 0, 0, 0.2);
		color: #fff;
		text-align: left;
		font-size: 14px;
		font-weight: 600;
	}
	.dropdown-trigger:hover:not(:disabled),
	.dropdown-trigger[aria-expanded='true'] {
		background: rgba(255, 255, 255, 0.1);
	}
	.dropdown-trigger.compact {
		min-height: 28px;
		height: 28px;
		width: auto;
		max-width: 100%;
		padding: 4px 10px;
		border-radius: 12px;
		font-size: 13px;
		font-weight: 500;
		background: rgba(0, 0, 0, 0.2);
		-webkit-backdrop-filter: blur(8px);
		backdrop-filter: blur(8px);
		color: #ffffff;
		gap: 6px;
		cursor: pointer;
		border: none;
		transition: background 0.15s ease;
	}
	.dropdown-trigger.compact:hover:not(:disabled),
	.dropdown-trigger.compact[aria-expanded='true'] {
		background: rgba(0, 0, 0, 0.15);
	}
	.selected-label {
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	.chevron-icon {
		width: 18px;
		height: 18px;
		flex-shrink: 0;
		align-self: center;
		display: block;
		opacity: 0.75;
		transition:
			transform 160ms ease,
			opacity 160ms ease;
	}
	[aria-expanded='true'] .chevron-icon {
		transform: rotate(180deg);
		opacity: 1;
	}
	.dropdown-list {
		display: none;
		inset: auto;
		margin: 0;
		padding: 4px;
		overflow-y: auto;
		scrollbar-width: none;
		-ms-overflow-style: none;
		border: none;
		border-radius: 14px;
		background: rgba(0, 0, 0, 0.4);
		-webkit-backdrop-filter: blur(8px);
		backdrop-filter: blur(8px);
		color: #fff;
		box-shadow: 0 12px 32px rgba(0, 0, 0, 0.6);
	}
	.dropdown-list:not(:popover-open) {
		display: none !important;
	}
	.dropdown-list:popover-open {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.dropdown-list::-webkit-scrollbar {
		display: none;
		width: 0;
		height: 0;
	}
	.dropdown-list [role='option'] {
		min-height: 28px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 6px 8px 6px 12px;
		border-radius: 10px;
		font-size: 13px;
		line-height: 1.3;
		cursor: pointer;
		color: rgba(255, 255, 255, 0.85);
		transition:
			background 0.15s ease,
			color 0.15s ease;
	}
	.dropdown-list [role='option']:hover,
	.dropdown-list [role='option']:focus-visible {
		outline: none;
		background: rgba(255, 255, 255, 0.08);
		color: #ffffff;
	}
	.dropdown-list [role='option'].selected {
		color: #ffffff;
		background: rgba(255, 255, 255, 0.12);
		font-weight: 500;
	}
	.check {
		display: flex;
		align-items: center;
		justify-content: center;
		align-self: center;
		font-size: 13px;
		font-weight: 600;
		line-height: 1;
		color: #ffffff;
		margin-right: 4px;
		flex-shrink: 0;
	}
</style>
