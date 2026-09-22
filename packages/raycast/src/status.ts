import { showHUD } from "@raycast/api";
import { failureMessage, readClosedLidHold } from "./power";

export default async function command() {
  let enabled: boolean;
  try {
    enabled = await readClosedLidHold();
  } catch (error) {
    await showHUD(failureMessage(error));
    return;
  }
  await showHUD(
    enabled ? "Closed-lid hold is enabled" : "Closed-lid hold is disabled",
  );
}
