// Damped pointer and scroll-reveal parallax for the unified SiteFooter scene.
// The animation loop runs only while the pointer or scroll target is moving,
// then sleeps cleanly once settled.

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

/**
 * Returns 1 when the footer is just entering from below the viewport and eases
 * down to 0 once the footer rests at the bottom of the page, so the resting
 * end-of-page composition always lands cleanly at zero scroll offset.
 */
export function computeScrollReveal(rectTop: number, rectHeight: number, viewportHeight: number): number {
  if (rectHeight <= 0 || viewportHeight <= 0) return 0;
  const remaining = rectTop + rectHeight - viewportHeight;
  const travel = Math.max(rectHeight, viewportHeight * 0.55);
  return clamp(remaining / travel, 0, 1);
}

export function mountFooterParallax(footer: HTMLElement): void {
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)');

  let visible = false;
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

  function step(): void {
    frame = 0;
    if (motion.matches) {
      clearVars();
      return;
    }

    pointerX += (targetPointerX - pointerX) * 0.11;
    pointerY += (targetPointerY - pointerY) * 0.11;
    scrollReveal += (targetScrollReveal - scrollReveal) * 0.16;

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
    const rect = footer.getBoundingClientRect();
    targetScrollReveal = computeScrollReveal(rect.top, rect.height, window.innerHeight);
    if (immediate) {
      scrollReveal = targetScrollReveal;
      applyVars();
    } else {
      wake();
    }
  }

  const observer = new IntersectionObserver(
    ([entry]) => {
      visible = entry?.isIntersecting ?? false;
      footer.dataset.sceneVisible = visible ? 'true' : 'false';
      if (visible) syncScroll();
    },
    { rootMargin: '120px 0px' }
  );
  observer.observe(footer);

  window.addEventListener(
    'scroll',
    () => {
      if (visible) syncScroll();
    },
    { passive: true }
  );

  window.addEventListener('resize', () => {
    if (visible) syncScroll(true);
  });

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
    if (motion.matches) {
      if (frame !== 0) window.cancelAnimationFrame(frame);
      frame = 0;
      clearVars();
    } else {
      syncScroll(true);
    }
  });

  syncScroll(true);
}
