import { INSTALLATION_FRAME_POLICY } from "./installation-frame";

if (INSTALLATION_FRAME_POLICY.stopRunOnUnmount) {
  throw new Error("closing or navigating away from an Installation frame must not stop its Run");
}
