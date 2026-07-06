import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export async function setClosedLidHold(enabled: boolean): Promise<void> {
  const value = enabled ? "1" : "0";

  await execFileAsync("/usr/bin/osascript", [
    "-e",
    `do shell script "/usr/bin/pmset -a disablesleep ${value}" with administrator privileges`,
  ]);
}

export async function readClosedLidHold(): Promise<boolean> {
  const { stdout } = await execFileAsync("/usr/bin/pmset", ["-g"]);
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
