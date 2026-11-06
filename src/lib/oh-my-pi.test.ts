import { describe, expect, it } from "vitest"

import {
  getAgentColor,
  getAgentLabel,
  setCustomAgentDisplay,
} from "@/lib/custom-agents"
import {
  AGENT_DISPLAY_ORDER,
  ALL_AGENT_TYPES,
  compareAgentType,
  isCustomAgentType,
} from "@/lib/types"

describe("Oh My Pi built-in identity", () => {
  it("appears exactly once in the built-in selection and display lists", () => {
    expect(ALL_AGENT_TYPES.filter((type) => type === "oh_my_pi")).toEqual([
      "oh_my_pi",
    ])
    expect(AGENT_DISPLAY_ORDER.filter((type) => type === "oh_my_pi")).toEqual([
      "oh_my_pi",
    ])
    expect(isCustomAgentType("oh_my_pi")).toBe(false)
    expect(compareAgentType("oh_my_pi", "custom:test")).toBeLessThan(0)
  })

  it("has a stable name and color without a custom-agent registration", () => {
    expect(getAgentLabel("oh_my_pi")).toBe("Oh My Pi")
    expect(getAgentColor("oh_my_pi")).toBe("bg-[#7C3AED]")
  })
})

it("preserves existing custom OMP registrations as separate agents", () => {
  const aliases = ["custom:omp", "custom:oh-my-pi", "custom:oh_my_pi"] as const
  setCustomAgentDisplay(
    aliases.map((agentType) => ({ agentType, name: `Existing ${agentType}` }))
  )
  try {
    for (const agentType of aliases) {
      expect(isCustomAgentType(agentType)).toBe(true)
      expect(getAgentLabel(agentType)).toBe(`Existing ${agentType}`)
      expect(compareAgentType("oh_my_pi", agentType)).toBeLessThan(0)
    }
    expect(getAgentLabel("oh_my_pi")).toBe("Oh My Pi")
  } finally {
    setCustomAgentDisplay([])
  }
})
