import { showHUD } from "@raycast/api";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export default async function command() {
  await execFileAsync("/usr/bin/osascript", [
    "-e",
    'do shell script "/usr/bin/pmset -a disablesleep 0" with administrator privileges',
  ]);

  await showHUD("Close My Lid stopped");
}
