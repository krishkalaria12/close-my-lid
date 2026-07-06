import { showHUD } from "@raycast/api";
import { readClosedLidHold } from "./power";

export default async function command() {
  const enabled = await readClosedLidHold();
  await showHUD(
    enabled ? "Closed-lid hold is enabled" : "Closed-lid hold is disabled",
  );
}
