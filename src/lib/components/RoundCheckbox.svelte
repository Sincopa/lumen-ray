<script lang="ts">
  import checkIcon from '../../assets/check.svg';

  interface Props {
    checked: boolean;
    ariaLabel?: string;
    disabled?: boolean;
    asSpan?: boolean;
    class?: string;
    onclick?: (e: MouseEvent) => void;
  }

  let {
    checked,
    ariaLabel,
    disabled = false,
    asSpan = false,
    class: className = '',
    onclick,
  }: Props = $props();
</script>

{#if asSpan}
  <span class="select-checkbox {className}" class:checked aria-hidden="true">
    {#if checked}
      <img src={checkIcon} alt="" />
    {/if}
  </span>
{:else}
  <button
    type="button"
    class="select-checkbox {className}"
    class:checked
    role="checkbox"
    aria-checked={checked}
    aria-label={ariaLabel}
    {disabled}
    {onclick}
  >
    {#if checked}
      <img src={checkIcon} alt="" />
    {/if}
  </button>
{/if}

<style>
  .select-checkbox {
    width: 26px;
    height: 26px;
    min-width: 26px;
    min-height: 0;
    border-radius: 50%;
    background: transparent;
    border: 2px solid #ffffff40;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    flex-shrink: 0;
    cursor: pointer;
    box-sizing: border-box;
    transition:
      background-color 160ms ease,
      border-color 160ms ease;
  }

  .select-checkbox:hover:not(:disabled) {
    border-color: #ffffff70;
  }

  .select-checkbox.checked {
    background: white;
    border-color: white;
  }

  .select-checkbox:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .select-checkbox img {
    width: 16px;
    height: 16px;
    object-fit: contain;
    filter: brightness(0);
  }
</style>
