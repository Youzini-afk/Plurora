import { INSTALLATION_FRAME_POLICY } from "./installation-frame";
import { POWERBOX_FRAME_POLICY } from "@/components/powerbox/powerbox-chooser";

if (INSTALLATION_FRAME_POLICY.stopRunOnUnmount) {
  throw new Error("closing or navigating away from an Installation frame must not stop its Run");
}

if (POWERBOX_FRAME_POLICY.stopOrRevokeOnUnmount) {
  throw new Error("closing a Powerbox or PWA frame must not stop a Run or revoke a Binding/Exposure");
}
