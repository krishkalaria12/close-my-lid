import { showHUD } from "@raycast/api";
import { setClosedLidHold } from "./power";

export default async function command() {
  await setClosedLidHold(true);
  await showHUD("Close My Lid is holding");
}
