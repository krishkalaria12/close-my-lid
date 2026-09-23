/**
 * The shipped release. One place, because the two URLs below spell it three
 * more times: a bump that missed one of them pointed the download button at the
 * previous release while the page claimed the new one.
 *
 * Kept in step with `[workspace.package] version` in `apps/desktop/Cargo.toml`,
 * which is what the app reports and compares against `appcast.xml`.
 */
const version = "0.6.0";

const repo = "https://github.com/krishkalaria12/close-my-lid";

export const site = {
  name: "Close My Lid",
  tagline: "Keeps your Mac awake with the lid closed",
  author: "Krish Kalaria",
  url: "https://closemylid.app",
  version,
  minMacOS: "macOS 14 or later",
  arch: "Apple silicon",
  repo,
  release: `${repo}/releases/tag/v${version}`,
  download: `${repo}/releases/download/v${version}/Close-My-Lid-v${version}-macOS.zip`,
  issues: `${repo}/issues`,
  brewCask: "brew install --cask krishkalaria12/close-my-lid/close-my-lid",
  twitter: "https://x.com/KrishKalaria",
  twitterHandle: "@KrishKalaria",
  privacyUpdated: "22 September 2026",
} as const;

export type Agent = {
  name: string;
  icon: string;
};

/** Mirrors `AgentHarness` in apps/desktop/crates/lidcore/src/agents/mod.rs. */
export const agents: Agent[] = [
  { name: "Claude Code", icon: "/agents/claude-code.svg" },
  { name: "OpenAI Codex", icon: "/agents/codex.svg" },
  { name: "OpenCode", icon: "/agents/opencode.svg" },
  { name: "Antigravity", icon: "/agents/antigravity.svg" },
  { name: "GitHub Copilot", icon: "/agents/copilot.svg" },
  { name: "Cursor", icon: "/agents/cursor.svg" },
  { name: "Pi", icon: "/agents/pi.svg" },
];

export const faqs = [
  {
    q: "What does Close My Lid do?",
    a: "It is a macOS menu bar app that keeps your Mac running with the lid shut, so builds, downloads and coding agents finish while the machine is in your bag. You start a hold for 30 minutes, 1 hour, 4 hours or indefinitely, and normal sleep comes back the moment it ends.",
  },
  {
    q: "How is it different from caffeinate or Amphetamine?",
    a: "Idle-sleep assertions do not survive a closed lid. <code>caffeinate</code> holds off idle sleep, but shutting the lid triggers clamshell sleep anyway unless the Mac is on power with an external display. Close My Lid sets <code>pmset -a disablesleep</code>, the setting that actually keeps a MacBook running lid-down, and restores it when the session ends.",
  },
  {
    q: "Will it ask for my password every time?",
    a: "Once. The first hold shows a single administrator prompt that installs a scoped <code>sudoers</code> drop-in allowlisting exactly <code>pmset -a disablesleep 1</code> and <code>pmset -a disablesleep 0</code> — no wildcards. Every hold after that is passwordless, and you can remove the grant from Settings.",
  },
  {
    q: "What if the app crashes while a hold is running?",
    a: "A dead-man watchdog LaunchAgent runs every 60 seconds. While holding, the app refreshes a heartbeat file; if that goes stale past three minutes, or a timed session outlives its end by two, the watchdog releases the hold itself. Both the setting and the heartbeat survive reboots, so a Mac restarted mid-hold is released about a minute after boot.",
  },
  {
    q: "Is it safe to leave a MacBook running in a bag?",
    a: "Prefer timed sessions so the Mac returns to normal sleep on its own. As a backstop, an active hold is released at 5% battery on battery power; a charging Mac carries no battery risk, so plugged-in holds are left alone. Heat is still yours to manage.",
  },
  {
    q: "How does it know which agents are running?",
    a: "It snapshots your own processes with one <code>sysctl(KERN_PROC_UID)</code> call — no <code>ps</code>, no <code>pgrep</code>, nothing spawned. Native binaries match by executable name; npm and Bun installs match on their install-directory path. A match only counts as a session when no ancestor matches the same harness, so wrapper scripts never inflate the count.",
  },
  {
    q: "Does it need Accessibility or Screen Recording?",
    a: "No. The only privileged thing it does is the two allowlisted <code>pmset</code> calls. There is no Accessibility, no Input Monitoring, no Screen Recording, no telemetry, and no network call other than the update check.",
  },
  {
    q: "Can I drive it from a script or Raycast?",
    a: "Yes. <code>close-my-lid enable</code>, <code>disable</code> and <code>status</code> cover scripts and CI hooks, and the Raycast extension ships Start Holding Lid, Stop Holding Lid and Check Lid Hold Status. All three read the same session and reconcile against live <code>pmset</code> state every 30 seconds.",
  },
  {
    q: "How much does it cost?",
    a: "Nothing. Close My Lid is free and MIT licensed, with no account and no subscription.",
  },
];
