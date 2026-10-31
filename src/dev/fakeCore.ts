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

/** Installs the fake core. `?entries=N` sets the tree size (default 20 000). */
export function installFakeCore(): void {
  const entries = Number(new URLSearchParams(location.search).get("entries") ?? "20000");
  build(entries);
  const status: ScanStatus = { generation: 1, root: "C:\\", driveScan: true, files: nodes.length, onDisk: nodes[0]?.size ?? 0, done: true, partial: false, elapsedMs: 4100 };
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "settings_get":
          return { language: null, mapStyle: "cells", motion: "living", measure: "onDisk", companion: "normal", firstRunDone: true };
        case "settings_set":
          return null;
        case "app_info":
          return { drives: [{ drive: "C:", totalBytes: 931 * GB, freeBytes: 83 * GB, fileSystem: "NTFS", system: true }], syncRoots: [], build: "fake" };
        case "scan_start":
          setTimeout(() => {
            emit("scan-progress", status).catch(() => undefined);
          }, 50);
          return 1;
        case "scan_status":
          return status;
        case "view":
          return view(Number(a["id"] ?? 0), Number(a["top"] ?? 120));
        case "details": {
          const n = nodes[Number(a["id"] ?? 0)] ?? nodes[0];
          if (!n) return null;
          return {
            node: viewNode(n),
            path: n.name,
            description: { ruleId: null, tier: null, actions: [{ type: "showInExplorer" }], generic: false, protected: null, cloud: false, quarantineRoot: null, requiresAdmin: false },
            cloud: null,
          };
        }
        case "suggestions":
          return [1, []];
        case "quarantine_state":
          return { batches: [], warnings: [], space: [] };
        case "cloud_roots":
          return [];
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
