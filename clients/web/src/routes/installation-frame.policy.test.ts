import { RUN_UNAVAILABLE_REASON } from "./installation-frame";

if (RUN_UNAVAILABLE_REASON.code !== "run_unavailable_phase4") throw new Error("Phase 3 must expose a structured Run-unavailable reason");
if (RUN_UNAVAILABLE_REASON.phase !== 4) throw new Error("Run lifecycle belongs to Phase 4");
