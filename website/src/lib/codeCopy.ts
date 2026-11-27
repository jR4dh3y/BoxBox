// Attaches a subtle copy-to-clipboard button to rendered <pre> code blocks.
// Idempotent across Astro view-transition swaps (skips blocks already wired).

export function mountCodeCopyButtons(root: ParentNode = document): void {
  const blocks = root.querySelectorAll<HTMLPreElement>('.doc-content pre, .blog-prose pre');
  for (const pre of blocks) {
    if (pre.dataset.copyReady === 'true') continue;
    const code = pre.querySelector('code');
    if (!code) continue;

    pre.dataset.copyReady = 'true';
    const wrapper = document.createElement('div');
    wrapper.className = 'code-block-wrap';
    pre.parentNode?.insertBefore(wrapper, pre);
    wrapper.appendChild(pre);

    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'code-copy-btn';
    button.setAttribute('aria-label', 'Copy code');
    button.textContent = 'Copy';

    let resetTimer: number | undefined;
    button.addEventListener('click', async () => {
      window.clearTimeout(resetTimer);
      const text = code.textContent ?? '';
      try {
        await navigator.clipboard.writeText(text);
        button.textContent = 'Copied';
        button.dataset.copied = 'true';
      } catch {
        button.textContent = 'Failed';
        delete button.dataset.copied;
      }
      resetTimer = window.setTimeout(() => {
        button.textContent = 'Copy';
        delete button.dataset.copied;
      }, 2000);
    });

    wrapper.appendChild(button);
  }
}
