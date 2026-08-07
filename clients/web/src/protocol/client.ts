export interface ProtocolResponse<T = unknown> {
  id: string;
  result?: T;
  error?: { code: string; message: string; details?: unknown };
  diagnostics?: ContractDiagnostic[];
}

export interface ContractDiagnostic {
  code: string;
  severity: string;
  requested_id: string;
  canonical_id: string;
  maturity: "deprecated" | "legacy_adapter" | string;
  message: string;
  deprecated_in?: string;
  replacement?: string;
  support_until?: string;
}

export type ContractOwnerLayer =
  | "substrate"
  | "host"
  | "protocol"
  | "shell"
  | "cross_layer"
  | "legacy_adapter";

export interface ContractVersionRequirement {
  layer: ContractOwnerLayer;
  version: string;
}

export interface ProtocolSelection {
  protocol_id: string;
  version: string;
  profile?: string;
}

export interface ContractSelection {
  profile: string;
  versions?: ContractVersionRequirement[];
  protocols?: ProtocolSelection[];
}

export interface HostContractInfo {
  protocol_version: string;
  supported_transports: string[];
  contract_registry_version?: string;
  default_profile?: string;
  layers?: unknown[];
  versions?: unknown[];
  profiles?: unknown[];
  aliases?: Array<{
    id: string;
    canonical_id: string;
    maturity?: string;
    deprecated_in?: string;
    replacement?: string;
    support_until?: string;
  }>;
  contract_methods?: unknown[];
  protocol_commons_registry_version?: string;
  protocols?: unknown[];
  maturity?: string;
  methods: unknown[];
}

import { resolveBrowserAccessToken } from "@/client-core/credentials";

export function resolveManagedProfile(): string {
  if (typeof window === "undefined") return "default";
  const injected = window.__PLURORA_RUNTIME__?.profile?.trim();
  if (injected) return injected;
  const platform = window.__PLURORA_RUNTIME__?.platform
    ?? new URLSearchParams(window.location.search).get("plurora_platform");
  return platform === "desktop" ? "desktop" : "default";
}

export {
  BROWSER_ACCESS_TOKEN_STORAGE_KEY,
  clearBrowserAccessToken,
  readBrowserAccessToken,
  resolveBrowserAccessToken,
  storeBrowserAccessToken,
} from "@/client-core/credentials";

export class ProtocolHttpError extends Error {
  constructor(
    readonly status: number,
    readonly body: string,
  ) {
    super(`${status}: ${body || "HTTP error"}`);
    this.name = "ProtocolHttpError";
  }

  get isAuthError(): boolean {
    return this.status === 401;
  }
}

export interface CapabilityInvocationResult<TOutput = unknown> {
  capability_id: string;
  correlation_id: string;
  duration_ms: number;
  output: TOutput;
  provider_package_id: string;
}

export interface InstallSource {
  root_url: string;
  root_ref?: string;
  lockfile?: string;
  require_signed?: boolean;
  strict_conformance?: boolean;
}

export interface InstallPlan {
  root_id: string;
  packages: InstallPlannedPackage[];
  project_descriptor?: unknown;
  permissions_summary: InstallPermissionsSummary;
  signature_summary: InstallSignatureSummary;
  integrity_summary: InstallIntegritySummary;
}

export interface InstallPlannedPackage {
  id: string;
  version: string;
  source: string;
  url?: string;
  ref?: string;
  path?: string;
  commit_sha?: string;
  manifest_hash: string;
  tree_hash: string;
  signed: boolean;
  signed_by?: string;
  permissions: {
    capabilities_invoke?: string[];
    network_hosts?: string[];
    secret_refs?: string[];
  };
  requires?: Array<{ id: string; source: unknown; version?: string }>;
  conformance?: InstallConformanceReport;
}

export interface InstallConformanceReport {
  passed?: boolean;
  checks?: Array<{ id?: string; status?: string; passed?: boolean; message?: string }>;
  failures?: unknown[];
  warnings?: unknown[];
  [key: string]: unknown;
}

export interface InstallPermissionsSummary {
  new_capabilities: string[];
  new_network_hosts: string[];
  new_secret_refs: string[];
}

export interface InstallSignatureSummary {
  all_signed: boolean;
  unsigned_packages: string[];
}

export interface InstallIntegritySummary {
  manifest_hashes_match_lockfile: boolean;
  drift_detected: unknown[];
}

export type InstallDetectedKind =
  | { kind: "native"; descriptor?: unknown }
  | { kind: "declared_external"; descriptor?: unknown }
  | { kind: "external"; has_manifest_yaml?: boolean };

export interface InstallConsent {
  approved_capabilities: string[];
  approved_network_hosts: string[];
  approved_secret_refs: string[];
}

export interface InstallExecuteResult {
  installed: Array<{ id: string }>;
  lockfile: string;
  project?: { project_id?: string } | null;
}

export interface InstallUninstallResult {
  removed_from_profile: boolean;
  store_path_orphaned?: string | null;
  store_paths_orphaned?: string[];
  project?: { project_id: string; data_action: string } | null;
}

export interface UpdateCheckRecord {
  id?: string;
  package_id?: string;
  project_id?: string | null;
  source_kind?: string;
  applicable?: boolean;
  status?: string;
  reason?: string;
  available?: boolean;
  dangling?: boolean;
  current_commit?: string | null;
  upstream_commit?: string | null;
  current_tree_hash?: string | null;
  available_tree_hash?: string | null;
  installed_at_store?: string | null;
}

export interface UpdateCheckResult {
  results: UpdateCheckRecord[];
}

export interface ProjectUpdateResult {
  status?: string;
  updated?: boolean;
  updated_packages?: string[];
  reason?: string;
  check?: UpdateCheckResult;
  execute?: unknown;
  store_gc?: unknown;
}

