import { describe, expect, it } from "vitest"
import { agentUsesCustomSkillDir } from "./agent-skill-dir"
import type { AcpAgentInfo, AgentType } from "./types"

function agent(agent_type: AgentType, env: Record<string, string> = {}) {
  return { agent_type, env } as AcpAgentInfo
}

describe("custom skill directory detection", () => {
  it.each([
    "PI_CODING_AGENT_DIR",
    "PI_CONFIG_DIR",
    "OMP_PROFILE",
    "PI_PROFILE",
  ])("excludes OMP with a per-agent %s override", (key) => {
    expect(
      agentUsesCustomSkillDir(agent("oh_my_pi", { [key]: "custom" }))
    ).toBe(true)
    expect(agentUsesCustomSkillDir(agent("oh_my_pi", { [key]: "" }))).toBe(true)
    expect(agentUsesCustomSkillDir(agent("oh_my_pi", { [key]: " " }))).toBe(
      true
    )
  })
  it("keeps default OMP and XDG-only OMP in skill surfaces", () => {
    expect(agentUsesCustomSkillDir(agent("oh_my_pi"))).toBe(false)
    expect(
      agentUsesCustomSkillDir(agent("oh_my_pi", { XDG_DATA_HOME: "/data" }))
    ).toBe(false)
  })
  it("preserves Pi and custom-agent behavior", () => {
    expect(
      agentUsesCustomSkillDir(agent("pi", { PI_CODING_AGENT_DIR: "/custom" }))
    ).toBe(true)
    expect(
      agentUsesCustomSkillDir(agent("pi", { PI_CODING_AGENT_DIR: "" }))
    ).toBe(false)
    expect(
      agentUsesCustomSkillDir(agent("custom:omp", { OMP_PROFILE: "custom" }))
    ).toBe(false)
  })
})
