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

const assets = `${repo}/releases/download/v${version}`;

export const site = {
  name: "Close My Lid",
  tagline: "Keeps your laptop awake with the lid closed",
  author: "Krish Kalaria",
  url: "https://closemylid.app",
  version,
  repo,
  release: `${repo}/releases/tag/v${version}`,
  issues: `${repo}/issues`,
  brewCask: "brew install --cask krishkalaria12/close-my-lid/close-my-lid",
  twitter: "https://x.com/KrishKalaria",
  twitterHandle: "@KrishKalaria",
  privacyUpdated: "23 September 2026",
} as const;

export type PlatformId = "mac" | "windows" | "linux";

export type Platform = {
  id: PlatformId;
  name: string;
  /** What the download gets you, in a few words. */
  app: string;
  requires: string;
  download: string;
  /** The archive's file name, shown under the button. */
  file: string;
  /** The one-line install, for the copy button. */
  install: string;
  installLabel: string;
  /** Where the app is once installed. */
  open: string;
};

/**
 * One entry per release asset. The file names are fixed by
 * `.github/workflows/release.yml`; the Homebrew cask reads the macOS one.
 */
export const platforms: Platform[] = [
  {
    id: "mac",
    name: "macOS",
    app: "Menu bar app and CLI",
    requires: "macOS 14 or later · Apple silicon and Intel",
    download: `${assets}/Close-My-Lid-v${version}-macOS.zip`,
    file: `Close-My-Lid-v${version}-macOS.zip`,
    install: "brew install --cask krishkalaria12/close-my-lid/close-my-lid",
    installLabel: "Copy the Homebrew install command",
    open: "Lives in the menu bar.",
  },
  {
    id: "windows",
    name: "Windows",
    app: "Desktop app and CLI",
    requires: "Windows 10 or 11 · x64",
    download: `${assets}/Close-My-Lid-v${version}-windows-x86_64.zip`,
    file: `Close-My-Lid-v${version}-windows-x86_64.zip`,
    install:
      "irm https://raw.githubusercontent.com/krishkalaria12/close-my-lid/main/scripts/install.ps1 | iex",
    installLabel: "Copy the PowerShell install command",
    open: "Adds a Start menu shortcut. No admin rights needed.",
  },
  {
    id: "linux",
    name: "Linux",
    app: "Desktop app and CLI",
    requires: "x86_64 · systemd-logind",
    download: `${assets}/close-my-lid-v${version}-linux-x86_64.tar.gz`,
    file: `close-my-lid-v${version}-linux-x86_64.tar.gz`,
    install:
      "curl -fsSL https://raw.githubusercontent.com/krishkalaria12/close-my-lid/main/scripts/install.sh | sh",
    installLabel: "Copy the Linux install command",
    open: "Adds Close My Lid to your application menu. No root needed.",
  },
];

export const mac = platforms[0];

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
    a: "It keeps your laptop running with the lid shut, so builds, downloads and coding agents finish while the machine is in your bag. Start a hold for 30 minutes, 1 hour, 4 hours or indefinitely, and normal sleep comes back the moment it ends. On a Mac it is a menu bar app; on Windows and Linux it is a desktop app.",
  },
  {
    q: "Does it work on Windows and Linux?",
    a: "Yes. Both get a desktop app and the <code>close-my-lid</code> CLI. On Linux it takes a logind <code>handle-lid-switch</code> inhibitor — the one inhibitor logind honours when the lid closes — so it works on stock configurations without editing <code>logind.conf</code> or asking for root. On Windows it sets the power plan's lid-close action to Do nothing for the length of the hold and puts your own setting back afterwards, with no administrator prompt.",
  },
  {
    q: "How is it different from caffeinate or Amphetamine?",
    a: "Idle-sleep assertions do not survive a closed lid. <code>caffeinate</code> holds off idle sleep, but shutting the lid triggers clamshell sleep anyway unless the Mac is on power with an external display. Close My Lid sets <code>pmset -a disablesleep</code>, the setting that actually keeps a MacBook running lid-down, and restores it when the session ends.",
  },
  {
    q: "Will it ask for my password every time?",
    a: "On a Mac, once. The first hold shows a single administrator prompt that installs a scoped <code>sudoers</code> drop-in allowlisting exactly <code>pmset -a disablesleep 1</code> and <code>pmset -a disablesleep 0</code> — no wildcards. Every hold after that is passwordless, and you can remove the grant from Settings. Windows and Linux never ask.",
  },
  {
    q: "What if the app crashes while a hold is running?",
    a: "Normal sleep comes back on every platform. On Linux the kernel drops the inhibitor the moment the process dies. On Windows the next launch finds the saved lid setting and restores it. On a Mac a dead-man watchdog LaunchAgent runs every 60 seconds and releases a hold whose heartbeat has gone stale, including after a reboot mid-hold.",
  },
  {
    q: "Is it safe to leave a laptop running in a bag?",
    a: "Prefer timed sessions so the laptop returns to normal sleep on its own. As a backstop, an active hold is released at 5% battery on battery power; a charging laptop carries no battery risk, so plugged-in holds are left alone. Heat is still yours to manage.",
  },
  {
    q: "How does it know which agents are running?",
    a: "It reads the list of running processes and matches native binaries by executable name, and npm and Bun installs by their install-directory path. A match only counts as a session when no ancestor matches the same agent, so wrapper scripts never inflate the count. On a Mac that is one <code>sysctl</code> call over your own processes, with nothing spawned. The counts are shown and thrown away, never stored or sent.",
  },
  {
    q: "Does it need special permissions?",
    a: "No. There is no Accessibility, no Input Monitoring, no Screen Recording, no telemetry, and no network call other than the update check. The only privileged thing it ever does is the Mac's two allowlisted <code>pmset</code> calls.",
  },
  {
    q: "Can I drive it from a script or Raycast?",
    a: "Yes. <code>close-my-lid enable</code>, <code>disable</code> and <code>status</code> cover scripts and CI hooks on all three systems, and the Raycast extension on macOS ships Start Holding Lid, Stop Holding Lid and Check Lid Hold Status. The apps, the CLI and Raycast all share the same session.",
  },
  {
    q: "How much does it cost?",
    a: "Nothing. Close My Lid is free and MIT licensed, with no account and no subscription.",
  },
];
