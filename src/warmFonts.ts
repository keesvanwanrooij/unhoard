// Prepares the body font's weights once the app is idle after start. The first use of a
// new weight of the system font makes Windows build a font instance, which took 60 to 70 ms
// at 4x throttling the first time a screen with bold text or chips opened (P11).

/** The weights and sizes the app's text uses. */
const SAMPLES: [number, number][] = [
  [400, 14],
  [600, 11],
  [600, 14],
  [700, 14],
];

/** Lays out a few invisible words in each weight, then removes them. */
export function warmFonts(): void {
  const run = (): void => {
    const box = document.createElement("div");
    box.setAttribute("aria-hidden", "true");
    box.style.cssText = "position:absolute;left:-9999px;top:0;visibility:hidden;pointer-events:none";
    for (const [weight, size] of SAMPLES) {
      const s = document.createElement("span");
      s.style.cssText = `font:${String(weight)} ${String(size)}px var(--font)`;
      s.textContent = "Unhoard 0123456789 ÀÉÏÔÜ";
      box.appendChild(s);
    }
    document.body.appendChild(box);
    // Asking for the size makes the browser lay the words out now.
    box.getBoundingClientRect();
    box.remove();
  };
  if ("requestIdleCallback" in window) window.requestIdleCallback(run, { timeout: 2000 });
  else setTimeout(run, 500);
}
