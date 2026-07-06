import { showHUD } from "@raycast/api";
import { setClosedLidHold } from "./power";

export default async function command() {
  await setClosedLidHold(false);
  await showHUD("Close My Lid stopped");
}
