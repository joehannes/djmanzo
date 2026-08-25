/**
 * Call back when a pointer goes down anywhere outside `node`.
 *
 * A Svelte action, used as `use:clickOutside={handler}`. It exists because
 * `ThemeSwitcher` was written as `<div onclick_outside={...}>`, which is not a
 * DOM attribute, not a Svelte directive, and not anything: the theme menu
 * simply never closed unless you clicked its own button again.
 *
 * `pointerdown` rather than `click`, and captured rather than bubbled, so a
 * menu still closes when the click lands on something that stops propagation
 * or that is removed from the page before `click` would have fired.
 */
export function clickOutside(node: HTMLElement, onOutside: () => void) {
  let handle = onOutside;

  const listener = (event: PointerEvent) => {
    const target = event.target;
    if (target instanceof Node && !node.contains(target)) {
      handle();
    }
  };

  document.addEventListener("pointerdown", listener, true);

  return {
    update(next: () => void) {
      handle = next;
    },
    destroy() {
      document.removeEventListener("pointerdown", listener, true);
    },
  };
}
