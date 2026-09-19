import { shaderSettings } from '../stores/shaderSettings.svelte';

/**
 * Svelte action that renders a custom tooltip above the element on hover/focus.
 * Usage: <button use:tooltip={label}>
 *
 * Respects shaderSettings.tooltipsEnabled — when false, nothing is shown.
 * Tail is a pure CSS border-triangle so body+tail fade as one seamless unit.
 */
export function tooltip(node: HTMLElement, text: string) {
  let tip: HTMLDivElement | null = null;
  let raf = 0;

  function create() {
    if (tip || !text) return;
    // Respect the "show tooltips" setting
    if (!shaderSettings.tooltipsEnabled) return;

    tip = document.createElement('div');
    tip.className = 'custom-tooltip';
    tip.setAttribute('role', 'tooltip');
    tip.innerHTML = `
      <div class="custom-tooltip__body">${escapeHtml(text)}</div>
      <div class="custom-tooltip__tail"></div>
    `;

    // Inject into the closest positioned ancestor so absolute positioning works
    const parent = node.closest('.app-viewport') ?? document.body;
    parent.appendChild(tip);

    position();

    // Start from small + transparent, animate to full size (spring easing)
    tip.style.opacity = '0';
    tip.style.transform = 'scale(0.82)';
    // Double-RAF so the initial state is painted before the transition starts
    raf = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!tip) return;
        tip.style.transition =
          'opacity 160ms ease, transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1)';
        tip.style.opacity = '1';
        tip.style.transform = 'scale(1)';
      });
    });
  }

  function position() {
    if (!tip) return;
    const parent = tip.parentElement!;
    const pRect = parent.getBoundingClientRect();
    const nRect = node.getBoundingClientRect();

    const tipW = tip.offsetWidth || 100;
    const tipH = tip.offsetHeight || 48;

    // Centre above the node; clamp to viewport horizontal bounds
    let left = nRect.left - pRect.left + nRect.width / 2 - tipW / 2;
    const top = nRect.top - pRect.top - tipH - 6;

    left = Math.max(8, Math.min(left, pRect.width - tipW - 8));

    tip.style.left = `${left}px`;
    tip.style.top = `${top}px`;
  }

  function destroy() {
    cancelAnimationFrame(raf);
    if (tip) {
      // Animate out: fade + shrink slightly (ease-in, shorter)
      tip.style.transition =
        'opacity 120ms ease, transform 120ms cubic-bezier(0.4, 0, 1, 1)';
      tip.style.opacity = '0';
      tip.style.transform = 'scale(0.88)';
      const el = tip;
      setTimeout(() => el.remove(), 130);
      tip = null;
    }
  }

  node.addEventListener('mouseenter', create);
  node.addEventListener('focus', create);
  node.addEventListener('mouseleave', destroy);
  node.addEventListener('blur', destroy);
  node.addEventListener('click', destroy);

  return {
    update(newText: string) {
      text = newText;
      if (tip) {
        const body = tip.querySelector('.custom-tooltip__body');
        if (body) body.textContent = newText;
        position();
      }
    },
    destroy() {
      destroy();
      node.removeEventListener('mouseenter', create);
      node.removeEventListener('focus', create);
      node.removeEventListener('mouseleave', destroy);
      node.removeEventListener('blur', destroy);
      node.removeEventListener('click', destroy);
    },
  };
}

function escapeHtml(str: string) {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
