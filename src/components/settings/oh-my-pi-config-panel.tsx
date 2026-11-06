"use client"

import { useTranslations } from "next-intl"

/** OMP owns authentication and provider configuration, without copying secrets. */
export function OhMyPiConfigPanel() {
  const t = useTranslations("AcpAgentSettings.ohMyPi")

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div>
        <h3 className="text-xs font-medium">{t("title")}</h3>
        <p className="mt-1 text-2xs text-muted-foreground">
          {t("description")}
        </p>
      </div>
      <p className="text-2xs text-muted-foreground">{t("setupHint")}</p>
      <code className="block overflow-x-auto rounded bg-muted px-2 py-1 text-2xs font-mono whitespace-nowrap">
        omp
      </code>
      <p className="text-2xs text-muted-foreground">{t("modelHint")}</p>
    </div>
  )
}
