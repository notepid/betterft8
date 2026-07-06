// Reusable Svelte actions for accessibility.
//
// These are additive a11y helpers used by the modal-style overlays and by
// conditionally-rendered inputs that need reliable autofocus.

import type { ActionReturn } from 'svelte/action'

/**
 * Focus the node as soon as it mounts.
 *
 * A reliable replacement for the HTML `autofocus` attribute, which browsers
 * apply inconsistently on elements that are inserted into the DOM after the
 * initial page load (e.g. inputs rendered inside an `{#if}` block). Use as
 * `use:focusOnMount` on the element that should receive focus.
 */
export function focusOnMount(node: HTMLElement): ActionReturn {
  // Focus after the current microtask/frame so the element is fully laid out
  // and (for inputs revealed by a state change) actually focusable.
  const raf = requestAnimationFrame(() => node.focus())

  return {
    destroy() {
      cancelAnimationFrame(raf)
    },
  }
}

export interface TrapFocusParams {
  /**
   * Called when Escape is pressed inside the trapped node. When provided, the
   * action also calls `preventDefault()` and `stopPropagation()` on the event
   * so a global window-level Escape shortcut does not fire while the dialog is
   * open. Omit it for dialogs that must not be dismissable via Escape.
   */
  onEscape?: () => void
}

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'area[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'button:not([disabled])',
  'iframe',
  'object',
  'embed',
  '[tabindex]:not([tabindex="-1"])',
  '[contenteditable="true"]',
].join(',')

/**
 * Focus trap for modal dialogs.
 *
 * On mount it moves focus to the first focusable element inside `node` (unless
 * focus is already inside — e.g. an input using `use:focusOnMount`). While
 * mounted, Tab / Shift+Tab cycle within the node instead of escaping to the
 * page behind it. If `onEscape` is supplied, pressing Escape invokes it (and
 * stops the event so a global Escape shortcut cannot also fire). On destroy,
 * focus is restored to whatever was focused when the trap mounted.
 */
export function trapFocus(
  node: HTMLElement,
  params: TrapFocusParams = {},
): ActionReturn<TrapFocusParams> {
  let onEscape = params.onEscape
  const previouslyFocused = document.activeElement as HTMLElement | null

  function focusable(): HTMLElement[] {
    return Array.from(
      node.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
    ).filter((el) => el.offsetParent !== null || el === document.activeElement)
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (onEscape) {
        // Prevent a global window-level Escape handler (e.g. Halt TX) from
        // firing while this dialog is open.
        e.preventDefault()
        e.stopPropagation()
        onEscape()
      }
      return
    }

    if (e.key !== 'Tab') return

    const items = focusable()
    if (items.length === 0) {
      e.preventDefault()
      return
    }

    const first = items[0]
    const last = items[items.length - 1]
    const active = document.activeElement

    if (e.shiftKey) {
      if (active === first || !node.contains(active)) {
        e.preventDefault()
        last.focus()
      }
    } else {
      if (active === last || !node.contains(active)) {
        e.preventDefault()
        first.focus()
      }
    }
  }

  node.addEventListener('keydown', handleKeydown)

  // Move focus into the dialog on open, unless something inside already grabbed
  // it (e.g. a field using use:focusOnMount).
  requestAnimationFrame(() => {
    if (!node.contains(document.activeElement)) {
      focusable()[0]?.focus()
    }
  })

  return {
    update(newParams: TrapFocusParams = {}) {
      onEscape = newParams.onEscape
    },
    destroy() {
      node.removeEventListener('keydown', handleKeydown)
      // Restore focus to the element that had it before the dialog opened.
      previouslyFocused?.focus?.()
    },
  }
}
