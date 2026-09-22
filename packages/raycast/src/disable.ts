import { showHUD } from "@raycast/api";
import { failureMessage, setClosedLidHold } from "./power";

export default async function command() {
  try {
    await setClosedLidHold(false);
  } catch (error) {
    await showHUD(failureMessage(error));
    return;
  }
  await showHUD("Close My Lid stopped");
}
