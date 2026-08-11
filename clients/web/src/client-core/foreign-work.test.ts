import {
  FOREIGN_LAUNCH_BINDING_SCHEMA,
  foreignLaunchSecretName,
  foreignLaunchSecretRef,
  parseForeignLaunchBinding,
} from "./foreign-work";

const name = await foreignLaunchSecretName("play");
if (!/^foreign-launch-[0-9a-f]{64}$/.test(name)) {
  throw new Error("foreign launch secret name must be a stable SHA-256-derived identifier");
}
if (await foreignLaunchSecretRef("play") !== `secret_ref:installation:${name}`) {
  throw new Error("foreign launch secret reference does not match the Host convention");
}

const marker = "private-coordinate-and-credential";
const binding = parseForeignLaunchBinding(JSON.stringify({
  schema: FOREIGN_LAUNCH_BINDING_SCHEMA,
  launch_id: "play",
  target: { kind: "remote_service", endpoint: `https://service.invalid/${marker}` },
  entitlement: {
    package_id: "vendor/store-adapter",
    capability_id: "vendor/store-adapter/check",
    input: { credential: marker },
  },
}), "play");
if (binding.target.kind !== "remote_service") throw new Error("valid foreign target was not preserved");

for (const malformed of [
  "not-json",
  JSON.stringify({ schema: FOREIGN_LAUNCH_BINDING_SCHEMA, launch_id: "other", target: { kind: "remote_service", endpoint: marker } }),
  JSON.stringify({ schema: FOREIGN_LAUNCH_BINDING_SCHEMA, launch_id: "play", target: { kind: "local_executable", executable: "" } }),
]) {
  try {
    parseForeignLaunchBinding(malformed, "play");
    throw new Error("malformed binding was accepted");
  } catch (error) {
    if (error instanceof Error && error.message !== "binding_invalid") throw error;
    if (String(error).includes(marker)) throw new Error("binding validation leaked a local coordinate or credential");
  }
}
