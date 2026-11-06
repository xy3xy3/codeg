import { piUsesCustomAgentDir } from "@/lib/pi-config"
import type { AcpAgentInfo } from "@/lib/types"

/**
 * Per-agent overrides reach the CLI, not Codeg's global skill store.
 * For OMP, even an empty override removes an inherited variable at spawn:
 * the CLI can then use a different root/profile from the backend skill store.
 * Conservatively exclude every explicitly present selector, including blanks.
 */
export function agentUsesCustomSkillDir(agent: AcpAgentInfo): boolean {
  if (piUsesCustomAgentDir(agent)) return true
  if (agent.agent_type !== "oh_my_pi") return false
  return [
    "PI_CODING_AGENT_DIR",
    "PI_CONFIG_DIR",
    "OMP_PROFILE",
    "PI_PROFILE",
  ].some((key) => Object.prototype.hasOwnProperty.call(agent.env, key))
}
