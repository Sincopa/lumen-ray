<script lang="ts">
  interface Props {
    checked: boolean;
    mode?: 'switch' | 'checkbox';
    disabled?: boolean;
    ariaLabel?: string;
    class?: string;
    onchange?: (checked: boolean) => void;
  }

  let {
    checked = $bindable(false),
    mode = 'switch',
    disabled = false,
    ariaLabel,
    class: className = '',
    onchange,
  }: Props = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

{#if mode === 'switch'}
  <button
    type="button"
    class="toggle-switch {className}"
    class:enabled={checked}
    role="switch"
    aria-checked={checked}
    aria-label={ariaLabel}
    {disabled}
    onclick={toggle}
  >
    <span class="toggle-thumb"></span>
  </button>
{:else}
  <label class="switch {className}">
    <input
      type="checkbox"
      {disabled}
      aria-label={ariaLabel}
      bind:checked
      onchange={(e) => onchange?.(e.currentTarget.checked)}
    />
    <span class="slider"></span>
  </label>
{/if}

<style>
  /* Button switch (used in rules) */
  .toggle-switch {
    width: 44px;
    height: 26px;
    min-width: 44px;
    min-height: 0;
    border-radius: 13px;
    background: #ffffff26;
    border: 0;
    padding: 0;
    position: relative;
    flex-shrink: 0;
    cursor: pointer;
    box-sizing: border-box;
    transition: background-color 180ms ease;
  }

  .toggle-switch.enabled {
    background: #ffffffd9;
  }

  .toggle-switch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toggle-thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: rgba(90, 90, 105, 0.95);
    box-shadow: 0 1px 3px #0003;
    transition:
      transform 220ms var(--ease-spring),
      background-color 180ms ease;
  }

  .enabled .toggle-thumb {
    transform: translateX(18px);
    background: #2a2a35;
  }

  /* Input checkbox slider (used in settings) */
  .switch {
    position: relative;
    display: inline-block;
    width: 44px;
    height: 24px;
    flex-shrink: 0;
  }

  .switch input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background-color: rgba(255, 255, 255, 0.15);
    transition: 0.3s ease;
    border-radius: 24px;
  }

  .slider:before {
    position: absolute;
    content: '';
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.3s ease;
    border-radius: 50%;
  }

  input:checked + .slider {
    background-color: #6841ee;
  }

  input:checked + .slider:before {
    transform: translateX(20px);
  }

  input:disabled + .slider {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
