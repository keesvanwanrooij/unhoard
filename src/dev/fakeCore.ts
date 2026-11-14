// A fake core for running the UI in a plain browser (development and the performance
// scenarios). It serves a generated tree of a 1 TB disk through the same commands as the
// Rust app, so the UI can be measured without a real disk. Never used in the app build.
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";

import type { Category, Explain, FileType, Place, ScanStatus, ViewNode, ViewResponse } from "../api/types";

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

/** Explanation facts for a fake item: a file type from its name, a place from its depth. */
function fakeExplain(n: FakeNode): Explain {
  const isFile = n.children.length === 0 && n.files > 0;
  const types: FileType[] = ["video", "archive", "diskImage", "photo", "database", "installer"];
  const places: Place[] = ["downloads", "documents", "appData", "programFiles", "profile", "drive"];
  const place = places[n.id % places.length] ?? "drive";
  const owned = place === "appData" || place === "programFiles";
  return {
    place,
    owner: owned ? "Example App" : null,
    ownerRoot: false,
    fileType: isFile ? (types[n.id % types.length] ?? null) : null,
    mix: !isFile && n.category !== "other" && n.category !== "hidden" ? { category: n.category, percent: 60 + (n.id % 40) } : null,
  };
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
    explain: fakeExplain(n),
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
          large: k.ruleId === null && k.steam === null,
          definition: false,
          explain: fakeExplain(n),
          protected: null,
          cloud: false,
          quarantineRoot: "C:\\Unhoard Quarantine",
          requiresAdmin: false,
          program: null,
          steam: k.steam,
          elevated: false,
          lastUse: null,
          leftover: null,
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

/** A month of daily measurements plus a few scans, with the drive filling up. */
function fakeTimeline(): unknown[] {
  const now = Math.floor(Date.now() / 1000);
  return Array.from({ length: 30 }, (_, i) => {
    const at = now - (29 - i) * 86_400;
    const used = (780 + i * 2.6 + (i > 24 ? 40 : 0)) * GB;
    return { at, kind: i % 7 === 3 ? "scan" : "daily", totals: { used, free: 951 * GB - used, hidden: 58 * GB } };
  });
}

/** Growers for the History screen and the growth colours. */
function fakeGrowth(days: number): unknown {
  const now = Math.floor(Date.now() / 1000);
  const top = nodes[0];
  const kids = (top?.children ?? []).map((c) => nodes[c]).filter((n): n is FakeNode => n !== undefined);
  const items = kids.slice(0, 6).map((n, i) => {
    const delta = Math.round((i === 0 ? 54 : i === 3 ? -31 : 8 / (i + 1)) * GB);
    return {
      path: `C:\\${n.name}`,
      name: n.name,
      before: Math.max(0, n.size - delta),
      after: n.size,
      delta,
      id: n.id,
      ruleId: null,
      explain: fakeExplain(n),
      trend: Array.from({ length: 8 }, (_, k) => [now - (7 - k) * 86_400, Math.max(0, n.size - delta + (delta * k) / 7)]),
    };
  });
  return {
    from: now - days * 86_400,
    to: now,
    items,
    deltas: kids.map((n, i) => [`C:\\${n.name}`, i === 0 ? 54 * GB : i === 3 ? -31 * GB : i % 2 === 0 ? 200 * 1024 ** 2 : 0]),
  };
}

/** A check-up with each kind of finding. */
function fakeFindings(): unknown[] {
  const f = (id: string, group: string, state: string, values: [string, string][], source: string): unknown => ({ id, group, state, values, source });
  return [
    f("memory", "performance", "ok", [["load", "48"], ["total", String(32 * GB)], ["available", String(16 * GB)], ["commit", String(20 * GB)], ["commitLimit", String(40 * GB)]], "memoryStatus"),
    f("process:0", "performance", "ok", [["name", "chrome.exe"], ["cpu", "6"], ["memory", String(2.1 * GB)], ["known", "chrome"]], "processList"),
    f("process:1", "performance", "ok", [["name", "docker desktop.exe"], ["cpu", "3"], ["memory", String(1.4 * GB)], ["known", "docker"]], "processList"),
    f("disk:NVMe SSD", "storage", "ok", [["name", "NVMe SSD"], ["media", "SSD"], ["health", "Healthy"], ["size", String(953 * GB)], ["partitions", "4"]], "storageManagement"),
    f("partition:NVMe SSD:1", "storage", "ok", [["disk", "NVMe SSD"], ["number", "1"], ["kind", "efi"], ["size", String(260 * 1024 ** 2)]], "storageManagement"),
    f("partition:NVMe SSD:2", "storage", "ok", [["disk", "NVMe SSD"], ["number", "2"], ["kind", "reserved"], ["size", String(16 * 1024 ** 2)]], "storageManagement"),
    f("partition:NVMe SSD:3", "storage", "ok", [["disk", "NVMe SSD"], ["number", "3"], ["kind", "data"], ["size", String(951 * GB)], ["letter", "C"], ["fileSystem", "NTFS"], ["label", "Windows"], ["free", String(83 * GB)]], "storageManagement"),
    f("partition:NVMe SSD:4", "storage", "ok", [["disk", "NVMe SSD"], ["number", "4"], ["kind", "recovery"], ["size", String(2 * GB)], ["fileSystem", "NTFS"], ["label", "WinRE_DRV"], ["free", String(GB)]], "storageManagement"),
    f("startup:UserRun:Chat", "startup", "action", [["name", "Chat"], ["command", "\"C:\\Apps\\chat.exe\" --tray"], ["enabled", "true"], ["missing", "false"], ["publisher", ""]], "startupApproved"),
    f("startup:UserRun:Old Tool", "startup", "look", [["name", "Old Tool"], ["command", "C:\\Old\\tool.exe"], ["enabled", "true"], ["missing", "true"], ["publisher", ""]], "startupApproved"),
    f("startup:MachineRun:Audio", "startup", "ok", [["name", "Audio"], ["command", "C:\\Windows\\audio.exe"], ["enabled", "true"], ["missing", "false"], ["publisher", ""]], "startupApproved"),
    f("freeSpace", "space", "ok", [["free", String(83 * GB)], ["total", String(951 * GB)]], "volume"),
    f("systemFiles", "health", "ok", [], "explainOnly"),
    f("pageFile", "config", "ok", [["on", "true"]], "memoryManagement"),
    f("power", "config", "ok", [["mode", "balanced"]], "powerMode"),
  ];
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
          return { language: null, mapStyle: "cells", motion: "living", measure: "onDisk", companion: "normal", adminRead: "never", firstRunDone: true, insights: true, chat: true, chatModel: null };
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
              large: false,
              definition: false,
              explain: fakeExplain(n),
              protected: null,
              cloud: false,
              quarantineRoot: null,
              requiresAdmin: false,
              program: null,
              steam: null,
              elevated: false,
              lastUse: { at: Date.now() / 1000 - 90 * 86_400, source: "changed" },
              leftover: null,
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
        case "history_timeline":
          return fakeTimeline();
        case "history_growth":
          return fakeGrowth(Number(a["days"] ?? 7));
        case "history_clear":
          return null;
        case "optimise_check":
          return fakeFindings();
        case "optimise_updates":
          return new Promise((resolve) =>
            setTimeout(() => {
              resolve([{ id: "updates", group: "health", state: "look", values: [["pending", "3"], ["failed", "0"]], source: "windowsUpdate" }]);
            }, 1500),
          );
        case "startup_preview":
          return { Ok: { name: a["name"], scope: a["scope"], old: null, enabled: a["enabled"] } };
        case "startup_set":
          return { Ok: 0 };
        case "startup_undo":
          return { Ok: null };
        case "chat_models":
          return { Ok: [{ name: "qwen3:8b", size: 5.2 * GB, recommended: true }, { name: "llama3.1:8b", size: 4.9 * GB, recommended: false }] };
        case "quarantine_preview": {
          // Every asked item moves to the quarantine on the same drive.
          const ids = (a["ids"] as number[] | undefined) ?? [];
          const moves = ids.map((id) => {
            const size = nodes[id]?.size ?? 0;
            return { node: id, path: `C:\\fake\\${String(id)}`, destRoot: "C:\\Unhoard Quarantine", logical: size, onDisk: size, cloud: false };
          });
          const total = moves.reduce((n, m) => n + m.onDisk, 0);
          return { moves, skipped: [], totalLogical: total, totalOnDisk: total, destinations: ["C:\\Unhoard Quarantine"] };
        }
        case "chat_setup":
          return {
            Ok: {
              ollama: "running",
              models: [{ name: "llama3.1:8b", size: 4.9 * GB, recommended: false }],
              recommendation: { model: "qwen3:8b", fit: "fast", download: 5.2 * GB },
              ram: 32 * GB,
              vram: 8 * GB,
            },
          };
        case "chat_pull":
          // A download that reports progress for a few seconds.
          return new Promise((resolve) => {
            for (let i = 0; i <= 10; i++)
              setTimeout(() => {
                emit("chat-pull", { status: "pulling", completed: i * 0.52 * GB, total: 5.2 * GB }).catch(() => undefined);
                if (i === 10) resolve({ Ok: true });
              }, i * 300);
          });
        case "chat_send": {
          // A canned answer that streams in, with a link to the largest item.
          const words = ["Je ", "schijf ", "is ", "vooral ", "vol ", "door ", "[Steam library](unhoard:item/1)", ". ", "Kijk ", "ook ", "bij ", "[de suggesties](unhoard:group/large)", "."];
          emit("chat-event", { kind: "tool", name: "overview" }).catch(() => undefined);
          words.forEach((w, i) => {
            setTimeout(() => {
              emit("chat-event", { kind: "delta", text: w }).catch(() => undefined);
            }, 400 + i * 80);
          });
          return new Promise((resolve) =>
            setTimeout(() => {
              emit("chat-event", { kind: "done" }).catch(() => undefined);
              resolve({ Ok: null });
            }, 500 + words.length * 80),
          );
        }
        case "chat_stop":
          return null;
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
