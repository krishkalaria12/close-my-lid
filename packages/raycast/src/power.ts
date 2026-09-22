import { execFile } from "node:child_process";
import { access, constants } from "node:fs/promises";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const pmsetPath = "/usr/bin/pmset";
const sudoPath = "/usr/bin/sudo";
const osascriptPath = "/usr/bin/osascript";

/**
 * Where an installed copy of the app's own binary lives.
 *
 * Raycast runs commands with a minimal environment, so the Homebrew prefixes
 * are spelled out rather than left to `PATH`. Ordered by how current the copy
 * is likely to be: the cask's bundle first, then either Homebrew prefix.
 */
const cliCandidates = [
  "/Applications/Close My Lid.app/Contents/MacOS/CloseMyLid",
  "/opt/homebrew/bin/close-my-lid",
  "/usr/local/bin/close-my-lid",
];

/**
 * The installed CLI, if there is one.
 *
 * Driving the app rather than `pmset` directly is what keeps a hold started
 * from Raycast a *session*: the CLI records it in the state file and writes the
 * watchdog heartbeat, so a hold survives into the menu bar app's readout and
 * the dead-man switch can still release it. Writing `pmset` behind the app's
 * back leaves a hold nothing is supervising.
 *
 * Resolved on each call rather than cached: a Raycast command is a short-lived
 * process, and the answer can change between one invocation and the next.
 */
async function findCli(): Promise<string | undefined> {
  for (const candidate of cliCandidates) {
    try {
      await access(candidate, constants.X_OK);
      return candidate;
    } catch {
      // Not installed at this path; try the next.
    }
  }
  return undefined;
}

export async function setClosedLidHold(enabled: boolean): Promise<void> {
  const cli = await findCli();
  if (cli) {
    await execFileAsync(cli, [enabled ? "enable" : "disable"]);
    return;
  }

  const value = enabled ? "1" : "0";

  // No app installed: fall back to the setting itself. Prefer the passwordless
  // allowlisted command the app installs (`/etc/sudoers.d/close-my-lid`), then
  // the classic admin prompt when the grant is missing.
  try {
    await execFileAsync(sudoPath, [
      "-n",
      pmsetPath,
      "-a",
      "disablesleep",
      value,
    ]);
    return;
  } catch {
    // fall through to the elevated path
  }

  await execFileAsync(osascriptPath, [
    "-e",
    `do shell script "${pmsetPath} -a disablesleep ${value}" with administrator privileges`,
  ]);
}

export async function readClosedLidHold(): Promise<boolean> {
  const { stdout } = await execFileAsync(pmsetPath, ["-g"]);
  return parseClosedLidHold(stdout);
}

export function parseClosedLidHold(output: string): boolean {
  return output.split("\n").some((line) => {
    const fields = line.trim().split(/\s+/);
    const key = fields[0]?.toLowerCase();
    return (
      fields.length >= 2 &&
      (key === "sleepdisabled" || key === "disablesleep") &&
      fields[1] === "1"
    );
  });
}

/**
 * The one line to show the user when a command fails.
 *
 * Every failure here is a refusal rather than a crash — the administrator
 * prompt was dismissed, or the CLI reported that another process owns the hold
 * — and an unhandled rejection shows Raycast's generic "command failed" instead
 * of any of that.
 */
export function failureMessage(error: unknown): string {
  const detail = describe(error);
  if (/user canceled|user cancelled|-128/i.test(detail)) {
    return "Administrator approval cancelled";
  }
  return detail || "Could not change closed-lid sleep";
}

function describe(error: unknown): string {
  if (typeof error === "object" && error !== null) {
    // `execFile` puts a failing command's own wording on stderr, which is the
    // part that actually says what went wrong.
    const stderr = (error as { stderr?: unknown }).stderr;
    if (typeof stderr === "string" && stderr.trim()) {
      return stderr.trim().split("\n")[0];
    }
  }
  return error instanceof Error ? error.message : String(error);
}
