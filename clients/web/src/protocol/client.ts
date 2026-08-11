import type {
  ArtifactDescriptor,
  BindingCandidatesRequest,
  BindingCandidatesResult,
  BindingListRequest,
  BindingMutationResult,
  BindingRevokeRequest,
  BindingSelectRequest,
  ExposureCreateRequest,
  ExposureListRequest,
  ExposureMutationResult,
  ExposureRevokeRequest,
  HostBindingListResult,
  HostExposureListResult,
  HostInstallationListResult,
  HostRealizationListResult,
  HostRunListResult,
  InstallationCreateRequest,
  InstallationMutationResult,
  InstallationRemoveRequest,
  InstallationStateArtifactGetResponse,
  InstallationStatus,
  InstallationUpdateRequest,
  InstallationView,
  RealizationApplyRequest,
  RealizationGetRequest,
  RealizationListRequest,
  RealizationMutationResult,
  RealizationPlanRequest,
  RealizationPlanResult,
  RealizationReconcileRequest,
  RealizationRevision,
  RealizationRollbackRequest,
  RealizationStopRequest,
  RunGetRequest,
  RunListRequest,
  RunMutationResult,
  RunStartRequest,
  RunStartResult,
  RunStatusRequest,
  RunStatusView,
  RunStopRequest,
  RunView,
} from "./generated-types";

export interface ProtocolResponse<T = unknown> {
  id: string;
  result?: T;
  error?: { code: string; message: string; details?: unknown };
}

export type ContractOwnerLayer =
  | "substrate"
  | "host"
  | "protocol"
  | "shell";

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
  installation_descriptor?: unknown;
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
  installation?: { installation_id?: string } | null;
}

/** Structured public-contract error. UI can render reason codes without parsing prose. */
export class ProtocolRpcError extends Error {
  readonly reasonCode: string;
  readonly nextStep?: string;
  readonly details: ProtocolFailureDetails;

  constructor(
    readonly code: string,
    _rawMessage: string,
    details?: unknown,
  ) {
    const safe = sanitizeProtocolFailureDetails(code, details);
    super(safe.reason_code);
    this.name = "ProtocolRpcError";
    this.reasonCode = safe.reason_code;
    this.nextStep = safe.next_step;
    this.details = safe;
  }
}

export interface ProtocolFailureDetails {
  reason_code: string;
  next_step?: string;
  installation_id?: string;
  run_id?: string;
  port_id?: string;
  target_id?: string;
  realization_id?: string;
}

function sanitizeProtocolFailureDetails(code: string, value: unknown): ProtocolFailureDetails {
  const record = value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
  const safeString = (field: string): string | undefined => {
    const candidate = record[field];
    return typeof candidate === "string" ? candidate : undefined;
  };
  return {
    reason_code: safeString("reason_code") ?? code,
    ...(safeString("next_step") ? { next_step: safeString("next_step") } : {}),
    ...(safeString("installation_id") ? { installation_id: safeString("installation_id") } : {}),
    ...(safeString("run_id") ? { run_id: safeString("run_id") } : {}),
    ...(safeString("port_id") ? { port_id: safeString("port_id") } : {}),
    ...(safeString("target_id") ? { target_id: safeString("target_id") } : {}),
    ...(safeString("realization_id") ? { realization_id: safeString("realization_id") } : {}),
  };
}

export interface InstallUninstallResult {
  removed_from_profile: boolean;
  store_path_orphaned?: string | null;
  store_paths_orphaned?: string[];
  installation?: { installation_id: string; data_action: string } | null;
}

