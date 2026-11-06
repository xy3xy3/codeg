import { render, screen } from "@testing-library/react"
import { NextIntlClientProvider } from "next-intl"
import { describe, expect, it } from "vitest"

import messages from "@/i18n/messages/en.json"
import { OhMyPiConfigPanel } from "./oh-my-pi-config-panel"

it("explains native sign-in without collecting or saving credentials", () => {
  render(
    <NextIntlClientProvider locale="en" messages={messages}>
      <OhMyPiConfigPanel />
    </NextIntlClientProvider>
  )
  expect(screen.getByText("omp")).toBeInTheDocument()
  expect(screen.getByText(/omp acp/)).toBeInTheDocument()
  expect(
    screen.getByText(/native ACP session configuration/)
  ).toBeInTheDocument()
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument()
  expect(screen.queryByRole("button")).not.toBeInTheDocument()
})

describe("Oh My Pi translations", () => {
  const locales = import.meta.glob<{ default: typeof messages }>(
    "../../i18n/messages/*.json",
    { eager: true }
  )

  it("provides setup instructions in every supported locale", () => {
    expect(Object.keys(locales)).toHaveLength(10)
    for (const locale of Object.values(locales)) {
      expect(Object.keys(locale.default.AcpAgentSettings.ohMyPi)).toEqual(
        Object.keys(messages.AcpAgentSettings.ohMyPi)
      )
    }
  })
})
