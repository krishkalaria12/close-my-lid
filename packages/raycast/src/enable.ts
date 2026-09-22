import { showHUD } from "@raycast/api";
import { failureMessage, setClosedLidHold } from "./power";

export default async function command() {
  try {
    await setClosedLidHold(true);
  } catch (error) {
    // Dismissing the administrator prompt is a normal outcome, not a crash.
    // Without this it surfaced as Raycast's generic command failure.
    await showHUD(failureMessage(error));
    return;
  }
  await showHUD("Close My Lid is holding");
}
