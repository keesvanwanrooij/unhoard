// Entry point: loads settings and translations, then renders the app.
import "@fontsource/rajdhani/600.css";
import "@fontsource/rajdhani/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "./theme.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { api, onMaintenanceDone } from "./api/ipc";
import { App } from "./App";
import { initI18n } from "./i18n";
import { DEFAULT_SETTINGS, useApp } from "./store";

async function boot(): Promise<void> {
  if (import.meta.env.MODE === "fake") {
    const { installFakeCore } = await import("./dev/fakeCore");
    installFakeCore();
  }
  const settings = await api.settingsGet().catch(() => DEFAULT_SETTINGS);
  const lang = await initI18n(settings.language);
  const st = useApp.getState();
  st.queue.quiet = settings.companion === "quiet";
  useApp.setState({ settings, lang });
  onMaintenanceDone(() => {
    useApp.getState().treeChanged();
  }).catch(() => undefined);
  api
    .appInfo()
    .then((info) => {
      st.setInfo(info);
      const system = info.drives.find((d) => d.system) ?? info.drives[0];
      if (system) st.setDrive(system.drive);
    })
    .catch(() => undefined);
  const root = document.getElementById("root");
  if (root) {
    createRoot(root).render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  }
}

boot().catch(() => {
  document.body.textContent = "Unhoard could not start.";
});
