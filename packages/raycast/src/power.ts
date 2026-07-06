import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const pmsetPath = "/usr/bin/pmset";
const osascriptPath = "/usr/bin/osascript";

export async function setClosedLidHold(enabled: boolean): Promise<void> {
  const value = enabled ? "1" : "0";

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
    return (
      fields.length >= 2 && fields[0] === "disablesleep" && fields[1] === "1"
    );
  });
}