export interface UpdateCheckRecord {
  id?: string;
  package_id?: string;
  installation_id?: string | null;
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

export interface InstallationUpdateResult {
  status?: string;
  updated?: boolean;
  updated_packages?: string[];
  reason?: string;
  check?: UpdateCheckResult;
  execute?: unknown;
  store_gc?: unknown;
}

const INSTALL_LAB_PROVIDER = "plurora/install-lab";
const INSTALL_LAB_CAPABILITIES = {
  resolvePlan: `${INSTALL_LAB_PROVIDER}/resolve_plan`,
  detectKind: `${INSTALL_LAB_PROVIDER}/detect_kind`,
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

export interface PlatformEvent {
  id: string;
  session_id: string;
  sequence: number;
  writer_package_id: string;
  kind: string;
  payload: unknown;
  metadata: unknown;
  created_at: string;
}

/** Public Host relay sessions. Private Powerbox journal events never use these. */
export const HOST_EVENT_SESSIONS = {
  installationLifecycle: "host_installation_lifecycle",
  powerbox: "host_powerbox",
  realizations: "host_realizations",
} as const;

export const HOST_INSTALLATION_LIFECYCLE_SESSION = HOST_EVENT_SESSIONS.installationLifecycle;
export const HOST_POWERBOX_SESSION = HOST_EVENT_SESSIONS.powerbox;
export const HOST_REALIZATIONS_SESSION = HOST_EVENT_SESSIONS.realizations;
export const HOST_POWERBOX_RELAY_SESSIONS = [
  HOST_EVENT_SESSIONS.powerbox,
  HOST_EVENT_SESSIONS.installationLifecycle,
] as const;

export type HostEventSessionId = typeof HOST_EVENT_SESSIONS[keyof typeof HOST_EVENT_SESSIONS];
export type EventSubscriptionOptions = { signal?: AbortSignal };

export const PUBLIC_POWERBOX_EVENT_KINDS = [
  "host/exposure.created",
  "host/exposure.revoked",
  "host/exposure.expired",
  "host/binding.selected",
  "host/binding.revoked",
  "host/binding.expired",
] as const;
export type PublicPowerboxEventKind = typeof PUBLIC_POWERBOX_EVENT_KINDS[number];

export const PUBLIC_REALIZATION_EVENT_KINDS = [
  "host/realization.planned",
  "host/realization.applying",
  "host/realization.active",
  "host/realization.stopped",
  "host/realization.failed",
  "host/realization.rolled_back",
  "host/realization.reconciled",
] as const;
export type PublicRealizationEventKind = typeof PUBLIC_REALIZATION_EVENT_KINDS[number];

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

export type {
  AcquisitionKind,
  AcquisitionRecord,
  ActiveBindingRecord,
  ArtifactDescriptor,
  AvailabilityPolicy,
  BindingCandidate,
  BindingCandidatesRequest,
  BindingCandidatesResult,
  BindingComponentDisclosure,
  BindingDecisionStatus,
  BindingEffectiveStatus,
  BindingEndpointPin,
  BindingGap,
  BindingId,
  BindingLifecyclePayloadSchema,
  BindingListRequest,
  BindingInstallationDisclosure,
  BindingLock,
  BindingMutationResult,
  BindingPhase,
  BindingRevokeRequest,
  BindingSelectRequest,
  BindingSelectionRecord,
  BindingView,
  BindingWorkDisclosure,
  CapabilityPin,
  ClaimStatus,
  ComponentPin,
  ComponentBoundaryClaims,
  ComponentClaimStatus,
  ComponentTrustClass,
  ExposureCreateRequest,
  ExposureId,
  ExposureLifecyclePayloadSchema,
  ExposureListRequest,
  ExposureMutationResult,
  ExposureRecord,
  ExposureRevokeRequest,
  ExposureStatus,
  ExposureView,
  HostBindingListResult,
  HostExposureListResult,
  HostInstallationListResult,
  HostRealizationListResult,
  HostRunListResult,
  InstallationChangeForArtifactDescriptor,
  InstallationChangeForAssemblyBinding,
  InstallationChangeForAssemblyNode,
  InstallationChangeForAssemblyPortExposure,
  InstallationChangeForBindingLock,
  InstallationChangeForNodeLock,
  InstallationChangeForProtocolProfilePin,
  InstallationChangeForWorkEntrypoint,
  InstallationCreateRequest,
  InstallationDiff,
  InstallationId,
  InstallationItemDiffForArtifactDescriptor,
  InstallationItemDiffForAssemblyBinding,
  InstallationItemDiffForAssemblyNode,
  InstallationItemDiffForAssemblyPortExposure,
  InstallationItemDiffForBindingLock,
  InstallationItemDiffForNodeLock,
  InstallationItemDiffForProtocolProfilePin,
  InstallationItemDiffForWorkEntrypoint,
  InstallationMutationResult,
  InstallationRecord,
  InstallationRemoveRequest,
  InstallationRollbackPointer,
  InstallationSecretPolicy,
  InstallationStateAction,
  InstallationStateArtifactGetParams,
  InstallationStateArtifactGetResponse,
  InstallationStateAuthorityEvidence,
  InstallationStateDecision,
  InstallationStateDecisionAction,
  InstallationStateDecisionReceipt,
  InstallationStateSnapshot,
  InstallationStateSnapshotEntry,
  InstallationStateSlotChange,
  InstallationStateSlotDiff,
  InstallationStateSlotRequirement,
  InstallationStatus,
  InstallationUpdateRequest,
  InstallationView,
  InstallationWorkSummary,
  PortId,
  PortDescriptor,
  PortRole,
  ResolvedPortPin,
  ResourceSelector,
  RightDisposition,
  RightsDeclaration,
  RealizationApplyRequest,
  RealizationApproval,
  RealizationBackendSelection,
  RealizationGetRequest,
  RealizationHealth,
  RealizationId,
  RealizationLifecyclePayloadSchema,
  RealizationListRequest,
  RealizationMutationResult,
  RealizationPlan,
  RealizationPlanRequest,
  RealizationPlanResult,
  RealizationPlanningGap,
  RealizationReconcileRequest,
  RealizationRevision,
  RealizationRollbackRequest,
  RealizationStatus,
  RealizationStopRequest,
  RealizedResource,
  RunEntrypointPreflight,
  RunGap,
  RunGetRequest,
  RunHealth,
  RunId,
  RunListRequest,
  RunMutationResult,
  RunRecord,
  RunRevisionPin,
  RunStartRequest,
  RunStartResult,
  RunStatus,
  RunStatusRequest,
  RunStatusView,
  RunStopRequest,
  RunView,
  SelectedTransport,
  StateAction,
  StateActionKind,
  StateBindingKind,
  StateBindingRecord,
  StateDisposition,
  StatePortability,
  StateSlotId,
  SourceVisibility,
  TransportRequirements,
  TransparencyDeclaration,
  WorkId,
  WorkEntrypoint,
  WorkEntrypointTarget,
} from "./generated-types";

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

export interface InstallationStorageSummary {
  data_bytes: number | null;
  cache_bytes: number | null;
  bundle_bytes: number | null;
  log_bytes: number | null;
  total_bytes: number | null;
  measured_at: string | null;
  measurement_state: "measured" | "unknown" | string;
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

export interface ExecutionTarget {
  id: string;
  name: string;
  reachability: "local_host" | string;
  status: "available" | "unavailable" | string;
  capabilities?: Array<"local_exec" | "port_lease" | "http_proxy_upstream" | "websocket_proxy_upstream" | string>;
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

export class PluroraProtocolClient {
  private readonly accessToken?: string;
  private contractSelection?: ContractSelection;

  constructor(readonly baseUrl = "http://127.0.0.1:8787", accessToken?: string | null) {
    this.accessToken = accessToken === undefined ? resolveBrowserAccessToken() : accessToken || undefined;
  }

  invoke<T = unknown>(method: string, params: unknown = {}): Promise<T> {
    return this.call<T>(method, params);
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
    if (envelope.error) throw new ProtocolRpcError(envelope.error.code, envelope.error.message, envelope.error.details);
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
    if (envelope.error) throw new ProtocolRpcError(envelope.error.code, envelope.error.message, envelope.error.details);
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
    if (envelope.error) {
      throw new Error(`${envelope.error.code}: ${envelope.error.message}`);
    }
    this.contractSelection = selection;
    return envelope.result as HostContractInfo;
  }

  clearContractSelection(): void {
    this.contractSelection = undefined;
  }

  packages() {
    return this.call<PackageRecord[]>("host.package.list");
  }

  packageStatus(packageId: string) {
    return this.call<PackageRecord>("host.package.status", { package_id: packageId });
  }

  packageLogs(packageId: string) {
    return this.call<SubprocessLogLine[]>("host.package.logs", { package_id: packageId });
  }

  capabilities() {
    return this.call<RegisteredCapability[]>("capability.discover");
  }

  diagnostics() {
    return this.call<Record<string, unknown>>("host.diagnostics");
  }

  listTargets() {
    return this.call<ExecutionTarget[]>("host.target.list");
  }

  targetStatus(targetId: string) {
    return this.call<ExecutionTarget>("host.target.status", { target_id: targetId });
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
    return this.call<AssetRecord[]>("object.list");
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

  async listInstallations(status?: InstallationStatus): Promise<InstallationView[]> {
    const result = await this.invoke<HostInstallationListResult>("host.installation.list", status ? { status } : {});
    return result;
  }

  getInstallation(installationId: string): Promise<InstallationView> {
    return this.invoke<InstallationView>("host.installation.get", { installation_id: installationId });
  }

  createInstallation(input: InstallationCreateRequest): Promise<InstallationMutationResult> {
    return this.invoke<InstallationMutationResult>("host.installation.create", input);
  }

  updateInstallation(input: InstallationUpdateRequest): Promise<InstallationMutationResult> {
    return this.invoke<InstallationMutationResult>("host.installation.update", input);
  }

  getInstallationStateArtifact(
    installationId: string,
    descriptor: ArtifactDescriptor,
  ): Promise<InstallationStateArtifactGetResponse> {
    return this.invoke<InstallationStateArtifactGetResponse>("object.get", {
      installation_id: installationId,
      installation_state_artifact: descriptor,
    });
  }

  removeInstallation(input: InstallationRemoveRequest): Promise<InstallationMutationResult> {
    return this.invoke<InstallationMutationResult>("host.installation.remove", input);
  }

  listRuns(input: RunListRequest = {}): Promise<HostRunListResult> {
    return this.invoke<HostRunListResult>("host.run.list", input);
  }

  getRun(input: RunGetRequest): Promise<RunView> {
    return this.invoke<RunView>("host.run.get", input);
  }

  statusRun(input: RunStatusRequest): Promise<RunStatusView> {
    return this.invoke<RunStatusView>("host.run.status", input);
  }

  startRun(input: RunStartRequest): Promise<RunStartResult> {
    return this.invoke<RunStartResult>("host.run.start", input);
  }

  stopRun(input: RunStopRequest): Promise<RunMutationResult> {
    return this.invoke<RunMutationResult>("host.run.stop", input);
  }

  listRealizations(input: RealizationListRequest = {}): Promise<HostRealizationListResult> {
    return this.invoke<HostRealizationListResult>("host.realization.list", input);
  }

  getRealization(input: RealizationGetRequest): Promise<RealizationRevision> {
    return this.invoke<RealizationRevision>("host.realization.get", input);
  }

  planRealization(input: RealizationPlanRequest): Promise<RealizationPlanResult> {
    return this.invoke<RealizationPlanResult>("host.realization.plan", input);
  }

  applyRealization(input: RealizationApplyRequest): Promise<RealizationMutationResult> {
    return this.invoke<RealizationMutationResult>("host.realization.apply", input);
  }

  stopRealization(input: RealizationStopRequest): Promise<RealizationMutationResult> {
    return this.invoke<RealizationMutationResult>("host.realization.stop", input);
  }

  rollbackRealization(input: RealizationRollbackRequest): Promise<RealizationMutationResult> {
    return this.invoke<RealizationMutationResult>("host.realization.rollback", input);
  }

  reconcileRealization(input: RealizationReconcileRequest): Promise<RealizationMutationResult> {
    return this.invoke<RealizationMutationResult>("host.realization.reconcile", input);
  }

  listExposures(input: ExposureListRequest = {}): Promise<HostExposureListResult> {
    return this.invoke<HostExposureListResult>("host.exposure.list", input);
  }

  createExposure(input: ExposureCreateRequest): Promise<ExposureMutationResult> {
    return this.invoke<ExposureMutationResult>("host.exposure.create", input);
  }

  revokeExposure(input: ExposureRevokeRequest): Promise<ExposureMutationResult> {
    return this.invoke<ExposureMutationResult>("host.exposure.revoke", input);
  }

  listBindings(input: BindingListRequest = {}): Promise<HostBindingListResult> {
    return this.invoke<HostBindingListResult>("host.binding.list", input);
  }

  bindingCandidates(input: BindingCandidatesRequest): Promise<BindingCandidatesResult> {
    return this.invoke<BindingCandidatesResult>("host.binding.candidates", input);
  }

  selectBinding(input: BindingSelectRequest): Promise<BindingMutationResult> {
    return this.invoke<BindingMutationResult>("host.binding.select", input);
  }

  revokeBinding(input: BindingRevokeRequest): Promise<BindingMutationResult> {
    return this.invoke<BindingMutationResult>("host.binding.revoke", input);
  }

  openSession(labels: string[] = [], metadata: Record<string, unknown> = {}, activePackageSet: string[] = []) {
    return this.call<{ id: string }>("context.open", {
      active_package_set: activePackageSet,
      labels,
      metadata,
    });
  }

  forkSession(parentSessionId: string, forkedFromSequence: number, metadata: Record<string, unknown> = {}) {
    return this.call<{ id: string }>("context.fork", {
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
    return this.call("capability.invoke", {
      capability_id: capabilityId,
      input,
      ...(providerPackageId ? { provider_package_id: providerPackageId } : {}),
      ...(sessionId ? { session_id: sessionId } : {}),
    });
  }

  private async invokeInstallLab<TOutput>(capabilityId: string, input: unknown): Promise<TOutput> {
    const session = await this.openSession(["install", "plurora/install-lab"], {
      source: "clients/web",
      capability_id: capabilityId,
    }, [INSTALL_LAB_PROVIDER]);
    const result = await this.invokeCapability<TOutput>(capabilityId, input, INSTALL_LAB_PROVIDER, session.id);
    return result.output;
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

  listEvents(sessionId: string) {
    return this.call<PlatformEvent[]>("journal.list", { session_id: sessionId, limit: 50 });
  }

  subscribeEvents(
    sessionId: string | undefined,
    onEvent: (event: PlatformEvent) => void,
    options: EventSubscriptionOptions = {},
  ): () => void {
    const targetSession = sessionId ?? HOST_EVENT_SESSIONS.installationLifecycle;
    if (options.signal?.aborted) return () => undefined;
    const source = new EventSource(this.eventSubscribeUrl(targetSession));
    let closed = false;
    source.addEventListener("journal.event", (message) => {
      if (!closed) onEvent(JSON.parse((message as MessageEvent).data));
    });
    const close = () => {
      if (closed) return;
      closed = true;
      source.close();
      options.signal?.removeEventListener("abort", close);
    };
    options.signal?.addEventListener("abort", close, { once: true });
    return close;
  }

  /** Subscribe to one or more public Host relay sessions with shared cleanup. */
  subscribeHostEvents(
    sessions: readonly HostEventSessionId[],
    onEvent: (event: PlatformEvent) => void,
    options: EventSubscriptionOptions = {},
  ): () => void {
    const uniqueSessions = [...new Set(sessions)];
    const close = uniqueSessions.map((session) => this.subscribeEvents(session, onEvent, options));
    let closed = false;
    return () => {
      if (closed) return;
      closed = true;
      close.forEach((unsubscribe) => unsubscribe());
    };
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

  private eventSubscribeUrl(sessionId: string): string {
    const url = new URL(`${this.baseUrl}/journal/subscribe/${encodeURIComponent(sessionId)}`);
    if (this.accessToken) {
      url.searchParams.set("access_token", this.accessToken);
    }
    return url.toString();
  }

  /* ────────────────────────────────────────────────────────────────
     Secret store — wraps `plurora/secret-store-lab` capabilities.
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
    }>("plurora/secret-store-lab/health", {})).output;
  }

  async listSecrets(installationId?: string): Promise<string[]> {
    if (installationId) {
      const result = (await this.invokeCapability<{ names: string[] }>("plurora/secret-store-lab/list_installation_secrets", {
        installation_id: installationId,
      })).output;
      return result.names ?? [];
    }
    const result = (await this.invokeCapability<{ names: string[] }>("plurora/secret-store-lab/list_secrets", {})).output;
    return result.names ?? [];
  }

  async putSecret(name: string, value: string, installationId?: string): Promise<{ created: boolean }> {
    const capability = installationId
      ? "plurora/secret-store-lab/put_installation_secret"
      : "plurora/secret-store-lab/put_secret";
    const params = installationId ? { installation_id: installationId, name, value } : { name, value };
    const result = (await this.invokeCapability<{ created: boolean }>(capability, params)).output;
    return { created: result.created };
  }

  async deleteSecret(name: string, installationId?: string): Promise<{ removed: boolean }> {
    const capability = installationId
      ? "plurora/secret-store-lab/delete_installation_secret"
      : "plurora/secret-store-lab/delete_secret";
    const params = installationId ? { installation_id: installationId, name } : { name };
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
