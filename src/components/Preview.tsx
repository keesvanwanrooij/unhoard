// The preview of a move: what will move, how much, where to, and what is skipped and why.
// Nothing changes until the user confirms.
import type { JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";

import type { Action, MovePlan, SkipReason } from "../api/types";
import { fmtBytes } from "../format";
import { useApp } from "../store";

/** The button text of an action. */
export function actionLabel(a: Action, t: TFunction): string {
  if (a.type === "windowsTool") return t(`actions.windowsTool.${a.tool}`);
  return t(`actions.${a.type}`);
}

function reasonText(r: SkipReason, t: TFunction): string {
  return t(`preview.reasons.${r.kind}`);
}

/** The preview sheet of a quarantine move. */
export function MovePreview({ plan, busy, onConfirm, onCancel }: { plan: MovePlan; busy: boolean; onConfirm: () => void; onCancel: () => void }): JSX.Element {
  const { t } = useTranslation();
  const { lang } = useApp(useShallow((s) => ({ lang: s.lang })));
  const sameDrive = plan.moves.some((m) => !m.cloud);
  return (
    <div className="sheet" role="group" aria-label={t("preview.title")}>
      <b>{t("preview.title")}</b>
      <div>{t("preview.items", { count: plan.moves.length, size: fmtBytes(plan.totalOnDisk, lang) })}</div>
      {plan.destinations.length > 0 ? (
        <div>
          {t("preview.destination")} <code>{plan.destinations.join(", ")}</code>
        </div>
      ) : null}
      {plan.skipped.length > 0 ? (
        <>
          <div>{t("preview.skipped")}</div>
          <ul>
            {plan.skipped.map((s) => (
              <li key={s.path}>
                <code>{s.path}</code> · {reasonText(s.reason, t)}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {sameDrive ? <div className="note">{t("preview.notFreed")}</div> : null}
      <div className="btns">
        <button className="btn primary" disabled={busy || plan.moves.length === 0} onClick={onConfirm}>
          {t("preview.confirm")}
        </button>
        <button className="btn" disabled={busy} onClick={onCancel}>
          {t("preview.cancel")}
        </button>
      </div>
    </div>
  );
}
