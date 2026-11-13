// The map's own thread: runs the renderer on an OffscreenCanvas, so relaxing the layout and
// drawing never block the thread that handles clicks and keys (P11). The page talks to it
// through MapView; messages are method calls one way and events the other.
/// <reference lib="webworker" />
import jetbrainsMono from "@fontsource/jetbrains-mono/files/jetbrains-mono-latin-400-normal.woff2?url";
import rajdhani from "@fontsource/rajdhani/files/rajdhani-latin-700-normal.woff2?url";

import { Renderer, type DrawnState } from "./renderer";
import type { ToRenderer } from "./mapView";

declare const self: DedicatedWorkerGlobalScope;

/** Messages the worker sends to the page. */
export type FromRenderer =
  | { type: "drawn"; state: DrawnState }
  | { type: "frame"; ms: number }
  | { type: "measure"; name: string; start: string };

let renderer: Renderer | null = null;

/** The label fonts, loaded in this thread too: a worker does not share the page's fonts. */
function loadFonts(): void {
  for (const [family, url, weight] of [
    ["Rajdhani", rajdhani, "700"],
    ["JetBrains Mono", jetbrainsMono, "400"],
  ] as const) {
    const face = new FontFace(family, `url(${url})`, { weight });
    face
      .load()
      .then((f) => {
        self.fonts.add(f);
      })
      .catch(() => {
        // The labels fall back to a system font.
      });
  }
}

self.onmessage = (e: MessageEvent<ToRenderer>) => {
  const m = e.data;
  if (m.type === "init") {
    loadFonts();
    renderer = new Renderer(
      m.canvas,
      {
        drawn: (state) => {
          self.postMessage({ type: "drawn", state } satisfies FromRenderer);
        },
        frame: (ms) => {
          self.postMessage({ type: "frame", ms } satisfies FromRenderer);
        },
        measure: (name, start) => {
          self.postMessage({ type: "measure", name, start } satisfies FromRenderer);
        },
      },
      m.theme,
    );
    return;
  }
  const r = renderer;
  if (!r) return;
  switch (m.type) {
    case "destroy":
      r.destroy();
      renderer = null;
      break;
    case "theme":
      r.setTheme(m.theme);
      break;
    case "resize":
      r.resize(m.w, m.h, m.dpr);
      break;
    case "cells":
      r.setCells(m.key, m.cells);
      break;
    case "flags":
      r.setFlags(m.flags);
      break;
    case "mode":
      r.setMode(m.mode);
      break;
    case "beginZoom":
      r.beginZoom(m.id);
      break;
    case "zoom":
      r.zoom(new Map(m.from), m.last);
      break;
    case "highlight":
      r.setHighlight(m.id);
      break;
    case "hover":
      r.setHover(m.id);
      break;
    case "poke":
      r.poke(m.id, m.strength);
      break;
    case "grab":
      r.grab(m.id, m.x, m.y);
      break;
    case "dragTo":
      r.dragTo(m.x, m.y);
      break;
    case "drop":
      r.drop();
      break;
  }
};
