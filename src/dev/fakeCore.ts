// A fake core for running the UI in a plain browser (development and the performance
// scenarios). It serves a generated tree of a 1 TB disk through the same commands as the
// Rust app, so the UI can be measured without a real disk. Never used in the app build.
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";

import type { Category, ScanStatus, ViewNode, ViewResponse } from "../api/types";

interface FakeNode {
  id: number;
  name: string;
  category: Category;
  size: number;
  files: number;
  parent: number | null;
  children: number[];
}

const GB = 1024 ** 3;
const nodes: FakeNode[] = [];

function add(name: string, category: Category, size: number, parent: number | null): number {
  const id = nodes.length;
  nodes.push({ id, name, category, size, files: 0, parent, children: [] });
  if (parent !== null) nodes[parent]?.children.push(id);
  return id;
}

/** Builds a tree of about `target` entries with realistic sizes. */
function build(target: number): void {
  nodes.length = 0;
  const root = add("C:\\", "other", 0, null);
  const tops: [string, Category, number][] = [
    ["Steam library", "apps", 214],
    ["Developer", "developer", 135],
    ["Photos & videos", "media", 94],
    ["Program Files", "apps", 70],
    ["Downloads", "downloads", 49],
    ["App data", "other", 40],
    ["Windows", "system", 38],
    ["OneDrive", "cloud", 38],
    ["Epic Games", "apps", 36],
    ["System files", "system", 28.8],
    ["Windows.old", "system", 24],
    ["Documents", "documents", 22],
    ["Music", "other", 14],
    ["Recycle Bin", "other", 7.2],
  ];
  let seed = 7;
  const rnd = (): number => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };
  const perTop = Math.floor(target / tops.length);
  for (const [name, cat, gb] of tops) {
    const top = add(name, cat, 0, root);
    let budget = gb * GB;
    let count = 0;
    const queue = [top];
    while (count < perTop && queue.length > 0) {
      const dir = queue.shift() ?? top;
      const n = 8 + Math.floor(rnd() * 24);
      for (let i = 0; i < n && count < perTop; i++) {
        const folder = rnd() < 0.25;
        const id = add(`${folder ? "folder" : "file"}-${String(count)}`, cat, 0, dir);
        count++;
        if (folder) queue.push(id);
        else {
          const s = Math.min(budget, Math.floor(rnd() ** 3 * gb * GB * 0.05) + 4096);
          budget -= s;
          const node = nodes[id];
          if (node) {
            node.size = s;
            node.files = 1;
          }
        }
      }
    }
  }
  for (let i = nodes.length - 1; i > 0; i--) {
    const n = nodes[i];
    const p = n && n.parent !== null ? nodes[n.parent] : undefined;
    if (n && p) {
      p.size += n.size;
      p.files += n.files;
    }
  }
}

function viewNode(n: FakeNode): ViewNode {
  return {
    id: n.id,
    name: n.name,
    kind: n.children.length > 0 || n.files === 0 ? "folder" : "file",
    logical: n.size,
    onDisk: n.size,
    files: n.files,
    category: n.category,
    tier: n.name === "Windows.old" ? "likelySafe" : null,
    ruleId: n.name === "Windows.old" ? "windows-old" : null,
    hasChildren: n.children.length > 0,
    unscanned: false,
    cloud: null,
  };
}

function view(id: number, top: number): ViewResponse {
  const n = nodes[id] ?? nodes[0];
  if (!n) throw new Error("empty fake tree");
  const kids = n.children.map((c) => nodes[c]).filter((c): c is FakeNode => c !== undefined).sort((a, b) => b.size - a.size);
  const shown = kids.slice(0, top);
  const rest = kids.slice(top);
  const crumbs = [];
  for (let c: FakeNode | undefined = n; c; c = c.parent === null ? undefined : nodes[c.parent]) crumbs.unshift({ id: c.id, name: c.name });
  return {
    generation: 1,
    view: {
      node: viewNode(n),
      path: crumbs.map((c) => c.name).join("\\"),
      breadcrumb: crumbs,
      children: shown.map(viewNode),
      others: { count: rest.length, logical: rest.reduce((s, x) => s + x.size, 0), onDisk: rest.reduce((s, x) => s + x.size, 0) },
    },
    hiddenBytes: id === 0 ? 58 * GB : null,
    scanning: false,
  };
}

