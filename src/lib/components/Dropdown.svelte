<script lang="ts">
  import { tick } from 'svelte';

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
  let selected = $derived(options.find((option) => option.value === value));

  function positionList() {
    const rect = trigger.getBoundingClientRect();
    const width = Math.min(
      Math.max(rect.width, compact && options.length > 2 ? 280 : rect.width),
      window.innerWidth - 24,
    );
    const preferredHeight = Math.min(248, options.length * 44 + 12);
    const below = window.innerHeight - rect.bottom - 12;
    const above = rect.top - 12;
    const placeAbove = below < preferredHeight && above > below;
    const available = Math.max(80, placeAbove ? above : below);
    const height = Math.min(preferredHeight, available);
    list.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`;
    list.style.width = `${width}px`;
    list.style.top = `${placeAbove ? rect.top - height - 6 : rect.bottom + 6}px`;
    list.style.maxHeight = `${height}px`;
  }

  async function show(focusIndex?: number) {
    if (disabled || !options.length) return;
    positionList();
    if (!list.matches(':popover-open')) list.showPopover();
    if (focusIndex !== undefined) {
      await tick();
      const items = list.querySelectorAll<HTMLElement>('[role="option"]');
      items[focusIndex]?.focus();
    }
  }

  function choose(option: Option) {
    value = option.value;
    onChange?.(option.value);
    list.hidePopover();
    trigger.focus({ preventScroll: true });
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
      list.hidePopover();
      trigger.focus({ preventScroll: true });
      return;
    } else return;
    event.preventDefault();
    items[next]?.focus();
  }
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
  onclick={() => (open ? list.hidePopover() : void show())}
  onkeydown={onTriggerKeydown}
>
  <span class="selected-label">{selected?.label ?? label}</span>
  <span class="chevron" aria-hidden="true"></span>
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
    border: 1px solid #ffffff29;
    border-radius: 16px;
    background: #ffffff0b;
    color: #fff;
    text-align: left;
    font-size: 14px;
    font-weight: 600;
  }
  .dropdown-trigger:hover:not(:disabled),
  .dropdown-trigger[aria-expanded='true'] {
    background: #ffffff15;
    border-color: #ffffff55;
  }
  .dropdown-trigger:focus-visible {
    outline: none !important;
    border-color: #e0baff;
  }
  .dropdown-trigger.compact {
    min-height: 36px;
    width: auto;
    max-width: 100%;
    padding: 0 10px;
    border-radius: 12px;
    font-size: 13px;
  }
  .selected-label {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .chevron {
    width: 8px;
    height: 8px;
    flex-shrink: 0;
    margin-right: 2px;
    border-right: 2px solid currentColor;
    border-bottom: 2px solid currentColor;
    transform: translateY(-2px) rotate(45deg);
    transition: transform 160ms ease;
  }
  [aria-expanded='true'] .chevron {
    transform: translateY(2px) rotate(225deg);
  }
  .dropdown-list {
    inset: auto;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
    border: 1px solid #ffffff30;
    border-radius: 16px;
    background: #1d1d20;
    color: #fff;
    box-shadow: 0 16px 40px #0009;
  }
  .dropdown-list [role='option'] {
    min-height: 40px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 12px;
    border-radius: 11px;
    font-size: 13px;
    line-height: 1.3;
    cursor: pointer;
  }
  .dropdown-list [role='option']:hover,
  .dropdown-list [role='option']:focus-visible {
    outline: none;
    background: #ffffff18;
  }
  .dropdown-list [role='option'].selected {
    color: #f3ff00;
    background: #ffffff10;
  }
  .check {
    font-size: 16px;
    font-weight: 700;
  }
</style>
