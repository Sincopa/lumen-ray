<script lang="ts">
  interface Props {
    variant?: 'active' | 'expired';
    size?: number;
    className?: string;
  }

  let { variant = 'active', size = 8, className = '' }: Props = $props();
</script>

<span
  class="status-dot {className}"
  class:expired={variant === 'expired'}
  style="width: {size}px; height: {size}px;"
  aria-hidden="true"
></span>

<style>
  .status-dot {
    display: inline-block;
    border-radius: 50%;
    background: #3aee29;
    box-shadow: 0 0 8px #3aee29a6;
    flex-shrink: 0;
  }

  .status-dot.expired {
    background: #ef3340;
    box-shadow: 0 0 8px #ef334099;
  }
</style>
