import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const pmsetPath = "/usr/bin/pmset";
const sudoPath = "/usr/bin/sudo";
const osascriptPath = "/usr/bin/osascript";

export async function setClosedLidHold(enabled: boolean): Promise<void> {
  const value = enabled ? "1" : "0";

  // Prefer the passwordless allowlisted command installed by the Close My
  // Lid app (`/etc/sudoers.d/close-my-lid`); fall back to the classic admin
  // prompt when the grant is missing.
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