/** The first fake items of at least 1 MiB, found without walking the whole tree. */
function someLargeItems(count: number): FakeNode[] {
  const out: FakeNode[] = [];
  for (const n of nodes) {
    if (n.parent !== null && n.size >= 2 ** 20) out.push(n);
    if (out.length >= count) break;
  }
  return out;
}

let suggestionCache: unknown[] | null = null;

/** Suggestions in a few groups (browser caches, build folders, Steam), to try the grouping.
 * Built once: the mock runs on the UI thread, where the real work would run in Rust. */
function fakeSuggestions(): unknown[] {
  suggestionCache ??= buildSuggestions();
  return suggestionCache;
}

function buildSuggestions(): unknown[] {
  const kinds: { ruleId: string | null; tier: string; steam: unknown; count: number }[] = [
    { ruleId: "chrome-cache", tier: "safe", steam: null, count: 9 },
    { ruleId: "node-modules", tier: "likelySafe", steam: null, count: 4 },
    { ruleId: null, tier: "review", steam: { kind: "game", appId: 1, name: "Card Game", lastPlayed: null }, count: 2 },
    { ruleId: null, tier: "likelySafe", steam: { kind: "leftover" }, count: 1 },
    { ruleId: null, tier: "review", steam: null, count: 3 },
  ];
  const pool = someLargeItems(40);
  let i = 0;
  return kinds.flatMap((k) =>
    Array.from({ length: k.count }, () => {
      const n = pool[i++ % pool.length] ?? nodes[0];
      if (!n) return null;
      const steamGame = (k.steam as { kind: string } | null)?.kind === "game";
      return {
        node: viewNode(n),
        path: `C:\\fake\\${n.name}`,
        tier: k.tier,
        description: {
          ruleId: k.ruleId,
          tier: k.tier,
          actions: steamGame ? [{ type: "appUninstall" }, { type: "showInExplorer" }] : [{ type: "quarantine" }, { type: "showInExplorer" }],
          generic: k.ruleId === null && k.steam === null,
          protected: null,
          cloud: false,
          quarantineRoot: "C:\\Unhoard Quarantine",
          requiresAdmin: false,
          program: null,
          steam: k.steam,
          elevated: false,
          lastUse: null,
        },
      };
    }),
  );
}

/** Priority-matrix points from the largest fake items, with varied ages. */
function matrixPoints(limit: number): unknown[] {
  const now = Date.now() / 1000;
  return someLargeItems(limit * 3)
    .sort((a, b) => b.size - a.size)
    .slice(0, limit)
    .map((n, i) => ({
      id: n.id,
      name: n.name,
      label: null,
      ruleId: null,
      onDisk: n.size,
      lastUse: i % 7 === 0 ? null : { at: now - ((i * 37) % 3000) * 86_400, source: i % 3 === 0 ? "played" : i % 3 === 1 ? "started" : "changed" },
      category: n.category,
      tier: "review",
    }));
}

