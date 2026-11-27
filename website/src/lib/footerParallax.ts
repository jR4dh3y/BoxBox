// Damped pointer and scroll-over curtain reveal parallax for SiteFooter.
// Activates sticky bottom-curtain uncover automatically whenever the footer fits
// inside the viewport, and keeps all layers aligned at zero offset at rest.

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

/**
 * Fallback scroll-reveal progress in [0, 1] when no `.page-curtain` precedes the footer:
 * returns 1 on initial viewport entry and eases down to 0 at end-of-page rest.
 */
export function computeScrollReveal(rectTop: number, rectHeight: number, viewportHeight: number): number {
  if (rectHeight <= 0 || viewportHeight <= 0) return 0;
  const remaining = rectTop + rectHeight - viewportHeight;
  const travel = Math.max(rectHeight, viewportHeight * 0.55);
  return clamp(remaining / travel, 0, 1);
}

/**
 * Curtain uncover progress in [0, 1] measured from the preceding `.page-curtain`'s
 * bottom edge: 1 while the page sheet fully covers the footer, easing smoothly to 0
 * as the page lifts up to expose the full footer height.
 */
export function computeCurtainReveal(
  curtainBottom: number,
  footerHeight: number,
  viewportHeight: number
): number {
  if (footerHeight <= 0 || viewportHeight <= 0) return 0;
  const uncovered = viewportHeight - curtainBottom;
  return clamp(1 - uncovered / footerHeight, 0, 1);
}

export function shouldUseStickyCurtain(
  footerHeight: number,
  curtainHeight: number,
  viewportHeight: number,
  viewportWidth: number
): boolean {
  return (
    viewportWidth >= 768 &&
    footerHeight > 0 &&
    footerHeight <= viewportHeight &&
    curtainHeight >= viewportHeight - 2
  );
}

export function mountFooterParallax(footer: HTMLElement): void {
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)');
  const prev = footer.previousElementSibling;
  const curtain = prev instanceof HTMLElement && prev.classList.contains('page-curtain') ? prev : null;

  let visible: boolean | null = null;
  let frame = 0;

  let pointerX = 0;
  let pointerY = 0;
  let targetPointerX = 0;
  let targetPointerY = 0;

  let scrollReveal = 0;
  let targetScrollReveal = 0;

  function applyVars(): void {
    footer.style.setProperty('--scene-x', pointerX.toFixed(4));
    footer.style.setProperty('--scene-y', pointerY.toFixed(4));
    footer.style.setProperty('--scene-reveal', scrollReveal.toFixed(4));
  }

  function clearVars(): void {
    footer.style.removeProperty('--scene-x');
    footer.style.removeProperty('--scene-y');
    footer.style.removeProperty('--scene-reveal');
  }

  function syncCurtainMode(): void {
    if (!curtain || motion.matches) {
      footer.dataset.curtain = 'flow';
      return;
    }
    const sticky = shouldUseStickyCurtain(
      footer.offsetHeight,
      curtain.offsetHeight,
      window.innerHeight,
      window.innerWidth
    );
    footer.dataset.curtain = sticky ? 'sticky' : 'flow';
  }

  function step(): void {
    frame = 0;
    if (motion.matches) {
      clearVars();
      return;
    }

    pointerX += (targetPointerX - pointerX) * 0.11;
    pointerY += (targetPointerY - pointerY) * 0.11;
    scrollReveal += (targetScrollReveal - scrollReveal) * 0.18;

    applyVars();

    const delta =
      Math.abs(targetPointerX - pointerX) +
      Math.abs(targetPointerY - pointerY) +
      Math.abs(targetScrollReveal - scrollReveal);

    if (delta > 0.0012) {
      frame = window.requestAnimationFrame(step);
    } else {
      pointerX = targetPointerX;
      pointerY = targetPointerY;
      scrollReveal = targetScrollReveal;
      applyVars();
    }
  }

  function wake(): void {
    if (!motion.matches && frame === 0) {
      frame = window.requestAnimationFrame(step);
    }
  }

  function syncScroll(immediate = false): void {
    if (motion.matches) return;

    const vh = window.innerHeight;
    if (curtain) {
      const curtainBottom = curtain.getBoundingClientRect().bottom;
      const netHeight = Math.max(1, footer.offsetHeight - 28);
      const nextVisible = curtainBottom < vh + 120;
      if (nextVisible !== visible) {
        visible = nextVisible;
        footer.dataset.sceneVisible = visible ? 'true' : 'false';
      }
      targetScrollReveal = computeCurtainReveal(curtainBottom, netHeight, vh);
    } else {
      const rect = footer.getBoundingClientRect();
      targetScrollReveal = computeScrollReveal(rect.top, rect.height, vh);
    }

    if (immediate) {
      scrollReveal = targetScrollReveal;
      applyVars();
    } else {
      wake();
    }
  }

  if (!curtain) {
    const observer = new IntersectionObserver(
      ([entry]) => {
        visible = entry?.isIntersecting ?? false;
        footer.dataset.sceneVisible = visible ? 'true' : 'false';
        if (visible) syncScroll();
      },
      { rootMargin: '120px 0px' }
    );
    observer.observe(footer);
  }

  window.addEventListener(
    'scroll',
    () => {
      if (curtain || visible === true) syncScroll();
    },
    { passive: true }
  );

  // Keep keyboard-focused footer links uncovered when the sticky curtain overlaps them.
  footer.addEventListener('focusin', (event) => {
    if (!curtain || footer.dataset.curtain !== 'sticky') return;
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const curtainBottom = curtain.getBoundingClientRect().bottom;
    const targetTop = target.getBoundingClientRect().top;
    if (targetTop < curtainBottom + 12) {
      window.scrollTo({
        top: document.documentElement.scrollHeight - window.innerHeight,
        behavior: motion.matches ? 'instant' : 'smooth',
      });
    }
  });

  window.addEventListener('resize', () => {
    syncCurtainMode();
    syncScroll(true);
  });

  const resizeObserver = new ResizeObserver(() => {
    syncCurtainMode();
    syncScroll(true);
  });
  resizeObserver.observe(footer);
  if (curtain) resizeObserver.observe(curtain);

  footer.addEventListener('pointermove', (event) => {
    if (event.pointerType !== 'mouse' || motion.matches) return;
    const rect = footer.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;

    const nx = clamp((event.clientX - rect.left) / rect.width, 0, 1);
    const ny = clamp((event.clientY - rect.top) / rect.height, 0, 1);

    targetPointerX = nx * 2 - 1;
    targetPointerY = ny * 2 - 1;
    wake();
  });

  footer.addEventListener('pointerleave', () => {
    targetPointerX = 0;
    targetPointerY = 0;
    wake();
  });

  motion.addEventListener('change', () => {
    syncCurtainMode();
    if (motion.matches) {
      if (frame !== 0) window.cancelAnimationFrame(frame);
      frame = 0;
      clearVars();
    } else {
      syncScroll(true);
    }
  });

  syncCurtainMode();
  syncScroll(true);
}
