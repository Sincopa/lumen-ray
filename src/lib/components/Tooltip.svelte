<script lang="ts">
  import { fade } from 'svelte/transition';
  import type { Snippet } from 'svelte';

  interface Props {
    /** Text to show inside the tooltip */
    text?: string;
    /** Custom content instead of text */
    children?: Snippet;
    /** Whether the tooltip is visible */
    visible?: boolean;
    /** Tail position: 'bottom-center' (default) | 'bottom-left' | 'bottom-right' */
    tail?: 'bottom-center' | 'bottom-left' | 'bottom-right';
    /** Extra CSS classes */
    class?: string;
  }

  let {
    text,
    children,
    visible = true,
    tail = 'bottom-center',
    class: extraClass = '',
  }: Props = $props();
</script>

{#if visible}
  <div
    class="tooltip {extraClass}"
    class:tail-center={tail === 'bottom-center'}
    class:tail-left={tail === 'bottom-left'}
    class:tail-right={tail === 'bottom-right'}
    role="tooltip"
    transition:fade={{ duration: 150 }}
  >
    <div class="tooltip-body">
      {#if children}
        {@render children()}
      {:else}
        {text}
      {/if}
    </div>
    <!-- Callout tail: rotated square, top half hidden behind .tooltip-body -->
    <div class="tooltip-tail"></div>
  </div>
{/if}

<style>
  .tooltip {
    position: absolute;
    z-index: 200;
    pointer-events: none;
    display: inline-flex;
    flex-direction: column;
    align-items: center;
  }

  /* Main bubble */
  .tooltip-body {
    position: relative;
    z-index: 1;
    background: rgba(0, 0, 0, 0.2);
    -webkit-backdrop-filter: blur(15px);
    backdrop-filter: blur(15px);
    border-radius: 20px;
    padding: 14px 28px;
    font-family: var(--font-family-base);
    font-size: 24px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
    letter-spacing: -0.3px;
    white-space: nowrap;
    line-height: 1.25;
    /* box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.50),
      0 2px 8px rgba(0, 0, 0, 0.30); */
  }

  /* Tail: rotated square, upper half hidden behind .tooltip-body (z-index 0 < 1) */
  .tooltip-tail {
    position: relative;
    z-index: 0;
    width: 20px;
    height: 20px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 3px;
    transform: rotate(45deg);
    margin-top: -13px;
    flex-shrink: 0;
    /* box-shadow: 3px 3px 8px rgba(0, 0, 0, 0.25); */
  }

  /* Tail horizontal offsets */
  .tooltip.tail-left {
    align-items: flex-start;
  }
  .tooltip.tail-left .tooltip-tail {
    margin-left: 20px;
  }

  .tooltip.tail-right {
    align-items: flex-end;
  }
  .tooltip.tail-right .tooltip-tail {
    margin-right: 20px;
  }

  @media (prefers-reduced-motion: reduce) {
    .tooltip {
      transition: none;
    }
  }
</style>