const INSTALL_LAB_PROVIDER = "official/install-lab";
const DOCKER_RUNTIME_LAB_PROVIDER = "official/docker-runtime-lab";
const INSTALL_LAB_CAPABILITIES = {
  resolvePlan: `${INSTALL_LAB_PROVIDER}/resolve_plan`,
  detectKind: `${INSTALL_LAB_PROVIDER}/detect_kind`,
  executePlan: `${INSTALL_LAB_PROVIDER}/execute_plan`,
  uninstall: `${INSTALL_LAB_PROVIDER}/uninstall`,
  checkForUpdates: `${INSTALL_LAB_PROVIDER}/check_for_updates`,
  updateProject: `${INSTALL_LAB_PROVIDER}/update_project`,
} as const;
const DOCKER_RUNTIME_LAB_CAPABILITIES = {
  startContainer: `${DOCKER_RUNTIME_LAB_PROVIDER}/start_container`,
  stopContainer: `${DOCKER_RUNTIME_LAB_PROVIDER}/stop_container`,
} as const;

function normalizeInstallRootUrl(input: string): string {
  const trimmed = input.trim();
  if (!trimmed) return trimmed;
  if (trimmed.startsWith("~")) {
    throw new Error("Home-relative paths are not accepted from the web UI. Use an absolute path or HTTPS Git URL.");
  }
  if (/^[\w.-]+\/[\w./-]+(?:\.git)?(?:#.+)?$/.test(trimmed) && !trimmed.includes("://")) {
    return `https://${trimmed}`;
  }
  return trimmed;
}

export interface PackageRecord {
  id: string;
  version: string;
  state: string;
  entry_kind: string;
  capability_count: number;
  hook_count: number;
  last_failure?: PackageFailureSummary;
}

export interface PackageFailureSummary {
  package_id: string;
  reason: string;
  exit_code?: string | null;
  signal?: string | null;
  failed_at: string;
  stderr_tail_redacted: string[];
  log_tail_redacted: SubprocessLogLine[];
  stderr_truncated: boolean;
  redaction_state: "redacted" | "safe" | "not_captured" | "policy_ref" | "unsafe_blocked";
  state: string;
}

export interface SubprocessLogLine {
  package_id: string;
  stream: string;
  line: string;
}

export interface RegisteredCapability {
  capability_id: string;
  provider_package_id: string;
  version: string;
  streaming: boolean;
}

export interface KernelEvent {
  id: string;
  session_id: string;
  sequence: number;
  writer_package_id: string;
  kind: string;
  payload: unknown;
  metadata: unknown;
  created_at: string;
}

export interface SurfaceActivation {
  launch_capability_id?: string;
  session_template?: Record<string, unknown>;
  input_schema?: unknown;
}

export interface SurfacePermissionRequirement {
  permission: string;
  scope?: string;
  reason?: string;
  risk: "low" | "medium" | "high";
}

export interface SurfaceContribution {
  id: string;
  version: string;
  slot: string;
  title: string;
  description?: string;
  capability_id?: string;
  allowed_capability_ids?: string[];
  activation: SurfaceActivation;
  required_permissions: SurfacePermissionRequirement[];
  approval_policy?: "none" | "user_approval" | "fork_then_approve";
  metadata: Record<string, unknown>;
}

export interface SurfaceContributionRecord {
  package_id: string;
  entry_kind: string;
  package_state: string;
  surface: SurfaceContribution;
}

export interface AssetRecord {
  id: string;
  origin_package_id: string;
  mime: string;
  hash: string;
  size_bytes: number;
  created_at: string;
  metadata?: unknown;
  descriptor?: ArtifactDescriptor | null;
}

export interface ArtifactDescriptor {
  artifact_type_uri: string;
  media_type: string;
  digest: string;
  size_bytes: number;
  references?: string[];
  annotations?: Record<string, unknown>;
}

export interface ProjectionRecord {
  id: string;
  session_id: string;
  source_kind_prefix?: string;
  state: unknown;
}

export interface ProposalRecord {
  id: string;
  status: string;
  target_session_id?: string;
  target_branch_id?: string;
  operations: unknown[];
  required_permissions: string[];
  expected_effects: unknown;
  result?: unknown;
}

export interface ProjectStorageSummary {
  data_bytes: number | null;
  cache_bytes: number | null;
  bundle_bytes: number | null;
  log_bytes: number | null;
  total_bytes: number | null;
  measured_at: string | null;
  measurement_state: "measured" | "unknown" | string;
}

export interface ProjectRecord {
  id: string;
  title: string;
  description?: string;
  type: "plurora_native" | "external_wrapped" | "external_workspace";
  state: "installed" | "stopped" | "starting" | "running" | "stopping" | "failed" | "archived";
  icon?: string;
  entry_surface_id?: string;
  running_session_id?: string;
  storage_summary?: ProjectStorageSummary;
  packages?: string[];
  metadata?: Record<string, unknown>;
}

export interface PortLeaseRequest {
  target_id: string;
  port_name: string;
  protocol?: "tcp" | "udp" | string;
  requested_port?: number | null;
}

export interface ProxyRegisterRequest {
  route_id?: string | null;
  protocol?: "http" | "websocket" | string;
  access?: "host_authenticated" | "public";
  upstream: {
    port_lease_id: string;
    port_name: string;
  };
}

export interface DockerStartContainerInput {
  image: string;
  container_port: number;
  host_port: number;
  route_id: string;
  port_lease_id: string;
  approved: true;
  pull_if_missing?: boolean;
  container_name?: string;
  name?: string;
}

export interface DockerStartContainerOutput {
  kind?: string;
  container_id?: string;
  container_name?: string;
  status?: string;
  image?: string;
  container_port?: number;
  host_port?: number;
  route_id?: string;
  port_lease_id?: string;
  docker_performed?: boolean;
  container_started?: boolean;
  reason?: string;
  diagnostics?: unknown;
  warnings?: unknown;
}

export interface DockerStopContainerInput {
  approved: true;
  container_id?: string;
  container_name?: string;
  container?: string;
  route_id: string;
  port_lease_id: string;
  timeout_secs?: number;
  force?: boolean;
}

export interface DockerStopContainerOutput {
  kind?: string;
  container_id?: string;
  container_name?: string;
  status?: string;
  docker_performed?: boolean;
  reason?: string;
}

export interface HostDeployProjectInput {
  project_id: string;
  image: string;
  container_port: number;
  port_name: string;
  route_id: string;
  route_access: "host_authenticated" | "public";
  health_path?: string;
  pull_if_missing: boolean;
}

export interface HostDeployProjectOutput {
  route_id: string;
  route_access: "host_authenticated" | "public";
  public_url: string;
  port_lease_id: string;
  container_id: string;
  container_name?: string | null;
}

export interface HostStopProjectDeploymentOutput {
  route_id: string;
  stopped: boolean;
  warnings: string[];
}

export type BuildDeployStrategy = "dockerfile" | "nixpacks";
export type BuildDeployJobState = "queued" | "cloning" | "building" | "starting" | "registering_proxy" | "probing" | "ready" | "failed" | "cancelled" | string;

export interface RuntimeEnvSpec {
  name: string;
  value?: string;
  secret_ref?: string;
}

export interface RuntimeMountSpec {
  source_host_path: string;
  container_path: string;
  mode?: "ro" | "rw";
  approved: boolean;
  high_risk_approved?: boolean;
  reason: string;
}

export interface HostBuildDeployRequest {
  project_id: string;
  source_url: string;
  ref_name: string;
  strategy?: BuildDeployStrategy;
  dockerfile?: string;
  container_port: number;
  port_name: string;
  route_id: string;
  route_access: "host_authenticated" | "public";
  health_path?: string;
  approved: true;
  source_commit?: string;
  build_id?: string;
  runtime_env?: RuntimeEnvSpec[];
  runtime_mounts?: RuntimeMountSpec[];
  idempotency_key?: string;
}

export interface RuntimeEnvSummary {
  name: string;
  source: "plain" | "secret_ref" | string;
}

export interface RuntimeMountSummary {
  container_path: string;
  mode: "ro" | "rw" | string;
  source_basename?: string | null;
  source_kind?: string;
  source_hash?: string;
  approved?: boolean;
}

export interface HostBuildDeployResult {
  route_id: string;
  route_access: "host_authenticated" | "public";
  public_url: string;
  port_lease_id: string;
  container_id: string;
  container_name?: string | null;
  image: string;
  build_id: string;
  source_commit: string;
  build_descriptor_hash: string;
  strategy: string;
  runtime_env?: RuntimeEnvSummary[];
  runtime_mounts?: RuntimeMountSummary[];
  warnings?: string[];
}

export interface BuildDeployJobSubmitResponse {
  job_id: string;
  status_url: string;
  events_url: string;
  state: BuildDeployJobState;
}

export interface BuildDeployJobStatusResponse {
  job_id: string;
  project_id: string;
  route_id: string;
  build_id?: string | null;
  state: BuildDeployJobState;
  created_at_ms: number;
  updated_at_ms: number;
  result?: HostBuildDeployResult | null;
  error?: string | null;
  events_url: string;
  idempotency_key?: string | null;
  operation: DeploymentOperation;
}

export interface BuildDeployJobEvent {
  job_id: string;
  state: BuildDeployJobState;
  message: string;
  sequence: number;
  timestamp_ms: number;
}

export interface BuildDeployCancelResponse {
  job_id: string;
  state: BuildDeployJobState;
  cancelled: boolean;
}

export type DeploymentOperation = "build_deploy" | "verified_activate" | "recover" | "rollback";
export type DeploymentSourceKind = "git_clone" | "verified_artifact";

export interface PersistedRuntimeEnvSpec {
  name: string;
  secret_ref: string;
}

export interface DeploymentRevision {
  revision_id: string;
  project_id: string;
  job_id?: string | null;
  operation: DeploymentOperation;
  parent_revision_id?: string | null;
  created_at_ms: number;
  target_id: string;
  source_kind: DeploymentSourceKind;
  source_url: string;
  ref_name: string;
  dockerfile?: string | null;
  container_port: number;
  port_name: string;
  route_id: string;
  route_access: "host_authenticated" | "public";
  health_path?: string | null;
  image: string;
  build_id: string;
  source_commit: string;
  build_descriptor_hash: string;
  strategy: string;
  runtime_env: PersistedRuntimeEnvSpec[];
  verified_change_set_id?: string | null;
  verification_ref?: ArtifactDescriptor | null;
  build_context_ref?: ArtifactDescriptor | null;
  preview_ref?: ArtifactDescriptor | null;
  approval_ref?: ArtifactDescriptor | null;
  verified_build_network_mode?: DevelopmentNetworkMode | null;
  target_deployment?: { deployment_id: string; route_id: string; port_lease_id: string } | null;
  recoverable: boolean;
  recovery_blockers: string[];
  receipt: HostBuildDeployResult;
}

export interface ProjectDeploymentsResponse {
  project_id: string;
  active_revision_id?: string | null;
  active_revision?: DeploymentRevision | null;
  recovery_required: boolean;
  runtime_ready: boolean;
  jobs: BuildDeployJobStatusResponse[];
  revisions: DeploymentRevision[];
}

export interface DeploymentActionResponse {
  operation: DeploymentOperation;
  previous_revision_id?: string | null;
  revision: DeploymentRevision;
  warnings: string[];
}

export type DevelopmentNetworkMode = "none" | "bridge";
export type DevelopmentWorkspaceOwnership = "managed_external" | "linked_local" | "native_managed";
export type DevelopmentChangeStatus = "drafted" | "approved" | "rejected" | "staging" | "verifying" | "promoting" | "verified" | "committed" | "recovery_required" | "failed";

export type DevelopmentVerificationPlan =
  | { kind: "static_validation" }
  | { kind: "docker_build"; dockerfile?: string; network_mode?: DevelopmentNetworkMode; timeout_secs?: number };

export type DevelopmentFileOperationRequest =
  | { op: "file_write"; path: string; content: string; executable?: boolean }
  | { op: "file_delete"; path: string };

export interface DevelopmentDraftRequest {
  goal: string;
  operations: DevelopmentFileOperationRequest[];
  verification?: DevelopmentVerificationPlan;
  expected_tree_digest?: string;
  idempotency_key?: string;
}

export interface DevelopmentVerificationResult {
  kind: string;
  succeeded: boolean;
  network_mode: DevelopmentNetworkMode;
  image?: string | null;
  log_tail?: string | null;
  artifact_ref: ArtifactDescriptor;
  deployment_artifact_ref?: ArtifactDescriptor;
}

export type DevelopmentDeploymentStatus =
  | "preparing"
  | "building"
  | "previewing"
  | "preview_ready"
  | "approved"
  | "rejected"
  | "activating"
  | "active"
  | "recovery_required"
  | "failed";

export interface DevelopmentDeploymentPreviewRequest {
  target_id: string;
  container_port: number;
  port_name: string;
  route_id: string;
  route_access?: "host_authenticated" | "public";
  health_path?: string;
  idempotency_key?: string;
}

export interface DevelopmentDeploymentApprovalRequest {
  approved: boolean;
  reason?: string;
}

export interface DevelopmentDeploymentPreview {
  route_id: string;
  public_url: string;
  port_lease_id: string;
  deployment: { deployment_id: string; route_id: string; port_lease_id: string };
  image: string;
  image_id: string;
  container_id: string;
  container_name?: string | null;
  build_operation_id: string;
  deployment_operation_id: string;
  ready_at_ms: number;
}

export interface DevelopmentDeploymentRecord {
  schema_version: number;
  deployment_id: string;
  status: DevelopmentDeploymentStatus;
  target_id: string;
  source_tree_digest: string;
  verification_ref: ArtifactDescriptor;
  build_context_ref: ArtifactDescriptor;
  authority_ref: ArtifactDescriptor;
  dockerfile: string;
  network_mode: DevelopmentNetworkMode;
  container_port: number;
  port_name: string;
  route_id: string;
  route_access: "host_authenticated" | "public";
  health_path?: string | null;
  preview_route_id: string;
  preview_port_lease_id?: string | null;
  target_deployment_id?: string | null;
  build_id: string;
  build_descriptor_hash: string;
  build_operation_id?: string | null;
  deployment_operation_id?: string | null;
  preview?: DevelopmentDeploymentPreview | null;
  preview_ref?: ArtifactDescriptor | null;
  approval_decision?: DevelopmentApprovalDecision | null;
  approval_ref?: ArtifactDescriptor | null;
  activation_revision_id?: string | null;
  previous_revision_id?: string | null;
  error?: string | null;
  created_at_ms: number;
  updated_at_ms: number;
  idempotency_key?: string | null;
  request_digest: string;
}

export interface DevelopmentApprovalDecision {
  id: string;
  decision_type_uri: string;
  change_set_id: string;
  outcome: "allowed" | "denied" | "requires_approval";
  principal: { kind: string } & Record<string, unknown>;
  reason?: string | null;
  evaluated_authority: string[];
  decided_at: string;
  policy_ref?: ArtifactDescriptor | null;
}

export interface DevelopmentChangeRecord {
  schema_version: number;
  revision: number;
  project_id: string;
  workspace_ownership: DevelopmentWorkspaceOwnership;
  intent: { id: string; goal: unknown; created_at: string } & Record<string, unknown>;
  intent_ref: ArtifactDescriptor;
  change_set: {
    id: string;
    operations: Array<{ op: string; target?: string | null; input_refs?: ArtifactDescriptor[]; payload?: unknown }>;
    required_authority: string[];
    expected_effects: unknown;
    created_at: string;
  } & Record<string, unknown>;
  change_set_ref: ArtifactDescriptor;
  policy_decision: Record<string, unknown>;
  policy_decision_ref: ArtifactDescriptor;
  approval_decision?: DevelopmentApprovalDecision | null;
  approval_ref?: ArtifactDescriptor | null;
  status: DevelopmentChangeStatus;
  base_tree_digest: string;
  proposed_tree_digest?: string | null;
  verification_plan: DevelopmentVerificationPlan;
  verification_result?: DevelopmentVerificationResult | null;
  managed_promotion?: {
    previous_tree_digest: string;
    proposed_tree_digest: string;
    destination_preexisting: boolean;
  } | null;
  recovery_kind?: "docker_verification" | "managed_promotion" | null;
  commit?: Record<string, unknown> | null;
  deployment?: DevelopmentDeploymentRecord | null;
  error?: string | null;
  created_at_ms: number;
  updated_at_ms: number;
  idempotency_key?: string | null;
}

export interface DevelopmentChangeListResponse {
  changes: DevelopmentChangeRecord[];
}

export interface DevelopmentExecuteResponse {
  accepted: boolean;
  change: DevelopmentChangeRecord;
}

export interface DevelopmentPatchBundle {
  schema_version: number;
  project_id: string;
  change_set_id: string;
  base_tree_digest: string;
  operations: Array<
    | { op: "file_write"; path: string; content: string; executable: boolean; content_digest: string }
    | { op: "file_delete"; path: string }
  >;
}

export interface ExecutionTarget {
  id: string;
  name: string;
  reachability: "local_host" | string;
  status: "available" | "unavailable" | string;
  capabilities?: Array<"local_exec" | "port_lease" | "http_proxy_upstream" | "websocket_proxy_upstream" | string>;
}

export type TargetOperationKind =
  | "artifact_materialize"
  | "artifact_release"
  | "deployment_apply"
  | "deployment_observe"
  | "deployment_drain"
  | "deployment_stop"
  | "health_probe"
  | "verifier_run";

export type TargetOperationStatus =
  | "requested"
  | "accepted"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "outcome_unknown"
  | "expired";

export interface TargetOperationAuthority {
  target_id: string;
  operation_id: string;
  step_id: string;
  project_id: string;
  effect: TargetOperationKind;
  artifact_digests: string[];
  lease_epoch: number;
  policy_epoch: number;
  issued_at_ms: number;
  expires_at_ms: number;
  nonce: string;
  request_digest: string;
  authority_digest: string;
}

export interface TargetOperationReceipt {
  operation_id: string;
  target_id: string;
  execution_id: string;
  step_id: string;
  request_digest: string;
  authority_digest: string;
  status: Exclude<TargetOperationStatus, "requested" | "accepted" | "running" | "expired">;
  completed_at_ms: number;
  output: unknown;
  diagnostics: string[];
}

export interface TargetOperationRecord {
  operation_id: string;
  target_id: string;
  project_id: string;
  revision: number;
  status: TargetOperationStatus;
  execution_id?: string | null;
  spec: { kind: TargetOperationKind } & Record<string, unknown>;
  authority: TargetOperationAuthority;
  idempotency_key?: string | null;
  receipt?: TargetOperationReceipt | null;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface ExecStatus {
  exec_id?: string | null;
  target_id?: string | null;
  kind: "pending" | "running" | "stopped" | "exited" | "failed" | "denied" | "unknown" | string;
  ready: boolean;
  exit_code?: number | null;
  message?: string | null;
}

export interface LocalExecListResponse {
  executions: ExecStatus[];
}

export interface LocalExecStatusResponse {
  status: ExecStatus;
  error?: string | null;
}

export interface LocalExecLogLine {
  seq: number;
  stream: "stdout" | "stderr" | "system" | string;
  message_redacted: string;
}

export interface LocalExecLogsResponse {
  exec_id: string;
  lines: LocalExecLogLine[];
  next_seq?: number | null;
  error?: string | null;
}

export interface PortLeaseRecord {
  id: string;
  target_id: string;
  port_name: string;
  host: string;
  port: number;
  protocol: "tcp" | "udp" | string;
  status: "active" | "released" | string;
  bind?: "loopback_only" | string;
}

export interface ProxyRouteRecord {
  id: string;
  protocol: "http" | "websocket" | string;
  access: "host_authenticated" | "public";
  public_url: string;
  iframe_url: string;
  status: "active" | "removed" | string;
  ready: boolean;
  upstream: {
    port_lease_id: string;
    port_name: string;
  };
}

export class YggProtocolClient {
  private readonly accessToken?: string;
  private contractSelection?: ContractSelection;
  private contractDiagnostics: ContractDiagnostic[] = [];

  constructor(readonly baseUrl = "http://127.0.0.1:8787", accessToken?: string | null) {
    this.accessToken = accessToken === undefined ? resolveBrowserAccessToken() : accessToken || undefined;
  }

  invoke(method: string, params: unknown = {}) {
    return this.call(method, params);
  }

  async invokeWithSession(method: string, params: unknown = {}, sessionId: string): Promise<unknown> {
    const response = await this.fetchRpc({
      id: crypto.randomUUID(),
      method,
      params,
      session_id: sessionId,
      ...(this.contractSelection ? { contract: this.contractSelection } : {}),
    });
    await throwForHttpError(response);
    const envelope = (await response.json()) as ProtocolResponse<unknown>;
    this.captureContractDiagnostics(envelope.diagnostics);
    if (envelope.error) {
      throw new Error(`${envelope.error.code}: ${envelope.error.message}`);
    }
    return envelope.result;
  }

  async call<T>(method: string, params: unknown = {}): Promise<T> {
    const response = await this.fetchRpc({
      id: crypto.randomUUID(),
      method,
      params,
      ...(this.contractSelection ? { contract: this.contractSelection } : {}),
    });
    await throwForHttpError(response);
    const envelope = (await response.json()) as ProtocolResponse<T>;
    this.captureContractDiagnostics(envelope.diagnostics);
    if (envelope.error) {
      throw new Error(`${envelope.error.code}: ${envelope.error.message}`);
    }
    return envelope.result as T;
  }

  async negotiateHost(selection: ContractSelection): Promise<HostContractInfo> {
    const response = await this.fetchRpc({
      id: crypto.randomUUID(),
      method: "host.info",
      params: {},
      contract: selection,
    });
    await throwForHttpError(response);
    const envelope = (await response.json()) as ProtocolResponse<HostContractInfo>;
    this.captureContractDiagnostics(envelope.diagnostics);
    if (envelope.error) {
      throw new Error(`${envelope.error.code}: ${envelope.error.message}`);
    }
    this.contractSelection = selection;
    return envelope.result as HostContractInfo;
  }

  clearContractSelection(): void {
    this.contractSelection = undefined;
  }

  drainContractDiagnostics(): ContractDiagnostic[] {
    const diagnostics = this.contractDiagnostics;
    this.contractDiagnostics = [];
    return diagnostics;
  }

  private captureContractDiagnostics(diagnostics: ContractDiagnostic[] | undefined): void {
    if (diagnostics) this.contractDiagnostics.push(...diagnostics);
  }

  packages() {
    return this.call<PackageRecord[]>("kernel.v1.package.list");
  }

  packageStatus(packageId: string) {
    return this.call<PackageRecord>("kernel.v1.package.status", { package_id: packageId });
  }

  packageLogs(packageId: string) {
    return this.call<SubprocessLogLine[]>("kernel.v1.package.logs", { package_id: packageId });
  }

  capabilities() {
    return this.call<RegisteredCapability[]>("kernel.v1.capability.discover");
  }

  diagnostics() {
    return this.call<Record<string, unknown>>("kernel.v1.host.diagnostics");
  }

  listTargets() {
    return this.call<ExecutionTarget[]>("host.target.list");
  }

  targetStatus(targetId: string) {
    return this.call<ExecutionTarget>("host.target.status", { target_id: targetId });
  }

  listTargetOperations(targetId: string): Promise<TargetOperationRecord[]> {
    return this.fetchHostGetJson(`/host/v1/targets/${encodeURIComponent(targetId)}/operations`);
  }

  getTargetOperation(targetId: string, operationId: string): Promise<TargetOperationRecord> {
    return this.fetchHostGetJson(
      `/host/v1/targets/${encodeURIComponent(targetId)}/operations/${encodeURIComponent(operationId)}`,
    );
  }

  listExecs() {
    return this.call<LocalExecListResponse>("host.exec.list");
  }

  execStatus(execId: string) {
    return this.call<LocalExecStatusResponse>("host.exec.status", { exec_id: execId });
  }

  execLogs(execId: string, limit = 80) {
    return this.call<LocalExecLogsResponse>("host.exec.logs", { exec_id: execId, limit });
  }

  listPortLeases() {
    return this.call<PortLeaseRecord[]>("host.port.list");
  }

  portStatus(leaseId: string) {
    return this.call<PortLeaseRecord>("host.port.status", { lease_id: leaseId });
  }

  leasePort(input: PortLeaseRequest) {
    return this.call<PortLeaseRecord>("host.port.lease", input);
  }

  releasePort(leaseId: string) {
    return this.call<PortLeaseRecord>("host.port.release", { lease_id: leaseId });
  }

  listProxyRoutes() {
    return this.call<ProxyRouteRecord[]>("host.proxy.list");
  }

  proxyStatus(routeId: string) {
    return this.call<ProxyRouteRecord>("host.proxy.status", { route_id: routeId });
  }

  registerProxy(input: ProxyRegisterRequest) {
    return this.call<ProxyRouteRecord>("host.proxy.register", input);
  }

  unregisterProxy(routeId: string) {
    return this.call<ProxyRouteRecord>("host.proxy.unregister", { route_id: routeId });
  }

  assets() {
    return this.call<AssetRecord[]>("kernel.v1.asset.list");
  }

  projections() {
    return this.call<ProjectionRecord[]>("projection.list");
  }

  proposals() {
    return this.call<ProposalRecord[]>("change.proposal.list");
  }

  approveProposal(proposalId: string) {
    return this.call<ProposalRecord>("change.proposal.approve", { proposal_id: proposalId, reason: "web-forge" });
  }

  applyProposal(proposalId: string) {
    return this.call<ProposalRecord>("change.proposal.apply", { proposal_id: proposalId });
  }

  surfaceContributions(slot?: string) {
    return this.call<SurfaceContributionRecord[]>("shell.contribution.list", slot ? { slot } : {});
  }

  describeSurface(surfaceId: string) {
    return this.call<SurfaceContributionRecord>("shell.contribution.describe", { surface_id: surfaceId });
  }

  async listProjects(): Promise<ProjectRecord[]> {
    const result = await this.invoke("host.project.list", {});
    return (result as { projects: ProjectRecord[] }).projects;
  }

  async getProject(projectId: string): Promise<ProjectRecord & { state_details?: Record<string, unknown>; packages?: string[] }> {
    const result = await this.invoke("host.project.get", { project_id: projectId });
    const descriptor = result as {
      project?: Omit<ProjectRecord, "type" | "state" | "storage_summary"> & { type?: ProjectRecord["type"]; packages?: string[] };
      state?: ProjectRecord["state"];
      running_session_id?: string;
      storage_summary?: ProjectStorageSummary;
    };
    return {
      ...(descriptor.project as ProjectRecord),
      state: descriptor.state ?? "installed",
      running_session_id: descriptor.running_session_id,
      storage_summary: descriptor.storage_summary,
      packages: descriptor.project?.packages,
    };
  }

  async startProject(projectId: string): Promise<{
    project_id: string;
    previous_state: string;
    new_state: string;
    session_id: string;
    already_running: boolean;
  }> {
    return await this.invoke("host.project.start", { project_id: projectId }) as {
      project_id: string;
      previous_state: string;
      new_state: string;
      session_id: string;
      already_running: boolean;
    };
  }

  async stopProject(projectId: string): Promise<{ project_id: string; previous_state: string; new_state: string; session_id?: string }> {
    return await this.invoke("host.project.stop", { project_id: projectId }) as { project_id: string; previous_state: string; new_state: string; session_id?: string };
  }

  async getProjectStatus(projectId: string): Promise<{
    project_id: string;
    state: string;
    sessions_count: number;
    secrets_count: number;
    storage_summary?: ProjectStorageSummary;
  }> {
    return await this.invoke("host.project.status", { project_id: projectId }) as {
      project_id: string;
      state: string;
      sessions_count: number;
      secrets_count: number;
      storage_summary?: ProjectStorageSummary;
    };
  }

  openSession(labels: string[] = [], metadata: Record<string, unknown> = {}, activePackageSet: string[] = []) {
    return this.call<{ id: string }>("kernel.v1.session.open", {
      active_package_set: activePackageSet,
      labels,
      metadata,
    });
  }

  forkSession(parentSessionId: string, forkedFromSequence: number, metadata: Record<string, unknown> = {}) {
    return this.call<{ id: string }>("kernel.v1.session.fork", {
      parent_session_id: parentSessionId,
      forked_from_sequence: forkedFromSequence,
      metadata,
    });
  }

  invokeCapability<TOutput = unknown>(
    capabilityId: string,
    input: unknown,
    providerPackageId?: string,
    sessionId?: string,
  ): Promise<CapabilityInvocationResult<TOutput>> {
    return this.call("kernel.v1.capability.invoke", {
      capability_id: capabilityId,
      input,
      ...(providerPackageId ? { provider_package_id: providerPackageId } : {}),
      ...(sessionId ? { session_id: sessionId } : {}),
    });
  }

  private async invokeInstallLab<TOutput>(capabilityId: string, input: unknown): Promise<TOutput> {
    const session = await this.openSession(["install", "official/install-lab"], {
      source: "clients/web",
      capability_id: capabilityId,
    }, [INSTALL_LAB_PROVIDER]);
    const result = await this.invokeCapability<TOutput>(capabilityId, input, INSTALL_LAB_PROVIDER, session.id);
    return result.output;
  }

  private async invokeDockerRuntimeLab<TOutput>(capabilityId: string, input: unknown): Promise<TOutput> {
    const session = await this.openSession(["deploy", "official/docker-runtime-lab"], {
      source: "clients/web",
      capability_id: capabilityId,
    }, [DOCKER_RUNTIME_LAB_PROVIDER]);
    const result = await this.invokeCapability<TOutput>(capabilityId, input, DOCKER_RUNTIME_LAB_PROVIDER, session.id);
    return result.output;
  }

  async startDockerContainer(input: DockerStartContainerInput): Promise<DockerStartContainerOutput> {
    return await this.invokeDockerRuntimeLab<DockerStartContainerOutput>(DOCKER_RUNTIME_LAB_CAPABILITIES.startContainer, input);
  }

  async stopDockerContainer(input: DockerStopContainerInput): Promise<DockerStopContainerOutput> {
    return await this.invokeDockerRuntimeLab<DockerStopContainerOutput>(DOCKER_RUNTIME_LAB_CAPABILITIES.stopContainer, input);
  }

  deployProject(input: HostDeployProjectInput): Promise<HostDeployProjectOutput> {
    return this.fetchHostJson("/host/v1/deploy", input);
  }

  stopProjectDeployment(input: { route_id: string }): Promise<HostStopProjectDeploymentOutput> {
    return this.fetchHostJson("/host/v1/deploy/stop", input);
  }

  buildDeployProject(input: HostBuildDeployRequest, options: { wait?: boolean } = {}): Promise<BuildDeployJobSubmitResponse | BuildDeployJobStatusResponse> {
    const suffix = options.wait ? "?wait=true" : "";
    return this.fetchHostJson(`/host/v1/build-deploy${suffix}`, input);
  }

  getBuildDeployJob(jobId: string): Promise<BuildDeployJobStatusResponse> {
    return this.fetchHostGetJson(`/host/v1/build-deploy/${encodeURIComponent(jobId)}`);
  }

  cancelBuildDeployJob(jobId: string): Promise<BuildDeployCancelResponse> {
    return this.fetchHostJson(`/host/v1/build-deploy/${encodeURIComponent(jobId)}/cancel`, {});
  }

  getProjectDeployments(projectId: string): Promise<ProjectDeploymentsResponse> {
    return this.fetchHostGetJson(`/host/v1/projects/${encodeURIComponent(projectId)}/deployments`);
  }

  recoverProjectDeployment(projectId: string): Promise<DeploymentActionResponse> {
    return this.fetchHostJson(`/host/v1/projects/${encodeURIComponent(projectId)}/deployments/recover`, {});
  }

  rollbackProjectDeployment(projectId: string, revisionId: string): Promise<DeploymentActionResponse> {
    return this.fetchHostJson(`/host/v1/projects/${encodeURIComponent(projectId)}/deployments/rollback`, {
      revision_id: revisionId,
    });
  }

  listProjectChanges(projectId: string): Promise<DevelopmentChangeListResponse> {
    return this.fetchHostGetJson(`/host/v1/projects/${encodeURIComponent(projectId)}/changes`);
  }

  getProjectChange(projectId: string, changeSetId: string): Promise<DevelopmentChangeRecord> {
    return this.fetchHostGetJson(`/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}`);
  }

  getProjectChangeBundle(projectId: string, changeSetId: string): Promise<DevelopmentPatchBundle> {
    return this.fetchHostGetJson(`/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/bundle`);
  }

  draftProjectChange(projectId: string, input: DevelopmentDraftRequest): Promise<DevelopmentChangeRecord> {
    return this.fetchHostJson(`/host/v1/projects/${encodeURIComponent(projectId)}/changes`, input);
  }

  approveProjectChange(projectId: string, changeSetId: string, approved: boolean, reason?: string): Promise<DevelopmentChangeRecord> {
    return this.fetchHostJson(`/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/approve`, {
      approved,
      ...(reason ? { reason } : {}),
    });
  }

  executeProjectChange(projectId: string, changeSetId: string): Promise<DevelopmentExecuteResponse> {
    return this.fetchHostJson(`/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/execute`, {});
  }

  recoverProjectChange(projectId: string, changeSetId: string): Promise<DevelopmentChangeRecord> {
    return this.fetchHostJson(`/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/recover`, {});
  }

  createProjectDeploymentPreview(
    projectId: string,
    changeSetId: string,
    input: DevelopmentDeploymentPreviewRequest,
  ): Promise<DevelopmentChangeRecord> {
    return this.fetchHostJson(
      `/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/deployment/preview`,
      input,
    );
  }

  approveProjectDeployment(
    projectId: string,
    changeSetId: string,
    input: DevelopmentDeploymentApprovalRequest,
  ): Promise<DevelopmentChangeRecord> {
    return this.fetchHostJson(
      `/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/deployment/approve`,
      input,
    );
  }

  activateProjectDeployment(projectId: string, changeSetId: string): Promise<DevelopmentChangeRecord> {
    return this.fetchHostJson(
      `/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/deployment/activate`,
      {},
    );
  }

  reconcileProjectDeployment(projectId: string, changeSetId: string): Promise<DevelopmentChangeRecord> {
    return this.fetchHostJson(
      `/host/v1/projects/${encodeURIComponent(projectId)}/changes/${encodeURIComponent(changeSetId)}/deployment/reconcile`,
      {},
    );
  }

  subscribeBuildDeployJob(jobId: string, onEvent: (event: BuildDeployJobEvent) => void, onError?: (error: Event) => void) {
    const source = new EventSource(this.buildDeployJobEventsUrl(jobId));
    source.addEventListener("build_deploy", (message) => onEvent(JSON.parse((message as MessageEvent).data)));
    if (onError) source.addEventListener("error", onError);
    return () => source.close();
  }

  async resolveInstallPlan(source: InstallSource): Promise<InstallPlan> {
    const rootUrl = normalizeInstallRootUrl(source.root_url);
    if (!/^https:\/\//i.test(rootUrl)) {
      throw new Error("The web install flow accepts public HTTPS Git URLs only. Use the CLI for local folders.");
    }
    const output = await this.invokeInstallLab<{ plan: InstallPlan }>(INSTALL_LAB_CAPABILITIES.resolvePlan, {
      root_url: rootUrl,
      root_ref: source.root_ref ?? "HEAD",
      ...(source.lockfile ? { lockfile: source.lockfile } : {}),
      ...(source.require_signed !== undefined ? { require_signed: source.require_signed } : {}),
      ...(source.strict_conformance !== undefined ? { strict_conformance: source.strict_conformance } : {}),
    });
    return output.plan;
  }

  async detectInstallKind(source: Pick<InstallSource, "root_url" | "root_ref">): Promise<InstallDetectedKind> {
    const rootUrl = normalizeInstallRootUrl(source.root_url);
    if (!/^https:\/\//i.test(rootUrl)) {
      throw new Error("The web install flow accepts public HTTPS Git URLs only. Use the CLI for local folders.");
    }
    const isLocalSource = false;
    return await this.invokeInstallLab<InstallDetectedKind>(INSTALL_LAB_CAPABILITIES.detectKind, {
      [isLocalSource ? "path" : "url"]: rootUrl,
      root_ref: source.root_ref ?? "HEAD",
    });
  }

  async executeInstallPlan(
    plan: InstallPlan,
    consent: InstallConsent = {
      approved_capabilities: plan.permissions_summary.new_capabilities,
      approved_network_hosts: plan.permissions_summary.new_network_hosts,
      approved_secret_refs: plan.permissions_summary.new_secret_refs,
    },
    profile = resolveManagedProfile(),
  ): Promise<InstallExecuteResult> {
    return await this.invokeInstallLab<InstallExecuteResult>(INSTALL_LAB_CAPABILITIES.executePlan, {
      plan,
      consent,
      profile,
    });
  }

  async uninstallProject(projectId: string, profile = resolveManagedProfile()): Promise<InstallUninstallResult> {
    return await this.invokeInstallLab<InstallUninstallResult>(INSTALL_LAB_CAPABILITIES.uninstall, {
      project_id: projectId,
      profile,
      delete_project_data: false,
    });
  }

  async checkProjectUpdates(projectId: string, profile = resolveManagedProfile()): Promise<UpdateCheckResult> {
    return await this.invokeInstallLab<UpdateCheckResult>(INSTALL_LAB_CAPABILITIES.checkForUpdates, {
      project_id: projectId,
      profile,
    });
  }

  async updateProject(projectId: string, profile = resolveManagedProfile(), force = false): Promise<ProjectUpdateResult> {
    return await this.invokeInstallLab<ProjectUpdateResult>(INSTALL_LAB_CAPABILITIES.updateProject, {
      project_id: projectId,
      profile,
      force,
    });
  }

  listEvents(sessionId: string) {
    return this.call<KernelEvent[]>("kernel.v1.event.list", { session_id: sessionId, limit: 50 });
  }

  subscribeEvents(sessionId: string | undefined, onEvent: (event: KernelEvent) => void) {
    const targetSession = sessionId ?? "kernel_project_lifecycle";
    const source = new EventSource(this.eventSubscribeUrl(targetSession));
    source.addEventListener("kernel.v1.event", (message) => onEvent(JSON.parse((message as MessageEvent).data)));
    return () => source.close();
  }

  private rpcHeaders(): Record<string, string> {
    return {
      "content-type": "application/json",
      ...(this.accessToken ? { authorization: `Bearer ${this.accessToken}` } : {}),
    };
  }

  private async fetchRpc(body: unknown): Promise<Response> {
    try {
      return await fetch(`${this.baseUrl}/rpc`, {
        method: "POST",
        headers: this.rpcHeaders(),
        body: JSON.stringify(body),
      });
    } catch (err: unknown) {
      if (isFetchTransportError(err)) {
        throw new Error(
          "Cannot reach the Plurora host RPC. Check that the host is still running, the access token is valid, and the deployment did not time out while resolving the install plan.",
        );
      }
      throw err;
    }
  }

  private async fetchHostJson<T>(path: string, body: unknown): Promise<T> {
    try {
      const response = await fetch(`${this.baseUrl}${path}`, {
        method: "POST",
        headers: this.rpcHeaders(),
        body: JSON.stringify(body),
      });
      await throwForHttpError(response);
      return (await response.json()) as T;
    } catch (err: unknown) {
      if (isFetchTransportError(err)) {
        throw new Error("Cannot reach the Plurora Host control API. Check that the Host is reachable and the access token is valid.");
      }
      throw err;
    }
  }

  private async fetchHostGetJson<T>(path: string): Promise<T> {
    try {
      const response = await fetch(`${this.baseUrl}${path}`, {
        method: "GET",
        headers: this.rpcHeaders(),
      });
      await throwForHttpError(response);
      return (await response.json()) as T;
    } catch (err: unknown) {
      if (isFetchTransportError(err)) {
        throw new Error("Cannot reach the Plurora Host control API. Check that the Host is reachable and the access token is valid.");
      }
      throw err;
    }
  }

  private eventSubscribeUrl(sessionId: string): string {
    const url = new URL(`${this.baseUrl}/kernel/v1/event.subscribe/${encodeURIComponent(sessionId)}`);
    if (this.accessToken) {
      url.searchParams.set("access_token", this.accessToken);
    }
    return url.toString();
  }

  private buildDeployJobEventsUrl(jobId: string): string {
    const url = new URL(`${this.baseUrl}/host/v1/build-deploy/${encodeURIComponent(jobId)}/events`);
    if (this.accessToken) {
      url.searchParams.set("access_token", this.accessToken);
    }
    return url.toString();
  }

  /* ────────────────────────────────────────────────────────────────
     Secret store — wraps `official/secret-store-lab` capabilities.
     The host injects raw values via secret_ref; the UI never reads
     raw secret values.
     ──────────────────────────────────────────────────────────────── */

  async secretsHealth(): Promise<{
    exists: boolean;
    secret_count: number;
    key_source: string;
  }> {
    return (await this.invokeCapability<{
      exists: boolean;
      secret_count: number;
      key_source: string;
    }>("official/secret-store-lab/health", {})).output;
  }

  async listSecrets(projectId?: string): Promise<string[]> {
    if (projectId) {
      const result = (await this.invokeCapability<{ names: string[] }>("official/secret-store-lab/list_project_secrets", {
        project_id: projectId,
      })).output;
      return result.names ?? [];
    }
    const result = (await this.invokeCapability<{ names: string[] }>("official/secret-store-lab/list_secrets", {})).output;
    return result.names ?? [];
  }

  async putSecret(name: string, value: string, projectId?: string): Promise<{ created: boolean }> {
    const capability = projectId
      ? "official/secret-store-lab/put_project_secret"
      : "official/secret-store-lab/put_secret";
    const params = projectId ? { project_id: projectId, name, value } : { name, value };
    const result = (await this.invokeCapability<{ created: boolean }>(capability, params)).output;
    return { created: result.created };
  }

  async deleteSecret(name: string, projectId?: string): Promise<{ removed: boolean }> {
    const capability = projectId
      ? "official/secret-store-lab/delete_project_secret"
      : "official/secret-store-lab/delete_secret";
    const params = projectId ? { project_id: projectId, name } : { name };
    const result = (await this.invokeCapability<{ removed: boolean }>(capability, params)).output;
    return { removed: result.removed };
  }
}

function isFetchTransportError(err: unknown): boolean {
  if (!(err instanceof TypeError)) return false;
  const message = err.message.toLowerCase();
  return message.includes("failed to fetch") || message.includes("networkerror") || message.includes("load failed");
}

async function throwForHttpError(response: Response): Promise<void> {
  if (response.ok) return;

  const body = await response.text().catch(() => response.statusText || "HTTP error");
  throw new ProtocolHttpError(response.status, body);
}