/** Installs the fake core. `?entries=N` sets the tree size (default 20 000). */
export function installFakeCore(): void {
  const entries = Number(new URLSearchParams(location.search).get("entries") ?? "20000");
  build(entries);
  // Answers the real core computes off the UI thread are prepared here, before any timing.
  fakeSuggestions();
  const status: ScanStatus = { generation: 1, root: "C:\\", driveScan: true, files: nodes.length, onDisk: nodes[0]?.size ?? 0, done: true, partial: false, elapsedMs: 4100 };
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "settings_get":
          return { language: null, mapStyle: "cells", motion: "living", measure: "onDisk", companion: "normal", adminRead: "never", firstRunDone: true };
        case "settings_set":
          return null;
        case "app_info":
          return { drives: [{ drive: "C:", totalBytes: 931 * GB, freeBytes: 83 * GB, fileSystem: "NTFS", system: true }], syncRoots: [], build: "fake" };
        case "scan_start":
          // A scan that takes three seconds, so progress and the companion can be watched.
          for (let k = 1; k <= 10; k++) {
            setTimeout(() => {
              const part: ScanStatus = { ...status, done: k === 10, files: Math.round((status.files * k) / 10), onDisk: Math.round((status.onDisk * k) / 10), elapsedMs: k * 300 };
              emit("scan-progress", part).catch(() => undefined);
            }, k * 300);
          }
          return 1;
        case "scan_status":
          return status;
        case "view":
          return view(Number(a["id"] ?? 0), Number(a["top"] ?? 120));
        case "details": {
          const n = nodes[Number(a["id"] ?? 0)] ?? nodes[0];
          if (!n) return null;
          // Two folders stand in for app adapters, so their panels can be tried in a browser.
          const ruleId = n.name === "App data" ? "ollama-models" : n.name === "Developer" ? "docker-disk" : null;
          return {
            node: viewNode(n),
            path: n.name,
            description: {
              ruleId,
              tier: ruleId ? "review" : null,
              actions: [{ type: "showInExplorer" }],
              generic: false,
              protected: null,
              cloud: false,
              quarantineRoot: null,
              requiresAdmin: false,
              program: null,
              steam: null,
              elevated: false,
              lastUse: { at: Date.now() / 1000 - 90 * 86_400, source: "changed" },
            },
            cloud: null,
            measured: null,
          };
        }
        case "suggestions":
          return [1, fakeSuggestions()];
        case "quarantine_state":
          return { batches: [], warnings: [], space: [] };
        case "cloud_roots":
          return [];
        case "matrix":
          return [1, matrixPoints(Number(a["limit"] ?? 300))];
        case "admin_start":
          return { Ok: true };
        case "admin_read":
          return { Ok: { folders: 3, onDisk: 6 * GB } };
        case "app_uninstall":
          return null;
        case "open_app":
          return { Ok: null };
        case "docker_usage":
          return {
            Ok: [
              { kind: "Images", total: 14, active: 3, bytes: 31 * GB, reclaimable: 22 * GB },
              { kind: "Containers", total: 9, active: 2, bytes: 2 * GB, reclaimable: 1.5 * GB },
              { kind: "Local Volumes", total: 6, active: 2, bytes: 12 * GB, reclaimable: 4 * GB },
              { kind: "Build Cache", total: 210, active: 0, bytes: 8 * GB, reclaimable: 8 * GB },
            ],
          };
        case "ollama_models":
          return [
            { name: "llama3:70b", manifest: "m1", blobs: [], uniqueBlobs: [], bytes: 40 * GB, uniqueBytes: 39 * GB, modified: Date.now() / 1000 - 200 * 86_400, ids: [1, 2] },
            { name: "coder:7b", manifest: "m2", blobs: [], uniqueBlobs: [], bytes: 4.5 * GB, uniqueBytes: 3 * GB, modified: Date.now() / 1000 - 12 * 86_400, ids: null },
          ];
        case "reveal_hidden":
          return {
            Ok: Array.from({ length: 14 }, (_, i) => ({
              path: `C:\\Protected folder ${String(i + 1)}`,
              kind: (["restorePoints", "storeApps", "defender", "other"] as const)[i % 4],
              logical: (14 - i) * GB,
              onDisk: (14 - i) * GB,
              files: 1000 * (i + 1),
              children: [],
              error: null,
            })),
          };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  setTimeout(() => {
    emit("scan-progress", status).catch(() => undefined);
  }, 300);
}
