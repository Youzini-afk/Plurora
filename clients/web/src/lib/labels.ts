import type { SupportedLocale } from "@/lib/locale";

export interface LocaleDictionary {
  languageName: string;
  languageShort: string;
  languageMenuLabel: string;
  languageAria: (label: string) => string;

  authEyebrow: string;
  authTitle: string;
  authBody: string;
  authTokenLabel: string;
  authHideToken: string;
  authShowToken: string;
  authPlaceholder: string;
  authCheckingButton: string;
  authSubmitButton: string;
  authStoredLocally: string;
  authCheckingAccess: string;
  authInvalidToken: string;
  authConnectionFailed: (message: string) => string;

  pairEyebrow: string;
  pairTitle: string;
  pairBody: string;
  pairLoading: string;
  pairMissingBody: string;
  pairInvalidTitle: string;
  pairInvalidBody: string;
  pairDevice: string;
  pairPermissions: string;
  pairResources: string;
  pairGrantExpires: string;
  pairSecureRequiredTitle: string;
  pairSecureRequiredBody: string;
  pairConfirm: string;
  pairClaiming: string;
  pairCompleteTitle: string;
  pairCompleteBody: string;
  pairOpenHost: string;

  topbarHome: string;
  topbarSettings: string;
  topbarInstallation: (installationId: string) => string;
  topbarNotifications: string;
  topbarThemeSystem: (theme: string) => string;
  topbarThemeLight: string;
  topbarThemeDark: string;
  topbarThemeAria: (preference: string) => string;
  topbarLogout: string;

  homeGreeting: string;
  homeEmptyWorkshop: string;
  homeReading: string;
  homeShelfSummary: (all: number, running: number, stopped: number, failed: number) => string;
  homeInstalledEyebrow: (count: number) => string;
  homeEmptyTitle: string;
  homeEmptyBody: string;
  homeInstallLabel: string;
  homeInstallHint: string;
  homeErrorTitle: string;
  homeErrorBody: string;
  retry: string;
  homeSearchPlaceholder: string;
  homeSortPrefix: string;
  homeSortRecent: string;
  homeFilterAll: string;
  homeFilterRunning: string;
  homeFilterStopped: string;
  homeFilterFailed: string;
  homeActivityLast24h: string;
  homeActivityNo24h: string;
  homeViewFullAuditLog: string;
  homeWorkshop: string;
  homeUpdates: string;
  homeUpdatesAvailable: (count: number) => string;
  homeEverythingUpToDate: string;
  homeUpdate: string;
  homeUpdateAll: string;
  homeDiskUsage: string;
  homeDiskUsed: (value: string) => string;
  homeUnknown: string;
  homeMeasuring: string;
  homeNoStorageMeasured: string;
  homeManageStorage: string;
  homeWorkshopCards: string;
  homeWorkshopCategoryTool: string;
  homeWorkshopCategoryTemplate: string;
  homeWorkshopCategoryExample: string;
  homeQuickActions: string;
  homeCapabilityCards: string;
  homeQuickInstallUrl: string;
  homeQuickDataFolder: string;
  homeQuickSettings: string;
  homeQuickSwitchProfile: string;
  homeOpenDataFolderToast: string;
  homePackageActionFoundTitle: (title: string) => string;
  homePackageActionFoundBody: string;
  homePackageActionFoundSurfaceBody: string;
  homeActionResume: string;
  homeActionOpen: string;
  homeActionRestart: string;
  homeActionPlay: string;
  homeActionStop: string;
  homeActionConfigure: string;
  homeActionViewLogs: string;
  homeActionUninstall: string;
  homeMore: string;
  homeInstallationPopupBlockedTitle: string;
  homeInstallationPopupBlockedBody: string;

  homeStoppedToast: (title: string) => string;
  homeStopFailedTitle: string;
  homeStopFailedBody: string;
  homeUninstallTitle: (title: string) => string;
  homeUninstallingBody: string;
  homeUninstalledTitle: (title: string) => string;
  homeUninstalledBody: string;
  homeUninstallFailedTitle: string;
  homeUninstallFailedBody: string;
  homeLoadingDiagnostics: string;
  homeLoadingDiagnosticsSummary: string;
  homeDescriptorNoPackages: string;
  homeNoPackageStatus: string;
  homeDiagnosticsUnavailable: string;
  homeNow: string;
  homeTimeMinutesAgo: (count: number) => string;
  homeTimeHoursAgo: (count: number) => string;
  homeTimeDaysAgo: (count: number) => string;
  homeTimeWeeksAgo: (count: number) => string;
  homeTimeMonthsAgo: (count: number) => string;
  homeTimeYearsAgo: (count: number) => string;
  homeNoDiagnosticAvailable: string;
  homeDiagnosticsUnavailableCause: string;
  homePackageFailureTitle: (packageId: string, state: string) => string;
  homePackageDegradedSummary: string;
  homeActionsAria: (title: string) => string;

  homeContinueTitle: string;
  homeContinueRunning: string;
  homeContinueStopped: string;
  homeContinueFailed: string;
  homeContinueOpenAction: string;
  homeContinueResumeAction: string;
  homeContinueDiagnoseAction: string;
  homeContinueAgeNow: string;
  homeContinueEmptyTitle: string;
  homeContinueEmptyBody: string;
  homeContinueEmptyInstall: string;
  homeContinueEmptyTryYdltavern: string;
  homeContinuePickInstalled: string;

  close: string;
  back: string;
  continue: string;

  uiModalClose: string;
  uiToastDismiss: string;

  installModalContentLabel: string;
  installUrlEyebrow: string;
  installUrlTitle: string;
  installUrlDescription: string;
  installSourceLabel: string;
  installSourceHelper: string;
  installShortcuts: string;
  installResolveErrorTitle: string;
  installKeyboardHint: string;
  installResolving: string;
  installPlanFailedTitle: string;
  installCompleteTitle: string;
  installCompleteBody: (count: number, installationId?: string) => string;
  installFailedTitle: string;
  installListMore: (count: number) => string;
  installKindNative: string;
  installKindDeclaredExternal: string;
  installKindExternal: string;
  installKindDetected: string;
  installNoConformanceDetails: string;
  installConformanceSummary: (checks: number, failures: number, warnings: number) => string;
  installPlanEyebrow: string;
  installPlanTitle: string;
  installPlanDescription: string;
  installResolved: string;
  installRootPrefix: string;
  installExternalCliOnlyTitle: string;
  installExternalCliOnlyBody: string;
  installInstallationSection: string;
  installKindLabel: string;
  installRootPackageLabel: string;
  installVersionLabel: string;
  installSourceMetaLabel: string;
  installCommitLabel: string;
  installSignedLabel: string;
  installAllSigned: string;
  installUnsignedPackages: string;
  installPackagesSection: string;
  installPackagesWillInstall: (count: number) => string;
  installPermissionsRequested: string;
  installTotalEntries: (count: number) => string;
  installPermissionCapabilities: string;
  installPermissionNetwork: string;
  installPermissionSecrets: string;
  installNoNewCapabilityInvokes: string;
  installNoNewNetworkHosts: string;
  installNoNewSecretRefs: string;
  installSignaturesTitle: string;
  installIntegrityTitle: string;
  installConformanceTitle: string;
  installUnsignedPrefix: string;
  installNone: string;
  installNoLockfileDrift: string;
  installDriftItems: (count: number) => string;
  installApprovePermissions: string;
  installInstalling: string;
  installInstallButton: string;
  installProgressEyebrow: string;
  installProgressTitleFailed: string;
  installProgressTitleComplete: string;
  installProgressTitleInstalling: string;
  installPhaseResolvedPlan: string;
  installPhasePackageCount: (count: number) => string;
  installPhaseComplete: string;
  installPhaseDetectedKind: string;
  installPhasePermissionsApproved: string;
  installPhaseExecutingPlan: string;
  installPhaseInProgress: string;
  installPhaseInstallCompleted: string;
  installPhaseInstalledCount: (count: number) => string;
  installPhaseWaiting: string;
  installStatusFailed: string;
  installStatusCompleted: string;
  installStatusExecuting: string;
  installSeeActivity: string;
  installActivity: string;
  installActivityResolvePlan: (target: string) => string;
  installActivityDetectKind: string;
  installActivityPermissionsApproved: string;
  installActivityExecutePlan: (status: string) => string;
  installActivityStatusFailed: string;
  installActivityStatusCompleted: string;
  installActivityStatusRunning: string;
  installActivityRegisteredInstallation: (installationId: string) => string;
  installActivityProfileUpdated: string;
  installExternalEyebrow: string;
  installExternalTitle: string;
  installExternalDescription: string;
  installExternalStatus: string;
  installExternalInfo: string;
  installExternalPackagesResolved: (count: number) => string;
  installExternalPlanUnavailable: string;
  installExternalChoiceWrapTitle: string;
  installExternalChoiceWrapDescription: string;
  installExternalChoiceWorkspaceTitle: string;
  installExternalChoiceWorkspaceDescription: string;
  installExternalChipCliOnlyGeneration: string;
  installExternalChipNoWebExecution: string;
  installExternalChipCliOnlyDescriptor: string;
  installExternalChipInstallBlocked: string;
  installExternalHelp: string;
  installRecommended: string;
  installContinueDisabled: string;

  failureInstallationFallback: string;
  failureContentLabel: (installationName: string) => string;
  failureEyebrow: (installationName: string) => string;
  failureTitle: string;
  failureDescription: string;
  failureLogCopied: string;
  failureDiagnosis: string;
  failureExitCode: string;
  failureCause: string;
  failureUptime: string;
  failureImpact: string;
  failureLastCheckpoint: string;
  failureSessions: string;
  failureSessionsPreserved: string;
  failureRedactedStderr: (count: number) => string;
  failureCopyLog: string;
  failureNoRedactedLog: string;
  failureNoDiagnosticLog: string;
  failureStopAndUninstall: string;
  failureRestartInstallation: string;

  installationFrameStartFailedTitle: string;
  installationFrameStartFailedBody: string;
  installationFrameMountFailedTitle: string;
  installationFrameMountFailedBody: string;
  installationFrameStopped: (title: string) => string;
  installationFrameStopFailedTitle: string;
  installationFrameStopFailedBody: string;
  installationFrameBackHome: string;
  installationFrameAuditLog: string;
  installationFrameAuditLogUnavailable: string;
  installationFrameStopInstallation: string;
  installationFrameStop: string;
  installationFrameMore: string;
  installationFrameMoreUnavailable: string;
  installationFrameState: (state: string) => string;
  installationFrameLoadingSurface: string;
  installationFrameStoppedTitle: string;
  installationFrameStoppedBody: string;
  installationFrameConsoleTitle: string;
  installationFrameConsoleBody: string;
  installationFrameOpenInstallationTab: string;
  installationFrameInstallationTabBlockedTitle: string;
  installationFrameInstallationTabBlockedBody: string;
  installationFrameRefresh: string;
  installationFrameRefreshing: string;
  installationFrameRefreshDiagnostics: string;
  installationFrameUpdateInstallation: string;
  installationFrameUpdating: string;
  installationFrameUpdateCompleteTitle: string;
  installationFrameUpdateCompleteBody: (count: number) => string;
  installationFrameUpdateCurrentTitle: string;
  installationFrameUpdateCurrentBody: string;
  installationFrameUpdateFailedTitle: string;
  installationFrameStopConfirm: string;
  installationFrameDiagnosticsLoading: string;
  installationFrameStatus: string;
  installationFramePackages: string;
  installationFrameUpdates: string;
  installationFrameActivity: string;
  installationFramePackageHealth: (healthy: number, total: number) => string;
  installationFrameRecentEvents: (count: number) => string;
  installationFrameUpdatesAvailable: (count: number) => string;
  installationFrameUpdatesCurrent: string;
  installationFrameUpdateUnavailable: string;
  installationFrameInstallationId: string;
  installationFrameInstallationType: string;
  installationFrameSession: string;
  installationFrameActiveSession: string;
  installationFrameStorage: string;
  installationFrameInterfaceSection: string;
  installationFrameInterfaceDescription: string;
  installationFrameEntrySurface: string;
  installationFrameBundleUrl: string;
  installationFrameBundleFingerprint: string;
  installationFrameBundleUnavailable: string;
  installationFrameLastResolved: string;
  installationFrameUpdatesSection: string;
  installationFrameUpdatesDescription: string;
  installationFrameNoUpdateRecords: string;
  installationFramePackagesSection: string;
  installationFramePackagesDescription: string;
  installationFrameNoPackages: string;
  installationFramePackageCounts: (capabilities: number, hooks: number) => string;
  installationFrameActivitySection: string;
  installationFrameActivityDescription: string;
  installationFrameNoEvents: string;
  installationFrameNoSession: string;
  installationFrameDiagnosticsWarnings: string;
  installationFrameDevelopmentSection: string;
  installationFrameDevelopmentDescription: string;
  installationFrameDevelopmentLoadFailed: string;
  installationFrameDevelopmentGoal: string;
  installationFrameDevelopmentGoalPlaceholder: string;
  installationFrameDevelopmentTarget: string;
  installationFrameDevelopmentOperation: string;
  installationFrameDevelopmentWrite: string;
  installationFrameDevelopmentDelete: string;
  installationFrameDevelopmentExecutable: string;
  installationFrameDevelopmentDockerBuild: string;
  installationFrameDevelopmentAllowNetwork: string;
  installationFrameDevelopmentDockerfile: string;
  installationFrameDevelopmentContent: string;
  installationFrameDevelopmentSafetyHint: string;
  installationFrameDevelopmentDrafting: string;
  installationFrameDevelopmentDraft: string;
  installationFrameDevelopmentHistory: string;
  installationFrameDevelopmentHistoryDescription: string;
  installationFrameDevelopmentRefresh: string;
  installationFrameDevelopmentEmpty: string;
  installationFrameDevelopmentDrafted: string;
  installationFrameDevelopmentDraftFailed: string;
  installationFrameDevelopmentApproveConfirm: string;
  installationFrameDevelopmentDecisionFailed: string;
  installationFrameDevelopmentExecuteConfirm: string;
  installationFrameDevelopmentExecutionStarted: string;
  installationFrameDevelopmentExecutionAlreadyActive: string;
  installationFrameDevelopmentExecutionFailed: string;
  installationFrameDevelopmentRecovered: string;
  installationFrameDevelopmentRecoveryFailed: string;
  installationFrameDevelopmentExportFailed: string;
  installationFrameDevelopmentLinkedHint: string;
  installationFrameDevelopmentReviewOperations: string;
  installationFrameDevelopmentReviewVerification: string;
  installationFrameDevelopmentReviewAuthority: string;
  installationFrameDevelopmentReviewEffects: string;
  installationFrameDevelopmentApprovalRecord: string;
  installationFrameDevelopmentRecoveryTarget: string;
  installationFrameDevelopmentVerificationStatic: string;
  installationFrameDevelopmentVerificationDocker: string;
  installationFrameDevelopmentStatusDrafted: string;
  installationFrameDevelopmentStatusApproved: string;
  installationFrameDevelopmentStatusRejected: string;
  installationFrameDevelopmentStatusStaging: string;
  installationFrameDevelopmentStatusVerifying: string;
  installationFrameDevelopmentStatusPromoting: string;
  installationFrameDevelopmentStatusVerified: string;
  installationFrameDevelopmentStatusCommitted: string;
  installationFrameDevelopmentStatusRecoveryRequired: string;
  installationFrameDevelopmentStatusFailed: string;
  installationFrameDevelopmentExport: string;
  installationFrameDevelopmentReject: string;
  installationFrameDevelopmentApprove: string;
  installationFrameDevelopmentExecute: string;
  installationFrameDevelopmentRecover: string;
  installationFrameStatusReady: string;
  installationFrameStatusNotReady: string;
  installationFrameShowLogs: string;
  installationFrameHideLogs: string;
  installationFrameLogsLoading: string;
  installationFrameNoLogs: string;
  installationFrameCopyAddress: string;
  installationFrameCopyUrl: string;
  installationFrameOpenUrl: string;
  installationFramePublicUrl: string;
  installationFrameIframeUrl: string;

  powerboxTitle: string;
  powerboxDescription: string;
  powerboxLoading: string;
  powerboxRetry: string;
  powerboxClose: string;
  powerboxConsumer: string;
  powerboxConsumerRun: string;
  powerboxImportPort: string;
  powerboxPhase: string;
  powerboxPreferenceHint: string;
  powerboxChooseProvider: string;
  powerboxProviderInstallation: string;
  powerboxProviderWork: string;
  powerboxProviderRun: string;
  powerboxProviderPort: string;
  powerboxOrigin: string;
  powerboxDeclaration: string;
  powerboxClaim: string;
  powerboxBoundaries: string;
  powerboxEvidence: string;
  powerboxTrust: string;
  powerboxProtocol: string;
  powerboxInterface: string;
  powerboxVersion: string;
  powerboxProfile: string;
  powerboxInteraction: string;
  powerboxTransport: string;
  powerboxAudience: string;
  powerboxScope: string;
  powerboxExpiry: string;
  powerboxDuration: string;
  powerboxDataRisk: string;
  powerboxEffectRisk: string;
  powerboxNotDeclared: string;
  powerboxUnbounded: string;
  powerboxExpired: string;
  powerboxRiskConfirm: string;
  powerboxDurationConfirm: string;
  powerboxSelect: string;
  powerboxSelecting: string;
  powerboxSelected: string;
  powerboxAuthorityDenied: string;
  powerboxNextStep: string;
  powerboxEmptyAbsent: string;
  powerboxEmptyForbidden: string;
  powerboxEmptyUnsupported: string;
  powerboxEmptyUnavailable: string;
  powerboxEmptyStale: string;
  powerboxBindingsTitle: string;
  powerboxBindingsEmpty: string;
  powerboxRevoke: string;
  powerboxRevoking: string;
  powerboxExposuresTitle: string;
  powerboxExposuresDescription: string;
  powerboxExportPort: string;
  powerboxAudienceKind: string;
  powerboxAudienceId: string;
  powerboxExpiresAt: string;
  powerboxExposureConfirm: string;
  powerboxCreateExposure: string;
  powerboxCreatingExposure: string;
  powerboxExposuresEmpty: string;
  powerboxTypedPortNotice: string;
  powerboxStartRunFirst: string;
  powerboxRuntimeContextNext: string;

  settingsTitle: string;
  settingsHelper: string;
  settingsApiConnections: string;
  settingsHostAccess: string;
  settingsInstalledPackages: string;
  settingsProfiles: string;
  settingsStorage: string;
  settingsAbout: string;

  accessEyebrow: string;
  accessTitle: string;
  accessDescription: string;
  accessIdentityUnknown: string;
  accessRootIdentity: string;
  accessDeviceIdentity: string;
  accessRefresh: string;
  accessLimitedTitle: string;
  accessLimitedBody: string;
  accessCreateTitle: string;
  accessCreateBody: string;
  accessDeviceName: string;
  accessDevicePlaceholder: string;
  accessGrantDays: string;
  accessGrantDaysHelper: string;
  accessPublicHostUrl: string;
  accessPublicHostUrlHelper: string;
  accessHttpsRequired: string;
  accessPermissions: string;
  accessInstallationResources: string;
  accessInstallationResourcesBody: string;
  accessAllInstallations: string;
  accessInstallationIdsPlaceholder: string;
  accessTargetResources: string;
  accessTargetResourcesBody: string;
  accessAllTargets: string;
  accessTargetIdsPlaceholder: string;
  accessInstallationResource: (id: string) => string;
  accessTargetResource: (id: string) => string;
  accessRunResources: string;
  accessRunResourcesBody: string;
  accessAllRuns: string;
  accessRunIdsPlaceholder: string;
  accessRunResource: (id: string) => string;
  accessPortResources: string;
  accessPortResourcesBody: string;
  accessAllPorts: string;
  accessPortIdsPlaceholder: string;
  accessPortResource: (id: string) => string;
  accessExposureResources: string;
  accessExposureResourcesBody: string;
  accessAllExposures: string;
  accessExposureIdsPlaceholder: string;
  accessExposureResource: (id: string) => string;
  accessBindingResources: string;
  accessBindingResourcesBody: string;
  accessAllBindings: string;
  accessBindingIdsPlaceholder: string;
  accessBindingResource: (id: string) => string;
  accessCreateValidation: string;
  accessCreating: string;
  accessCreateButton: string;
  accessOneTimeEyebrow: string;
  accessOneTimeTitle: string;
  accessOneTimeBody: string;
  accessCopyLink: string;
  accessCopied: string;
  accessShareLink: string;
  accessPairingShareText: string;
  accessDevicesTitle: string;
  accessDevicesBody: string;
  accessNoDevices: string;
  accessCurrentDevice: string;
  accessStatusActive: string;
  accessStatusRevoked: string;
  accessStatusExpired: string;
  accessExpires: (date: string) => string;
  accessRevoke: string;
  accessRevokeConfirm: (name: string) => string;
  accessPendingTitle: string;
  accessPendingBody: string;
  accessNoPending: string;
  accessTicketExpires: (time: string) => string;
  accessCancelTicket: string;
  accessScopeObserve: string;
  accessScopeObserveBody: string;
  accessScopeInstallationOperate: string;
  accessScopeInstallationOperateBody: string;
  accessScopeBindingManage: string;
  accessScopeBindingManageBody: string;
  accessScopeExposureManage: string;
  accessScopeExposureManageBody: string;
  accessScopeRealization: string;
  accessScopeRealizationBody: string;
  accessScopeDevelopPropose: string;
  accessScopeDevelopProposeBody: string;
  accessScopeDevelopApprove: string;
  accessScopeDevelopApproveBody: string;
  accessScopeDevelopExecute: string;
  accessScopeDevelopExecuteBody: string;
  accessScopeManage: string;
  accessScopeManageBody: string;

  apiEyebrowLoading: string;
  apiEyebrowCount: (count: number) => string;
  apiTitle: string;
  apiDescription: string;
  apiStoredSecrets: string;
  apiAddSecret: string;
  apiLoadErrorTitle: string;
  apiLoadErrorBody: string;
  apiEmptyTitle: string;
  apiEmptyBody: string;
  apiStoreStatus: string;
  apiEncryption: string;
  apiMasterKey: string;
  apiStorage: string;
  apiTotal: string;
  apiConfigured: string;
  apiNotCreated: string;
  apiSecretsCount: (count: number) => string;
  apiHowUsed: string;
  apiHowUsedBody: string;
  apiOpenAuditLog: string;
  apiBackup: string;
  apiExportFile: string;
  apiImportFile: string;
  apiExportToast: string;
  apiImportToast: string;
  apiRemoved: (name: string) => string;
  apiDeleteFailedTitle: string;
  apiDeleteFailedBody: string;
  apiCopiedSecretName: string;
  apiStored: (name: string) => string;
  apiSaveFailedTitle: string;
  apiSaveFailedBody: string;
  apiHideName: string;
  apiRevealName: string;
  apiToggleReveal: string;
  apiCopyName: string;
  apiCopy: string;
  apiMore: string;
  apiRotate: string;
  apiDelete: string;
  apiAddContentLabel: string;
  apiAddEyebrow: string;
  apiAddTitle: string;
  apiAddDescription: string;
  apiProvider: string;
  apiSecretName: string;
  apiSecretNameHelper: string;
  apiValue: string;
  apiValueHelper: string;
  apiScope: string;
  apiScopePlatform: string;
  apiScopeInstallation: string;
  cancel: string;
  apiSaveKey: string;

  packagesEyebrowLoading: string;
  packagesEyebrowCount: (count: number) => string;
  packagesTitle: string;
  packagesDescription: string;
  packagesFilterPlaceholder: string;
  packagesFilterAll: string;
  packagesFilterInstallations: string;
  packagesFilterPlurora: string;
  packagesFilterThirdParty: string;
  packagesRefreshing: string;
  packagesRefresh: string;
  packagesLoadErrorTitle: string;
  packagesLoadErrorBody: string;
  packagesEmptyTitle: string;
  packagesNoMatchTitle: string;
  packagesEmptyBody: string;
  packagesNoMatchBody: string;
  packagesTablePackage: string;
  packagesTableVersion: string;
  packagesTableKind: string;
  packagesTableCapabilities: string;
  packagesTableState: string;
  packagesCopyId: string;
  packagesViewPermissions: string;
  packagesViewLogs: string;
  packagesUninstall: string;
  packagesShowing: (visible: number, total: number) => string;
  packagesShowAll: string;
  packagesCopiedId: string;
  packagesLogsTitle: (packageId: string) => string;
  packagesNoLogsTitle: string;
  packagesNoLogsBody: string;
  packagesLogsLoadErrorTitle: string;
  packagesLogsLoadErrorBody: string;

  profilesEyebrowLoading: string;
  profilesEyebrowActive: (name: string) => string;
  profilesEyebrowNone: string;
  profilesTitle: string;
  profilesDescriptionPrefix: string;
  profilesDescriptionSuffix: string;
  profilesOnMachine: string;
  profilesNew: string;
  profilesCreateTitle: string;
  profilesCreateBody: string;
  profilesDiagnosticsErrorTitle: string;
  profilesDiagnosticsErrorBody: string;
  profilesEmptyTitle: string;
  profilesEmptyBody: string;
  profilesActive: string;
  profilesLoadedPackages: string;
  profilesLoadedPackagesHint: string;
  profilesNetworkAllowlist: string;
  profilesOutboundBlocked: string;
  profilesSwitch: string;
  profilesSwitchHint: string;
  profilesSwitchRequiresRestart: string;
  profilesSwitchBody: (id: string) => string;
  profilesSwitchViaCli: string;
  profilesDefaultDescription: (packages: number, hosts: number) => string;
  hostConnectionsEyebrowActive: (name: string) => string;
  hostConnectionsTitle: string;
  hostConnectionsDescription: string;
  hostConnectionsSaved: string;
  hostConnectionsCurrent: string;
  hostConnectionsNew: string;
  hostConnectionsNewTitle: string;
  hostConnectionsNewBody: string;
  hostConnectionsName: string;
  hostConnectionsEndpoint: string;
  hostConnectionsEndpointHint: string;
  hostConnectionsConnect: string;
  hostConnectionsRemove: string;
  hostConnectionsRemoveConfirm: (name: string) => string;
  hostConnectionsReturnCurrent: string;
  hostConnectionsCredentialHint: string;

  storageTitleEyebrow: string;
  storageTitle: string;
  storageDescription: string;
  storageAreas: string;
  storageAreaInstallationData: string;
  storageAreaInstallationDataDesc: string;
  storageAreaPackageStore: string;
  storageAreaPackageStoreDesc: string;
  storageAreaProfiles: string;
  storageAreaProfilesDesc: string;
  storageAreaSecrets: string;
  storageAreaSecretsDesc: string;
  storageAreaCache: string;
  storageAreaCacheDesc: string;
  storageEventStore: string;
  storageSqliteDesc: string;
  storagePostgresDesc: string;
  storageMemoryDesc: string;
  storageCustomDesc: string;
  storageBackendNeutrality: string;
  storageBackendBody: string;

  aboutEyebrow: string;
  aboutSubtitle: string;
  aboutVersion: string;
  aboutBuild: string;
  aboutReleased: string;
  aboutChannel: string;
  aboutWhat: string;
  aboutPara1: string;
  aboutPara2: string;
  aboutPara3: string;
  aboutCredits: string;
  aboutBuiltOn: string;
  aboutFonts: string;
  aboutIcons: string;
  aboutLicense: string;
  aboutLicenseBody: string;
  aboutReadLicense: string;
  aboutLinks: string;
  aboutSourceCode: string;
  aboutDocumentation: string;
  aboutReportIssue: string;
  aboutCommunity: string;
  aboutChangelog: string;
  aboutGratitude: string;
  aboutGratitudeBody: string;
}

export const labels = {
  en: {
    languageName: "English",
    languageShort: "EN",
    languageMenuLabel: "Language",
    languageAria: (label) => `Language: ${label}`,

    authEyebrow: "Authentication",
    authTitle: "Access token required",
    authBody: "The Plurora host requires an access token. Paste your token to continue.",
    authTokenLabel: "Access token",
    authHideToken: "Hide token",
    authShowToken: "Show token",
    authPlaceholder: "Paste your access token…",
    authCheckingButton: "Checking…",
    authSubmitButton: "Authenticate",
    authStoredLocally: "Your token is stored locally in this browser.",
    authCheckingAccess: "Checking access…",
    authInvalidToken: "Invalid access token. Please check your token and try again.",
    authConnectionFailed: (message) => `Connection failed: ${message}`,

    pairEyebrow: "Secure device pairing",
    pairTitle: "Connect this device",
    pairBody:
      "Review the access prepared by your Plurora Host, then create an expiring, revocable session for this device.",
    pairLoading: "Validating the one-time invitation…",
    pairMissingBody: "This pairing link does not contain a one-time credential.",
    pairInvalidTitle: "Pairing invitation unavailable",
    pairInvalidBody: "This invitation is invalid, expired, cancelled, or has already been used.",
    pairDevice: "Device",
    pairPermissions: "Granted abilities",
    pairResources: "Granted resources",
    pairGrantExpires: "Access expires",
    pairSecureRequiredTitle: "HTTPS is required",
    pairSecureRequiredBody:
      "Remote sessions use a Secure, HttpOnly cookie. Reopen this invitation through the Host's HTTPS address.",
    pairConfirm: "Pair this device",
    pairClaiming: "Pairing…",
    pairCompleteTitle: "Device paired",
    pairCompleteBody:
      "The one-time invitation has been consumed. This browser now has a scoped session that the Host owner can revoke at any time.",
    pairOpenHost: "Open Plurora",

    topbarHome: "Home",
    topbarSettings: "Settings",
    topbarInstallation: (installationId) => `Installations / ${installationId}`,
    topbarNotifications: "Notifications",
    topbarThemeSystem: (theme) => `System (${theme === "dark" ? "Dark" : "Light"})`,
    topbarThemeLight: "Light mode",
    topbarThemeDark: "Dark mode",
    topbarThemeAria: (preference) => `Theme preference: ${preference}`,
    topbarLogout: "Log out",

    homeGreeting: "Welcome back",
    homeEmptyWorkshop: "Your workshop is empty. Install a installation to begin.",
    homeReading: "Reading your workshop…",
    homeShelfSummary: (all, running, stopped, failed) =>
      `${all} installations on the shelf. ${running} running, ${stopped} idle. ${failed > 0 ? `${failed} need attention.` : "No pending updates."}`,
    homeInstalledEyebrow: (count) => `Installations — ${count.toString().padStart(2, "0")} installed`,
    homeEmptyTitle: "No installations installed yet",
    homeEmptyBody:
      "Plurora is your workshop. Install a installation to begin — installations can be a Plurora-native source like YdlTavern, or any external git/local repo.",
    homeInstallLabel: "Install a installation",
    homeInstallHint: "Paste a GitHub URL or local path",
    homeErrorTitle: "Couldn't reach the host",
    homeErrorBody: "Installation inventory is unavailable. Try again from the local UI.",
    retry: "Retry",
    homeSearchPlaceholder: "Search installations, packages...",
    homeSortPrefix: "Sort",
    homeSortRecent: "Recent",
    homeFilterAll: "All",
    homeFilterRunning: "Running",
    homeFilterStopped: "Stopped",
    homeFilterFailed: "Failed",
    homeActivityLast24h: "Activity — last 24h",
    homeActivityNo24h: "No activity in the last 24 hours.",
    homeViewFullAuditLog: "View full audit log →",
    homeWorkshop: "Workshop",
    homeUpdates: "Updates",
    homeUpdatesAvailable: (count) => `${count} available`,
    homeEverythingUpToDate: "Everything is up to date.",
    homeUpdate: "Update",
    homeUpdateAll: "Update all →",
    homeDiskUsage: "Disk usage",
    homeDiskUsed: (value) => `${value} used`,
    homeUnknown: "Unknown",
    homeMeasuring: "Measuring",
    homeNoStorageMeasured: "No installation storage measured.",
    homeManageStorage: "Manage storage →",
    homeWorkshopCards: "Workshop cards",
    homeWorkshopCategoryTool: "Tool",
    homeWorkshopCategoryTemplate: "Template",
    homeWorkshopCategoryExample: "Example",
    homeQuickActions: "Quick actions",
    homeCapabilityCards: "Home capability cards",
    homeQuickInstallUrl: "Install URL",
    homeQuickDataFolder: "Data folder",
    homeQuickSettings: "Settings",
    homeQuickSwitchProfile: "Switch profile",
    homeOpenDataFolderToast: "Use the CLI to open the local platform data directory.",
    homePackageActionFoundTitle: (title) => `${title} found`,
    homePackageActionFoundBody: "This package action is available. Action wiring needs package details in a later pass.",
    homePackageActionFoundSurfaceBody:
      "This package surface is available. Opening it safely needs package details in a later pass.",
    homeActionResume: "Resume",
    homeActionOpen: "Open",
    homeActionRestart: "Restart",
    homeActionPlay: "Play",
    homeActionStop: "Stop",
    homeActionConfigure: "Configure…",
    homeActionViewLogs: "View logs",
    homeActionUninstall: "Uninstall…",
    homeMore: "More",
    homeInstallationPopupBlockedTitle: "Installation tab was blocked",
    homeInstallationPopupBlockedBody: "Allow pop-ups for this site, then open the installation interface from the console.",

    homeStoppedToast: (title) => `Stopped ${title}`,
    homeStopFailedTitle: "Stop failed",
    homeStopFailedBody: "The installation could not be stopped. Check the local host and try again.",
    homeUninstallTitle: (title) => `Uninstall ${title}`,
    homeUninstallingBody: "Removing installed packages from the active profile and archiving installation data.",
    homeUninstalledTitle: (title) => `Uninstalled ${title}`,
    homeUninstalledBody: "Installation data was archived locally. Reinstall the installation to use it again.",
    homeUninstallFailedTitle: "Uninstall failed",
    homeUninstallFailedBody: "The installation could not be uninstalled from the web shell. Check host diagnostics and retry.",
    homeLoadingDiagnostics: "Loading diagnostics…",
    homeLoadingDiagnosticsSummary: "Reading bounded package failure details from the kernel.",
    homeDescriptorNoPackages: "Installation descriptor does not list packages.",
    homeNoPackageStatus: "No associated package status was available.",
    homeDiagnosticsUnavailable: "Diagnostics are unavailable. Try again from the local UI.",
    homeNow: "now",
    homeTimeMinutesAgo: (count) => `${count} minute${count === 1 ? "" : "s"} ago`,
    homeTimeHoursAgo: (count) => `${count} hour${count === 1 ? "" : "s"} ago`,
    homeTimeDaysAgo: (count) => `${count} day${count === 1 ? "" : "s"} ago`,
    homeTimeWeeksAgo: (count) => `${count} week${count === 1 ? "" : "s"} ago`,
    homeTimeMonthsAgo: (count) => `${count} month${count === 1 ? "" : "s"} ago`,
    homeTimeYearsAgo: (count) => `${count} year${count === 1 ? "" : "s"} ago`,
    homeNoDiagnosticAvailable: "No diagnostic available",
    homeDiagnosticsUnavailableCause: "unavailable",
    homePackageFailureTitle: (packageId, state) => `Package ${packageId} ${state}`,
    homePackageDegradedSummary: "Package status is degraded, but no failure summary was reported.",
    homeActionsAria: (title) => `${title} actions`,

    homeContinueTitle: "Continue",
    homeContinueRunning: "Running",
    homeContinueStopped: "Stopped",
    homeContinueFailed: "Failed",
    homeContinueOpenAction: "Open",
    homeContinueResumeAction: "Continue",
    homeContinueDiagnoseAction: "View diagnostics",
    homeContinueAgeNow: "just now",
    homeContinueEmptyTitle: "No installation opened yet",
    homeContinueEmptyBody: "Install a installation to continue from here.",
    homeContinueEmptyInstall: "Install installation",
    homeContinueEmptyTryYdltavern: "Try YdlTavern",
    homeContinuePickInstalled: "Pick one of your installed installations",

    close: "Close",
    back: "Back",
    continue: "Continue",

    uiModalClose: "Close",
    uiToastDismiss: "Dismiss",

    installModalContentLabel: "Install installation",
    installUrlEyebrow: "Install — Step 1 of 3",
    installUrlTitle: "Where is the installation?",
    installUrlDescription:
      "Plurora installs from public Git repositories or local folders. We'll review the installation before anything runs.",
    installSourceLabel: "Source URL or path",
    installSourceHelper:
      "Public HTTPS Git only in the web shell. Local folders use the CLI or a native file picker flow.",
    installShortcuts: "Shortcuts",
    installResolveErrorTitle: "Could not resolve install plan",
    installKeyboardHint: "Press ⌘V to paste · ↵ to continue · Esc to cancel",
    installResolving: "Resolving…",
    installPlanFailedTitle: "Install plan failed",
    installCompleteTitle: "Install complete",
    installCompleteBody: (count, installationId) =>
      `${count} package${count === 1 ? "" : "s"} installed${installationId ? ` · installation ${installationId}` : ""}`,
    installFailedTitle: "Install failed",
    installListMore: (count) => `+${count} more`,
    installKindNative: "Native installation",
    installKindDeclaredExternal: "Declared external",
    installKindExternal: "External",
    installKindDetected: "Detected",
    installNoConformanceDetails: "No conformance details returned",
    installConformanceSummary: (checks, failures, warnings) =>
      `${checks} check${checks === 1 ? "" : "s"}, ${failures} failure${failures === 1 ? "" : "s"}, ${warnings} warning${warnings === 1 ? "" : "s"}`,

    installPlanEyebrow: "Install — Step 2 of 3",
    installPlanTitle: "Review the install plan",
    installPlanDescription:
      "Install Lab resolved this plan. Approve requested permissions to begin installation.",
    installResolved: "RESOLVED",
    installRootPrefix: "root:",
    installExternalCliOnlyTitle: "External adapter generation is CLI-only in this build.",
    installExternalCliOnlyBody:
      "The package plan is real, but the web UI will not execute it without a installation descriptor.",
    installInstallationSection: "Installation",
    installKindLabel: "Kind",
    installRootPackageLabel: "Root package",
    installVersionLabel: "Version",
    installSourceMetaLabel: "Source",
    installCommitLabel: "Commit",
    installSignedLabel: "Signed",
    installAllSigned: "All signed",
    installUnsignedPackages: "Unsigned packages",
    installPackagesSection: "Packages",
    installPackagesWillInstall: (count) => `${count} package${count === 1 ? "" : "s"} will be installed`,
    installPermissionsRequested: "Permissions requested",
    installTotalEntries: (count) => `${count} total entries`,
    installPermissionCapabilities: "Capabilities",
    installPermissionNetwork: "Network",
    installPermissionSecrets: "Secrets",
    installNoNewCapabilityInvokes: "No new capability invokes",
    installNoNewNetworkHosts: "No new network hosts",
    installNoNewSecretRefs: "No new secret refs",
    installSignaturesTitle: "Signatures",
    installIntegrityTitle: "Integrity",
    installConformanceTitle: "Conformance",
    installUnsignedPrefix: "Unsigned:",
    installNone: "none",
    installNoLockfileDrift: "No lockfile drift detected",
    installDriftItems: (count) => `${count} drift item${count === 1 ? "" : "s"}`,
    installApprovePermissions: "Approve requested permissions",
    installInstalling: "Installing…",
    installInstallButton: "Install",

    installProgressEyebrow: "Install — Step 3 of 3",
    installProgressTitleFailed: "Install failed",
    installProgressTitleComplete: "Install complete",
    installProgressTitleInstalling: "Installing installation",
    installPhaseResolvedPlan: "Resolved install plan",
    installPhasePackageCount: (count) => `${count} package${count === 1 ? "" : "s"}`,
    installPhaseComplete: "complete",
    installPhaseDetectedKind: "Detected installation kind",
    installPhasePermissionsApproved: "Permissions approved",
    installPhaseExecutingPlan: "Executing install plan",
    installPhaseInProgress: "in progress",
    installPhaseInstallCompleted: "Install completed",
    installPhaseInstalledCount: (count) => `${count} installed`,
    installPhaseWaiting: "waiting",
    installStatusFailed: "Failed",
    installStatusCompleted: "Completed",
    installStatusExecuting: "Executing",
    installSeeActivity: "see activity",
    installActivity: "Activity",
    installActivityResolvePlan: (target) => `resolve_plan completed for ${target}`,
    installActivityDetectKind: "detect_kind completed",
    installActivityPermissionsApproved: "requested permissions approved",
    installActivityExecutePlan: (status) => `execute_plan ${status}`,
    installActivityStatusFailed: "failed",
    installActivityStatusCompleted: "completed",
    installActivityStatusRunning: "running",
    installActivityRegisteredInstallation: (installationId) => `registered installation ${installationId}`,
    installActivityProfileUpdated: "profile updated · lockfile refreshed",

    installExternalEyebrow: "Install — External installation",
    installExternalTitle: "External adapter generation is CLI-only",
    installExternalDescription:
      "This source does not declare a Plurora installation descriptor. The web UI will not execute the package install without one.",
    installExternalStatus: "EXTERNAL",
    installExternalInfo:
      "Use the CLI to generate a descriptor for wrap/workspace mode, then install the declared installation from web.",
    installExternalPackagesResolved: (count) => `${count} package${count === 1 ? "" : "s"} resolved`,
    installExternalPlanUnavailable: "Package plan not available",
    installExternalChoiceWrapTitle: "Wrap with adapter",
    installExternalChoiceWrapDescription:
      "Requires CLI descriptor generation in this build before web install can execute.",
    installExternalChoiceWorkspaceTitle: "Open as workspace",
    installExternalChoiceWorkspaceDescription:
      "Also requires a CLI-generated workspace descriptor before this web install path can continue.",
    installExternalChipCliOnlyGeneration: "CLI-only generation",
    installExternalChipNoWebExecution: "No web execution",
    installExternalChipCliOnlyDescriptor: "CLI-only descriptor",
    installExternalChipInstallBlocked: "Install blocked here",
    installExternalHelp: "Generate a installation descriptor with the CLI, then return here.",
    installRecommended: "RECOMMENDED",
    installContinueDisabled: "Continue disabled",

    failureInstallationFallback: "Installation",
    failureContentLabel: (installationName) => `${installationName} failure details`,
    failureEyebrow: (installationName) => `Failure — ${installationName.toUpperCase()}`,
    failureTitle: "Installation failed",
    failureDescription: "Installation state is preserved. See the log below for the failure.",
    failureLogCopied: "Log copied",
    failureDiagnosis: "Diagnosis",
    failureExitCode: "Exit code",
    failureCause: "Cause",
    failureUptime: "Uptime",
    failureImpact: "Impact",
    failureLastCheckpoint: "Last checkpoint",
    failureSessions: "Sessions",
    failureSessionsPreserved: "preserved",
    failureRedactedStderr: (count) => `Redacted stderr · last ${count} lines`,
    failureCopyLog: "Copy log",
    failureNoRedactedLog: "No redacted log",
    failureNoDiagnosticLog: "No diagnostic log tail is available for this package.",
    failureStopAndUninstall: "Stop and uninstall",
    failureRestartInstallation: "Restart installation",

    installationFrameStartFailedTitle: "Failed to start installation",
    installationFrameStartFailedBody: "The installation frame could not be started. Check the local host and try again.",
    installationFrameMountFailedTitle: "Installation surface failed to mount",
    installationFrameMountFailedBody: "The installation is running, but its browser surface could not be loaded. Check the local host and surface bundle.",
    installationFrameStopped: (title) => `Stopped ${title}`,
    installationFrameStopFailedTitle: "Stop failed",
    installationFrameStopFailedBody: "The installation could not be stopped. Check the local host and try again.",
    installationFrameBackHome: "Back to Home",
    installationFrameAuditLog: "Audit log",
    installationFrameAuditLogUnavailable: "Audit log is not wired yet",
    installationFrameStopInstallation: "Stop installation",
    installationFrameStop: "Stop",
    installationFrameMore: "More",
    installationFrameMoreUnavailable: "More installation actions are not wired yet",
    installationFrameState: (state) => state.toUpperCase(),
    installationFrameLoadingSurface: "Loading installation interface…",
    installationFrameStoppedTitle: "Installation stopped",
    installationFrameStoppedBody: "This tab can be closed. Reopen the installation from Home when you want to resume.",
    installationFrameConsoleTitle: "Installation console",
    installationFrameConsoleBody: "The installation is running from this Plurora tab. Its own interface opens in a separate installation tab so the console controls remain available here.",
    installationFrameOpenInstallationTab: "Open installation interface",
    installationFrameInstallationTabBlockedTitle: "Installation tab was blocked",
    installationFrameInstallationTabBlockedBody: "Allow pop-ups for this site, then open the installation interface from the console.",
    installationFrameRefresh: "Refresh",
    installationFrameRefreshing: "Refreshing…",
    installationFrameRefreshDiagnostics: "Refresh diagnostics",
    installationFrameUpdateInstallation: "Update installation",
    installationFrameUpdating: "Updating…",
    installationFrameUpdateCompleteTitle: "Installation updated",
    installationFrameUpdateCompleteBody: (count) => `Updated ${count} package${count === 1 ? "" : "s"}.`,
    installationFrameUpdateCurrentTitle: "Installation is current",
    installationFrameUpdateCurrentBody: "No package updates are available.",
    installationFrameUpdateFailedTitle: "Update failed",
    installationFrameStopConfirm: "Stopping will terminate the running session and installation packages. Open work in the installation UI may be lost. Stop this installation?",
    installationFrameDiagnosticsLoading: "Loading diagnostics…",
    installationFrameStatus: "Status",
    installationFramePackages: "Packages",
    installationFrameUpdates: "Updates",
    installationFrameActivity: "Activity",
    installationFramePackageHealth: (healthy, total) => `${healthy}/${total} healthy`,
    installationFrameRecentEvents: (count) => `${count} recent event${count === 1 ? "" : "s"}`,
    installationFrameUpdatesAvailable: (count) => `${count} update${count === 1 ? "" : "s"} available`,
    installationFrameUpdatesCurrent: "Up to date",
    installationFrameUpdateUnavailable: "Update check unavailable",
    installationFrameInstallationId: "Installation ID",
    installationFrameInstallationType: "Type",
    installationFrameSession: "Session",
    installationFrameActiveSession: "Active",
    installationFrameStorage: "Storage",
    installationFrameInterfaceSection: "Installation interface",
    installationFrameInterfaceDescription: "Standalone installation tab plus the resolved surface bundle used by the iframe host.",
    installationFrameEntrySurface: "Entry surface",
    installationFrameBundleUrl: "Bundle URL",
    installationFrameBundleFingerprint: "Fingerprint",
    installationFrameBundleUnavailable: "Bundle could not be resolved",
    installationFrameLastResolved: "Last resolved",
    installationFrameUpdatesSection: "Updates",
    installationFrameUpdatesDescription: "Checks use plurora/install-lab capabilities through capability.invoke.",
    installationFrameNoUpdateRecords: "No update records returned for this installation.",
    installationFramePackagesSection: "Package health",
    installationFramePackagesDescription: "Runtime package state, counts, and redacted failure/log summaries when available.",
    installationFrameNoPackages: "No installation packages were reported by the host.",
    installationFramePackageCounts: (capabilities, hooks) => `${capabilities} capabilities · ${hooks} hooks`,
    installationFrameActivitySection: "Recent activity",
    installationFrameActivityDescription: "Best-effort tail of the running installation session events.",
    installationFrameNoEvents: "No recent events for this session.",
    installationFrameNoSession: "No running session is available yet.",
    installationFrameDiagnosticsWarnings: "Diagnostic warnings",
    installationFrameDevelopmentSection: "Development",
    installationFrameDevelopmentDescription: "Draft content-addressed ChangeSets, approve them explicitly, verify in an isolated scratch workspace, then promote only Host-owned trees.",
    installationFrameDevelopmentLoadFailed: "Development history unavailable",
    installationFrameDevelopmentGoal: "Goal",
    installationFrameDevelopmentGoalPlaceholder: "Describe the intended source change",
    installationFrameDevelopmentTarget: "Relative file path",
    installationFrameDevelopmentOperation: "Operation",
    installationFrameDevelopmentWrite: "Write file",
    installationFrameDevelopmentDelete: "Delete file",
    installationFrameDevelopmentExecutable: "Executable file",
    installationFrameDevelopmentDockerBuild: "Verify with Docker build",
    installationFrameDevelopmentAllowNetwork: "Allow Docker build egress",
    installationFrameDevelopmentDockerfile: "Dockerfile path",
    installationFrameDevelopmentContent: "UTF-8 source content",
    installationFrameDevelopmentSafetyHint: "Drafts store source blobs by digest, never raw source in the journal. Docker scratch builds default to no network and discard verification images. Managed external trees may be promoted; native trees are verify-only, and linked-local installations must first be imported as managed.",
    installationFrameDevelopmentDrafting: "Drafting…",
    installationFrameDevelopmentDraft: "Draft ChangeSet",
    installationFrameDevelopmentHistory: "Change history",
    installationFrameDevelopmentHistoryDescription: "Durable intent, policy, approval, verification, commit, and effect receipts.",
    installationFrameDevelopmentRefresh: "Refresh",
    installationFrameDevelopmentEmpty: "No development ChangeSets have been drafted for this installation.",
    installationFrameDevelopmentDrafted: "Development ChangeSet drafted",
    installationFrameDevelopmentDraftFailed: "Could not draft development ChangeSet",
    installationFrameDevelopmentApproveConfirm: "Approve this exact ChangeSet and its listed filesystem, Docker, and network authority?",
    installationFrameDevelopmentDecisionFailed: "Could not record approval decision",
    installationFrameDevelopmentExecuteConfirm: "Execute the approved ChangeSet in a Host-owned scratch workspace now?",
    installationFrameDevelopmentExecutionStarted: "Development execution started",
    installationFrameDevelopmentExecutionAlreadyActive: "Development execution is already active; status refreshed",
    installationFrameDevelopmentExecutionFailed: "Could not start development execution",
    installationFrameDevelopmentRecovered: "Development promotion reconciled",
    installationFrameDevelopmentRecoveryFailed: "Development promotion still requires recovery",
    installationFrameDevelopmentExportFailed: "Could not export patch bundle",
    installationFrameDevelopmentLinkedHint: "Linked-local installations are proposal-only. Import a managed copy before Host verification; Plurora never writes the linked user source.",
    installationFrameDevelopmentReviewOperations: "Exact operations",
    installationFrameDevelopmentReviewVerification: "Verification",
    installationFrameDevelopmentReviewAuthority: "Required authority",
    installationFrameDevelopmentReviewEffects: "Expected effects",
    installationFrameDevelopmentApprovalRecord: "Recorded approval decision",
    installationFrameDevelopmentRecoveryTarget: "Recovery reconciliation target",
    installationFrameDevelopmentVerificationStatic: "Static validation (no code execution)",
    installationFrameDevelopmentVerificationDocker: "Docker build verification",
    installationFrameDevelopmentStatusDrafted: "Drafted",
    installationFrameDevelopmentStatusApproved: "Approved",
    installationFrameDevelopmentStatusRejected: "Rejected",
    installationFrameDevelopmentStatusStaging: "Staging",
    installationFrameDevelopmentStatusVerifying: "Verifying",
    installationFrameDevelopmentStatusPromoting: "Promoting",
    installationFrameDevelopmentStatusVerified: "Verified",
    installationFrameDevelopmentStatusCommitted: "Committed",
    installationFrameDevelopmentStatusRecoveryRequired: "Recovery required",
    installationFrameDevelopmentStatusFailed: "Failed",
    installationFrameDevelopmentExport: "Export bundle",
    installationFrameDevelopmentReject: "Reject",
    installationFrameDevelopmentApprove: "Approve",
    installationFrameDevelopmentExecute: "Execute",
    installationFrameDevelopmentRecover: "Reconcile",
    installationFrameStatusReady: "Ready",
    installationFrameStatusNotReady: "Not ready",
    installationFrameShowLogs: "Show logs",
    installationFrameHideLogs: "Hide logs",
    installationFrameLogsLoading: "Loading logs…",
    installationFrameNoLogs: "No bounded logs returned for this execution.",
    installationFrameCopyAddress: "Copy address",
    installationFrameCopyUrl: "Copy URL",
    installationFrameOpenUrl: "Open URL",
    installationFramePublicUrl: "Public URL",
    installationFrameIframeUrl: "Iframe URL",

    powerboxTitle: "Connect a provider",
    powerboxDescription: "The Host calculated visible, compatible Exposures for this exact Installation, import Port, and binding phase. Nothing is selected automatically.",
    powerboxLoading: "Recomputing provider candidates…",
    powerboxRetry: "Recompute candidates",
    powerboxClose: "Close chooser",
    powerboxConsumer: "Consumer Installation",
    powerboxConsumerRun: "Consumer Run",
    powerboxImportPort: "Root import Port",
    powerboxPhase: "Binding phase",
    powerboxPreferenceHint: "Saved preference — UI hint only",
    powerboxChooseProvider: "Choose this provider",
    powerboxProviderInstallation: "Provider Installation",
    powerboxProviderWork: "Provider Work",
    powerboxProviderRun: "Provider Run",
    powerboxProviderPort: "Provider Port",
    powerboxOrigin: "Origin / provenance",
    powerboxDeclaration: "Declaration",
    powerboxClaim: "Claim status",
    powerboxBoundaries: "Enforced boundaries",
    powerboxEvidence: "Verified evidence",
    powerboxTrust: "Runtime trust",
    powerboxProtocol: "Protocol",
    powerboxInterface: "Interface",
    powerboxVersion: "Version",
    powerboxProfile: "Profile",
    powerboxInteraction: "Interaction",
    powerboxTransport: "Transport",
    powerboxAudience: "Audience",
    powerboxScope: "Resource scope",
    powerboxExpiry: "Lease expiry",
    powerboxDuration: "Remaining duration",
    powerboxDataRisk: "Data risk",
    powerboxEffectRisk: "Effect risk",
    powerboxNotDeclared: "Not declared",
    powerboxUnbounded: "No fixed expiry",
    powerboxExpired: "Expired",
    powerboxRiskConfirm: "I reviewed the provider, trust boundary, data risk, and effect risk.",
    powerboxDurationConfirm: "I reviewed the Exposure audience and lease expiry.",
    powerboxSelect: "Select and continue",
    powerboxSelecting: "Selecting…",
    powerboxSelected: "Binding selected. Rechecking Run status before continuing.",
    powerboxAuthorityDenied: "This Host identity lacks exact Powerbox authority.",
    powerboxNextStep: "Next step",
    powerboxEmptyAbsent: "No provider Exposure exists for this import Port.",
    powerboxEmptyForbidden: "Compatible providers may exist, but this identity cannot observe them.",
    powerboxEmptyUnsupported: "The Host does not support the required protocol, interaction, or transport.",
    powerboxEmptyUnavailable: "Compatible providers exist but are not currently available.",
    powerboxEmptyStale: "Candidate evidence is stale or expired. Recompute before choosing.",
    powerboxBindingsTitle: "Active bindings",
    powerboxBindingsEmpty: "No selected bindings are active for this Installation.",
    powerboxRevoke: "Revoke binding",
    powerboxRevoking: "Revoking…",
    powerboxExposuresTitle: "Verified export Exposures",
    powerboxExposuresDescription: "Expose only a verified export Port from the active Run, with an exact audience and lease.",
    powerboxExportPort: "Root export Port ID",
    powerboxAudienceKind: "Audience resource kind",
    powerboxAudienceId: "Audience resource ID",
    powerboxExpiresAt: "Expires at",
    powerboxExposureConfirm: "I confirm this exact export Port, audience, Run, and lease.",
    powerboxCreateExposure: "Create Exposure",
    powerboxCreatingExposure: "Creating…",
    powerboxExposuresEmpty: "No Exposures are recorded for this Installation.",
    powerboxTypedPortNotice: "The current public Installation view does not enumerate export Port details. Enter a verified root Port ID explicitly; the Host remains authoritative and rejects invalid input.",
    powerboxStartRunFirst: "Start a Run before creating a Run-scoped Exposure.",
    powerboxRuntimeContextNext: "Refresh the active Run and enter an exact Runtime import Port.",

    settingsTitle: "Settings",
    settingsHelper: "Settings live on this machine. No SaaS sync.",
    settingsApiConnections: "API Connections",
    settingsHostAccess: "Host Access",
    settingsInstalledPackages: "Installed Packages",
    settingsProfiles: "Profiles",
    settingsStorage: "Storage",
    settingsAbout: "About",

    accessEyebrow: "Host control plane",
    accessTitle: "Devices & access",
    accessDescription:
      "Pair phones and browsers without copying the Host root token. Every device receives an explicit scope, expiry, and revocable grant stored in the Host journal.",
    accessIdentityUnknown: "Unknown Host identity",
    accessRootIdentity: "Host root authority",
    accessDeviceIdentity: "Paired device session",
    accessRefresh: "Refresh",
    accessLimitedTitle: "This device has limited access",
    accessLimitedBody:
      "Its grant can use the Host but cannot create, inspect, or revoke other device grants.",
    accessCreateTitle: "Pair another device",
    accessCreateBody:
      "Create a short-lived invitation. The invitation is consumed once and the resulting device session never exposes the Host root credential.",
    accessDeviceName: "Device name",
    accessDevicePlaceholder: "e.g. Lin's phone",
    accessGrantDays: "Grant lifetime (days)",
    accessGrantDaysHelper: "Between 1 and 365 days.",
    accessPublicHostUrl: "HTTPS Host address",
    accessPublicHostUrlHelper: "The phone must be able to reach this exact Host address.",
    accessHttpsRequired: "Enter the public HTTPS origin for this Host, without a path.",
    accessPermissions: "Device abilities",
    accessInstallationResources: "Installation access",
    accessInstallationResourcesBody: "Grant every installation or enter exact installation ids separated by commas.",
    accessAllInstallations: "All installations",
    accessInstallationIdsPlaceholder: "installation-a, installation-b",
    accessTargetResources: "Execution targets",
    accessTargetResourcesBody: "Grant every target or enter exact target ids separated by commas.",
    accessAllTargets: "All targets",
    accessTargetIdsPlaceholder: "local, preview-server",
    accessInstallationResource: (id) => `Installation · ${id}`,
    accessTargetResource: (id) => `Target · ${id}`,
    accessRunResources: "Run selectors",
    accessRunResourcesBody: "Grant all Runs or enter exact Run IDs used by Runtime Bindings and Exposures.",
    accessAllRuns: "All Runs",
    accessRunIdsPlaceholder: "run-a, run-b",
    accessRunResource: (id) => `Run · ${id}`,
    accessPortResources: "Powerbox Ports",
    accessPortResourcesBody: "Grant all Port selectors or enter exact InstallationID/root-PortID selectors separated by commas.",
    accessAllPorts: "All Ports",
    accessPortIdsPlaceholder: "installation-id/save-import, installation-id/lobby-export",
    accessPortResource: (id) => `Port · ${id}`,
    accessExposureResources: "Exposure selectors",
    accessExposureResourcesBody: "Grant all Exposures or enter exact Exposure IDs separated by commas.",
    accessAllExposures: "All Exposures",
    accessExposureIdsPlaceholder: "exposure-a, exposure-b",
    accessExposureResource: (id) => `Exposure · ${id}`,
    accessBindingResources: "Binding selectors",
    accessBindingResourcesBody: "Grant all Bindings or enter exact Binding IDs separated by commas.",
    accessAllBindings: "All Bindings",
    accessBindingIdsPlaceholder: "binding-a, binding-b",
    accessBindingResource: (id) => `Binding · ${id}`,
    accessCreateValidation: "Enter a device name, 1–365 days, and a valid HTTPS Host address.",
    accessCreating: "Creating…",
    accessCreateButton: "Create pairing link",
    accessOneTimeEyebrow: "One-time invitation",
    accessOneTimeTitle: "Send this link to the device",
    accessOneTimeBody:
      "It expires in 10 minutes and disappears permanently after the first successful claim. Treat it as a short-lived secret.",
    accessCopyLink: "Copy link",
    accessCopied: "Copied",
    accessShareLink: "Share…",
    accessPairingShareText: "Pair this device with my Plurora Host.",
    accessDevicesTitle: "Device grants",
    accessDevicesBody: "Durable, scoped sessions known to this Host.",
    accessNoDevices: "No paired device grants yet.",
    accessCurrentDevice: "This device",
    accessStatusActive: "Active",
    accessStatusRevoked: "Revoked",
    accessStatusExpired: "Expired",
    accessExpires: (date) => `expires ${date}`,
    accessRevoke: "Revoke",
    accessRevokeConfirm: (name) => `Revoke Host access for ${name}?`,
    accessPendingTitle: "Pending invitations",
    accessPendingBody: "Unclaimed pairing links that have not expired.",
    accessNoPending: "No pending invitations.",
    accessTicketExpires: (time) => `invitation expires ${time}`,
    accessCancelTicket: "Cancel",
    accessScopeObserve: "Observe",
    accessScopeObserveBody: "Read Host, installation, deployment, and diagnostic state.",
    accessScopeInstallationOperate: "Operate installations",
    accessScopeInstallationOperateBody: "Start, stop, open, and fork installation sessions.",
    accessScopeBindingManage: "Manage Bindings",
    accessScopeBindingManageBody: "Explicitly select or revoke providers for exact consumer Installation, Port, Exposure, and Binding selectors.",
    accessScopeExposureManage: "Manage Exposures",
    accessScopeExposureManageBody: "Create or revoke an exact provider Installation, Run, export Port, audience, and Exposure.",
    accessScopeRealization: "Realization",
    accessScopeRealizationBody: "Plan, apply, reconcile, roll back, and stop managed Realizations.",
    accessScopeDevelopPropose: "Propose changes",
    accessScopeDevelopProposeBody: "Draft and inspect controlled installation change sets.",
    accessScopeDevelopApprove: "Approve changes",
    accessScopeDevelopApproveBody: "Approve or reject reviewed change sets.",
    accessScopeDevelopExecute: "Execute changes",
    accessScopeDevelopExecuteBody: "Run, promote, and recover approved change sets.",
    accessScopeManage: "Manage access",
    accessScopeManageBody: "Create and revoke device grants. Reserve for trusted administrators.",

    apiEyebrowLoading: "API Connections · loading…",
    apiEyebrowCount: (count) => `API Connections · ${count} keys stored`,
    apiTitle: "Local secret store",
    apiDescription:
      "Keys stay on this machine, encrypted with your platform key. Plurora never transmits raw keys — installations request them through audited capability calls.",
    apiStoredSecrets: "Stored secrets",
    apiAddSecret: "Add secret",
    apiLoadErrorTitle: "Couldn't load secrets",
    apiLoadErrorBody: "Secret metadata is unavailable. Try again from the local UI.",
    apiEmptyTitle: "No secrets stored",
    apiEmptyBody: "Add your first key. Plurora encrypts it with your platform key.",
    apiStoreStatus: "Store status",
    apiEncryption: "Encryption",
    apiMasterKey: "Master key",
    apiStorage: "Storage",
    apiTotal: "Total",
    apiConfigured: "configured",
    apiNotCreated: "not created",
    apiSecretsCount: (count) => `${count} secrets`,
    apiHowUsed: "How they're used",
    apiHowUsedBody:
      "The host injects the raw value into outbound requests on the installation's behalf.",
    apiOpenAuditLog: "Open audit log →",
    apiBackup: "Backup",
    apiExportFile: "Export to file",
    apiImportFile: "Import from file",
    apiExportToast: "Use plurora secrets export on the CLI",
    apiImportToast: "Use plurora secrets import on the CLI",
    apiRemoved: (name) => `Removed ${name}`,
    apiDeleteFailedTitle: "Delete failed",
    apiDeleteFailedBody: "The secret could not be removed. Check the local host and try again.",
    apiCopiedSecretName: "Copied secret name",
    apiStored: (name) => `Stored ${name}`,
    apiSaveFailedTitle: "Save failed",
    apiSaveFailedBody: "The secret could not be stored. Check the local host and try again.",
    apiHideName: "Hide name",
    apiRevealName: "Reveal name",
    apiToggleReveal: "Toggle reveal",
    apiCopyName: "Copy name",
    apiCopy: "Copy",
    apiMore: "More",
    apiRotate: "Rotate",
    apiDelete: "Delete…",
    apiAddContentLabel: "Add secret",
    apiAddEyebrow: "API Connections · Add",
    apiAddTitle: "Store a new key",
    apiAddDescription:
      "Plurora encrypts the value with your platform key and never sends raw keys to any installation.",
    apiProvider: "Provider",
    apiSecretName: "Secret name",
    apiSecretNameHelper: "Convention: PROVIDER_API_KEY (uppercase, underscores)",
    apiValue: "Value",
    apiValueHelper: "The raw key never leaves this machine.",
    apiScope: "Scope",
    apiScopePlatform: "Platform-wide",
    apiScopeInstallation: "Installation-only (configure on Home)",
    cancel: "Cancel",
    apiSaveKey: "Save key",

    packagesEyebrowLoading: "Installed packages · loading…",
    packagesEyebrowCount: (count) => `Installed packages · ${count} packages`,
    packagesTitle: "Workshop inventory",
    packagesDescription:
      "Installations, Plurora packages, and dependencies installed in this workshop. Refresh checks upstream sources.",
    packagesFilterPlaceholder: "Filter packages…",
    packagesFilterAll: "All",
    packagesFilterInstallations: "Installations",
    packagesFilterPlurora: "Plurora",
    packagesFilterThirdParty: "Third-party",
    packagesRefreshing: "Refreshing inventory…",
    packagesRefresh: "Refresh",
    packagesLoadErrorTitle: "Couldn't load packages",
    packagesLoadErrorBody: "Package inventory is unavailable. Try again from the local UI.",
    packagesEmptyTitle: "No packages installed yet",
    packagesNoMatchTitle: "No packages match this filter",
    packagesEmptyBody: "Install a installation from Home or run plurora install on the CLI.",
    packagesNoMatchBody: "Try a different filter or clear the search.",
    packagesTablePackage: "Package",
    packagesTableVersion: "Version",
    packagesTableKind: "Kind",
    packagesTableCapabilities: "Capabilities",
    packagesTableState: "State",
    packagesCopyId: "Copy package id",
    packagesViewPermissions: "View permissions",
    packagesViewLogs: "View logs",
    packagesUninstall: "Uninstall…",
    packagesShowing: (visible, total) => `Showing ${visible} of ${total}`,
    packagesShowAll: "Show all →",
    packagesCopiedId: "Package id copied",
    packagesLogsTitle: (packageId) => `Redacted logs for ${packageId}`,
    packagesNoLogsTitle: "No logs available",
    packagesNoLogsBody: "The kernel did not return a bounded redacted log tail for this package.",
    packagesLogsLoadErrorTitle: "Couldn't load logs",
    packagesLogsLoadErrorBody: "Diagnostics are unavailable. Try again or inspect the local CLI logs.",

    profilesEyebrowLoading: "Profiles · loading…",
    profilesEyebrowActive: (name) => `Profiles · Active: ${name}`,
    profilesEyebrowNone: "Profiles · No active profile",
    profilesTitle: "Workshop profiles",
    profilesDescriptionPrefix:
      "A profile bundles host configuration: which packages autoload, which outbound hosts are allowed, secret resolver settings. Profiles are YAML files passed to",
    profilesDescriptionSuffix: ".",
    profilesOnMachine: "Profiles on this machine",
    profilesNew: "New profile",
    profilesCreateTitle: "Create a profile",
    profilesCreateBody: "Create a YAML profile and start the host with --profile <path>.",
    profilesDiagnosticsErrorTitle: "Couldn't read host diagnostics",
    profilesDiagnosticsErrorBody: "Host diagnostics are unavailable. Try again from the local UI.",
    profilesEmptyTitle: "No profile in use",
    profilesEmptyBody: "Start the host with --profile <path> to enable profile-aware features.",
    profilesActive: "ACTIVE",
    profilesLoadedPackages: "Loaded packages",
    profilesLoadedPackagesHint: "Defined in the profile's packages list.",
    profilesNetworkAllowlist: "Network allowlist",
    profilesOutboundBlocked: "All outbound blocked.",
    profilesSwitch: "Switch profile…",
    profilesSwitchHint: "Switching restarts the host. Installation state is preserved.",
    profilesSwitchRequiresRestart: "Profile switch requires restart",
    profilesSwitchBody: (id) => `Use plurora host serve --profile profiles/${id}.yaml on the CLI to activate.`,
    profilesSwitchViaCli: "Switch profile via CLI",
    profilesDefaultDescription: (packages, hosts) =>
      `Active profile · ${packages} packages loaded · ${hosts} hosts allowed`,

    hostConnectionsEyebrowActive: (name) => `Host connection · ${name}`,
    hostConnectionsTitle: "Host connections",
    hostConnectionsDescription:
      "Use the managed Host that served this client, or connect the same Web, PWA, and Desktop UI to an explicit remote Host. Each connection keeps an independent access credential.",
    hostConnectionsSaved: "Saved Host connections",
    hostConnectionsCurrent: "Current managed Host",
    hostConnectionsNew: "Add Host",
    hostConnectionsNewTitle: "Connect another Host",
    hostConnectionsNewBody:
      "Remote Hosts require HTTPS. Plain HTTP is accepted only for loopback development endpoints.",
    hostConnectionsName: "Connection name",
    hostConnectionsEndpoint: "Host endpoint",
    hostConnectionsEndpointHint:
      "Use the Host origin only; credentials, paths, queries, and fragments are rejected.",
    hostConnectionsConnect: "Save and connect",
    hostConnectionsRemove: "Remove",
    hostConnectionsRemoveConfirm: (name) => `Remove ${name} and its locally stored credential?`,
    hostConnectionsReturnCurrent: "Return to managed Host",
    hostConnectionsCredentialHint:
      "The endpoint and display name are saved as a profile. Access tokens remain browser-local and are isolated per Host.",

    storageTitleEyebrow: "Storage",
    storageTitle: "Where your data lives",
    storageDescription:
      "Plurora keeps data on this machine by default. The UI summarizes storage areas without exposing host-specific absolute paths.",
    storageAreas: "Storage areas",
    storageAreaInstallationData: "Installation data",
    storageAreaInstallationDataDesc: "Installation metadata, checkpoints, package state, and run records.",
    storageAreaPackageStore: "Package store",
    storageAreaPackageStoreDesc: "Installed package sources and lockfile-managed revisions.",
    storageAreaProfiles: "Profiles",
    storageAreaProfilesDesc: "Host profiles passed to plurora host serve --profile.",
    storageAreaSecrets: "Secrets",
    storageAreaSecretsDesc: "Encrypted platform and installation secret stores. Raw values are never shown here.",
    storageAreaCache: "Cache",
    storageAreaCacheDesc: "Generated bundles, tokenizer caches, and other rebuildable data.",
    storageEventStore: "Event store",
    storageSqliteDesc: "Local file backend, default for single-host workshops.",
    storagePostgresDesc: "PostgreSQL backend, suitable for shared/team hosts.",
    storageMemoryDesc: "In-memory backend, no persistence between restarts.",
    storageCustomDesc: "Custom backend.",
    storageBackendNeutrality: "Backend neutrality",
    storageBackendBody:
      "Plurora's storage layer is backend-neutral. SQLite is the default for local single-host workshops. PostgreSQL is reserved for shared/team hosts. Multimodal retrieval providers (TDB, pgvector, others) are exposed as ordinary capability packages, never as kernel primitives.",

    aboutEyebrow: "About",
    aboutSubtitle: "Open platform for play and creation.",
    aboutVersion: "Version",
    aboutBuild: "Build",
    aboutReleased: "Released",
    aboutChannel: "Channel",
    aboutWhat: "What Plurora is",
    aboutPara1:
      "Plurora is a kernel and a contract. The kernel hosts your installations in sandboxes. The contract lets any installation — Plurora, community, or self-built — participate as a first-class citizen.",
    aboutPara2:
      "It runs on your machine, with your keys, your files, your network. There is no SaaS account, no central registry, no telemetry. Installations you install live in the local platform data directory and stay there until you remove them.",
    aboutPara3:
      "The shell you are looking at right now is one of many possible UIs. Anyone can write another. The platform is the contract — not this window.",
    aboutCredits: "Credits",
    aboutBuiltOn: "Built on",
    aboutFonts: "Fonts",
    aboutIcons: "Icons",
    aboutLicense: "License",
    aboutLicenseBody: "Free to use, modify, run. Network use requires source disclosure.",
    aboutReadLicense: "Read full license →",
    aboutLinks: "Links",
    aboutSourceCode: "Source code",
    aboutDocumentation: "Documentation",
    aboutReportIssue: "Report an issue",
    aboutCommunity: "Community",
    aboutChangelog: "Changelog",
    aboutGratitude: "Gratitude",
    aboutGratitudeBody:
      "SillyTavern community for the asset formats and extension API patterns referenced in YdlTavern compatibility work.",
  },
  "zh-CN": {
    languageName: "简体中文",
    languageShort: "中",
    languageMenuLabel: "语言",
    languageAria: (label) => `语言：${label}`,

    authEyebrow: "身份验证",
    authTitle: "需要访问令牌",
    authBody: "Plurora 主机需要访问令牌。粘贴令牌后继续。",
    authTokenLabel: "访问令牌",
    authHideToken: "隐藏令牌",
    authShowToken: "显示令牌",
    authPlaceholder: "粘贴访问令牌…",
    authCheckingButton: "正在检查…",
    authSubmitButton: "验证",
    authStoredLocally: "令牌仅保存在此浏览器本地。",
    authCheckingAccess: "正在检查访问权限…",
    authInvalidToken: "访问令牌无效。请检查令牌后重试。",
    authConnectionFailed: (message) => `连接失败：${message}`,

    pairEyebrow: "安全设备配对",
    pairTitle: "连接这台设备",
    pairBody: "确认 Plurora Host 为此设备准备的权限，然后创建一个可过期、可撤销的设备会话。",
    pairLoading: "正在验证一次性邀请…",
    pairMissingBody: "此配对链接中没有一次性凭据。",
    pairInvalidTitle: "配对邀请不可用",
    pairInvalidBody: "此邀请无效、已过期、已取消，或已经被使用。",
    pairDevice: "设备",
    pairPermissions: "授予能力",
    pairResources: "授权资源",
    pairGrantExpires: "访问到期时间",
    pairSecureRequiredTitle: "必须使用 HTTPS",
    pairSecureRequiredBody: "远程会话使用 Secure、HttpOnly Cookie。请通过 Host 的 HTTPS 地址重新打开此邀请。",
    pairConfirm: "配对此设备",
    pairClaiming: "正在配对…",
    pairCompleteTitle: "设备已配对",
    pairCompleteBody: "一次性邀请已经消耗。本浏览器现持有受限会话，Host 所有者可随时撤销。",
    pairOpenHost: "打开 Plurora",

    topbarHome: "首页",
    topbarSettings: "设置",
    topbarInstallation: (installationId) => `项目 / ${installationId}`,
    topbarNotifications: "通知",
    topbarThemeSystem: (theme) => `跟随系统（${theme === "dark" ? "深色" : "浅色"}）`,
    topbarThemeLight: "浅色模式",
    topbarThemeDark: "深色模式",
    topbarThemeAria: (preference) => `主题偏好：${preference}`,
    topbarLogout: "退出登录",

    homeGreeting: "欢迎回来",
    homeEmptyWorkshop: "工作台还是空的。安装一个项目开始吧。",
    homeReading: "正在读取工作台…",
    homeShelfSummary: (all, running, stopped, failed) =>
      `架上共有 ${all} 个项目。${running} 个运行中，${stopped} 个空闲。${failed > 0 ? `${failed} 个需要处理。` : "没有待处理更新。"}`,
    homeInstalledEyebrow: (count) => `项目 — 已安装 ${count.toString().padStart(2, "0")} 个`,
    homeEmptyTitle: "还没有安装项目",
    homeEmptyBody:
      "Plurora 是你的工作台。安装一个项目开始吧——项目可以是 Plurora 原生源（如 YdlTavern），也可以是任意外部 git/本地仓库。",
    homeInstallLabel: "安装项目",
    homeInstallHint: "粘贴 GitHub URL 或本地路径",
    homeErrorTitle: "无法连接主机",
    homeErrorBody: "项目清单暂不可用。请从本地 UI 重试。",
    retry: "重试",
    homeSearchPlaceholder: "搜索项目、包…",
    homeSortPrefix: "排序",
    homeSortRecent: "最近",
    homeFilterAll: "全部",
    homeFilterRunning: "运行中",
    homeFilterStopped: "已停止",
    homeFilterFailed: "失败",
    homeActivityLast24h: "活动 — 最近 24 小时",
    homeActivityNo24h: "最近 24 小时没有活动。",
    homeViewFullAuditLog: "查看完整审计日志 →",
    homeWorkshop: "工作台",
    homeUpdates: "更新",
    homeUpdatesAvailable: (count) => `${count} 个可用`,
    homeEverythingUpToDate: "所有内容都是最新的。",
    homeUpdate: "更新",
    homeUpdateAll: "全部更新 →",
    homeDiskUsage: "磁盘用量",
    homeDiskUsed: (value) => `已用 ${value}`,
    homeUnknown: "未知",
    homeMeasuring: "测量中",
    homeNoStorageMeasured: "尚未测量项目存储。",
    homeManageStorage: "管理存储 →",
    homeWorkshopCards: "工作台卡片",
    homeWorkshopCategoryTool: "工具",
    homeWorkshopCategoryTemplate: "模板",
    homeWorkshopCategoryExample: "示例",
    homeQuickActions: "快捷操作",
    homeCapabilityCards: "首页能力卡片",
    homeQuickInstallUrl: "安装 URL",
    homeQuickDataFolder: "数据文件夹",
    homeQuickSettings: "设置",
    homeQuickSwitchProfile: "切换配置",
    homeOpenDataFolderToast: "请使用 CLI 打开本地平台数据目录。",
    homePackageActionFoundTitle: (title) => `已发现 ${title}`,
    homePackageActionFoundBody: "此包操作已可见。操作接线需要在后续实现中读取包详情。",
    homePackageActionFoundSurfaceBody: "此包界面已可见。安全打开它需要在后续实现中读取包详情。",
    homeActionResume: "继续",
    homeActionOpen: "打开",
    homeActionRestart: "重启",
    homeActionPlay: "启动",
    homeActionStop: "停止",
    homeActionConfigure: "配置…",
    homeActionViewLogs: "查看日志",
    homeActionUninstall: "卸载…",
    homeMore: "更多",
    homeInstallationPopupBlockedTitle: "项目标签页被拦截",
    homeInstallationPopupBlockedBody: "请允许此站点打开弹出式窗口，然后从控制台重新打开项目界面。",

    homeStoppedToast: (title) => `已停止 ${title}`,
    homeStopFailedTitle: "停止失败",
    homeStopFailedBody: "无法停止该项目。请检查本地主机后重试。",
    homeUninstallTitle: (title) => `卸载 ${title}`,
    homeUninstallingBody: "正在从当前配置移除已安装包，并归档项目数据。",
    homeUninstalledTitle: (title) => `已卸载 ${title}`,
    homeUninstalledBody: "项目数据已在本机归档。重新安装后可再次使用。",
    homeUninstallFailedTitle: "卸载失败",
    homeUninstallFailedBody: "无法从 Web Shell 卸载该项目。请检查主机诊断后重试。",
    homeLoadingDiagnostics: "正在加载诊断…",
    homeLoadingDiagnosticsSummary: "正在从内核读取限定的包失败详情。",
    homeDescriptorNoPackages: "项目描述符没有列出包。",
    homeNoPackageStatus: "没有可用的关联包状态。",
    homeDiagnosticsUnavailable: "诊断暂不可用。请从本地 UI 重试。",
    homeNow: "现在",
    homeTimeMinutesAgo: (count) => `${count} 分钟前`,
    homeTimeHoursAgo: (count) => `${count} 小时前`,
    homeTimeDaysAgo: (count) => `${count} 天前`,
    homeTimeWeeksAgo: (count) => `${count} 周前`,
    homeTimeMonthsAgo: (count) => `${count} 个月前`,
    homeTimeYearsAgo: (count) => `${count} 年前`,
    homeNoDiagnosticAvailable: "没有可用诊断",
    homeDiagnosticsUnavailableCause: "不可用",
    homePackageFailureTitle: (packageId, state) => `包 ${packageId} ${state}`,
    homePackageDegradedSummary: "包状态已降级，但没有返回失败摘要。",
    homeActionsAria: (title) => `${title} 操作`,

    homeContinueTitle: "继续上次",
    homeContinueRunning: "运行中",
    homeContinueStopped: "已停止",
    homeContinueFailed: "启动失败",
    homeContinueOpenAction: "打开",
    homeContinueResumeAction: "继续",
    homeContinueDiagnoseAction: "查看诊断",
    homeContinueAgeNow: "刚刚",
    homeContinueEmptyTitle: "还没打开过项目",
    homeContinueEmptyBody: "安装一个项目就可以从这里继续。",
    homeContinueEmptyInstall: "安装项目",
    homeContinueEmptyTryYdltavern: "试试 YdlTavern",
    homeContinuePickInstalled: "从已安装的项目里选一个开始",

    close: "关闭",
    back: "返回",
    continue: "继续",

    uiModalClose: "关闭",
    uiToastDismiss: "关闭通知",

    installModalContentLabel: "安装项目",
    installUrlEyebrow: "安装 — 第 1 / 3 步",
    installUrlTitle: "项目在哪里？",
    installUrlDescription: "Plurora 可从公开 Git 仓库或本地文件夹安装。运行任何内容前，我们会先让你检查项目。",
    installSourceLabel: "源 URL 或路径",
    installSourceHelper: "Web shell 仅支持公开 HTTPS Git。本地文件夹请使用 CLI 或原生文件选择器流程。",
    installShortcuts: "快捷入口",
    installResolveErrorTitle: "无法解析安装计划",
    installKeyboardHint: "按 ⌘V 粘贴 · ↵ 继续 · Esc 取消",
    installResolving: "正在解析…",
    installPlanFailedTitle: "安装计划失败",
    installCompleteTitle: "安装完成",
    installCompleteBody: (count, installationId) => `已安装 ${count} 个包${installationId ? ` · 项目 ${installationId}` : ""}`,
    installFailedTitle: "安装失败",
    installListMore: (count) => `+${count} 项`,
    installKindNative: "原生项目",
    installKindDeclaredExternal: "已声明外部项目",
    installKindExternal: "外部项目",
    installKindDetected: "已检测",
    installNoConformanceDetails: "没有返回一致性详情",
    installConformanceSummary: (checks, failures, warnings) =>
      `${checks} 项检查，${failures} 项失败，${warnings} 项警告`,

    installPlanEyebrow: "安装 — 第 2 / 3 步",
    installPlanTitle: "检查安装计划",
    installPlanDescription: "Install Lab 已解析此计划。批准所请求的权限后即可开始安装。",
    installResolved: "已解析",
    installRootPrefix: "root:",
    installExternalCliOnlyTitle: "此构建中外部适配器生成仅支持 CLI。",
    installExternalCliOnlyBody: "包计划是真实的，但没有项目描述符时，Web UI 不会执行它。",
    installInstallationSection: "项目",
    installKindLabel: "类型",
    installRootPackageLabel: "根包",
    installVersionLabel: "版本",
    installSourceMetaLabel: "来源",
    installCommitLabel: "Commit",
    installSignedLabel: "签名",
    installAllSigned: "全部已签名",
    installUnsignedPackages: "存在未签名包",
    installPackagesSection: "包",
    installPackagesWillInstall: (count) => `将安装 ${count} 个包`,
    installPermissionsRequested: "请求的权限",
    installTotalEntries: (count) => `共 ${count} 项`,
    installPermissionCapabilities: "能力",
    installPermissionNetwork: "网络",
    installPermissionSecrets: "密钥",
    installNoNewCapabilityInvokes: "没有新增能力调用",
    installNoNewNetworkHosts: "没有新增网络主机",
    installNoNewSecretRefs: "没有新增 secret_ref",
    installSignaturesTitle: "签名",
    installIntegrityTitle: "完整性",
    installConformanceTitle: "一致性",
    installUnsignedPrefix: "未签名：",
    installNone: "无",
    installNoLockfileDrift: "未检测到 lockfile 漂移",
    installDriftItems: (count) => `${count} 个漂移项`,
    installApprovePermissions: "批准请求的权限",
    installInstalling: "正在安装…",
    installInstallButton: "安装",

    installProgressEyebrow: "安装 — 第 3 / 3 步",
    installProgressTitleFailed: "安装失败",
    installProgressTitleComplete: "安装完成",
    installProgressTitleInstalling: "正在安装项目",
    installPhaseResolvedPlan: "已解析安装计划",
    installPhasePackageCount: (count) => `${count} 个包`,
    installPhaseComplete: "完成",
    installPhaseDetectedKind: "已检测项目类型",
    installPhasePermissionsApproved: "权限已批准",
    installPhaseExecutingPlan: "正在执行安装计划",
    installPhaseInProgress: "进行中",
    installPhaseInstallCompleted: "安装已完成",
    installPhaseInstalledCount: (count) => `已安装 ${count} 个`,
    installPhaseWaiting: "等待中",
    installStatusFailed: "失败",
    installStatusCompleted: "已完成",
    installStatusExecuting: "执行中",
    installSeeActivity: "见活动",
    installActivity: "活动",
    installActivityResolvePlan: (target) => `resolve_plan 已完成：${target}`,
    installActivityDetectKind: "detect_kind 已完成",
    installActivityPermissionsApproved: "请求的权限已批准",
    installActivityExecutePlan: (status) => `execute_plan ${status}`,
    installActivityStatusFailed: "失败",
    installActivityStatusCompleted: "完成",
    installActivityStatusRunning: "运行中",
    installActivityRegisteredInstallation: (installationId) => `已注册项目 ${installationId}`,
    installActivityProfileUpdated: "profile 已更新 · lockfile 已刷新",

    installExternalEyebrow: "安装 — 外部项目",
    installExternalTitle: "外部适配器生成仅支持 CLI",
    installExternalDescription: "此来源没有声明 Plurora 项目描述符。没有描述符时，Web UI 不会执行包安装。",
    installExternalStatus: "外部",
    installExternalInfo: "请用 CLI 为 wrap/workspace 模式生成描述符，然后从 Web 安装已声明的项目。",
    installExternalPackagesResolved: (count) => `已解析 ${count} 个包`,
    installExternalPlanUnavailable: "包计划不可用",
    installExternalChoiceWrapTitle: "用适配器包装",
    installExternalChoiceWrapDescription: "此构建需要先通过 CLI 生成描述符，然后 Web 安装才能执行。",
    installExternalChoiceWorkspaceTitle: "作为 workspace 打开",
    installExternalChoiceWorkspaceDescription: "此 Web 安装路径继续前，也需要先由 CLI 生成 workspace 描述符。",
    installExternalChipCliOnlyGeneration: "仅 CLI 生成",
    installExternalChipNoWebExecution: "Web 不执行",
    installExternalChipCliOnlyDescriptor: "仅 CLI 描述符",
    installExternalChipInstallBlocked: "此处阻止安装",
    installExternalHelp: "使用 CLI 生成项目描述符后，再回到这里。",
    installRecommended: "推荐",
    installContinueDisabled: "继续已禁用",

    failureInstallationFallback: "项目",
    failureContentLabel: (installationName) => `${installationName} 失败详情`,
    failureEyebrow: (installationName) => `失败 — ${installationName.toUpperCase()}`,
    failureTitle: "项目失败",
    failureDescription: "项目状态已保留。请查看下面的日志了解失败原因。",
    failureLogCopied: "日志已复制",
    failureDiagnosis: "诊断",
    failureExitCode: "退出码",
    failureCause: "原因",
    failureUptime: "运行时长",
    failureImpact: "影响",
    failureLastCheckpoint: "上次检查点",
    failureSessions: "会话",
    failureSessionsPreserved: "已保留",
    failureRedactedStderr: (count) => `已脱敏 stderr · 最近 ${count} 行`,
    failureCopyLog: "复制日志",
    failureNoRedactedLog: "没有脱敏日志",
    failureNoDiagnosticLog: "此包没有可用的诊断日志尾部。",
    failureStopAndUninstall: "停止并卸载",
    failureRestartInstallation: "重启项目",

    installationFrameStartFailedTitle: "启动项目失败",
    installationFrameStartFailedBody: "无法启动项目框架。请检查本地主机后重试。",
    installationFrameMountFailedTitle: "项目界面挂载失败",
    installationFrameMountFailedBody: "项目已经运行，但浏览器界面未能加载。请检查本地主机和 surface bundle。",
    installationFrameStopped: (title) => `已停止 ${title}`,
    installationFrameStopFailedTitle: "停止失败",
    installationFrameStopFailedBody: "无法停止该项目。请检查本地主机后重试。",
    installationFrameBackHome: "返回首页",
    installationFrameAuditLog: "审计日志",
    installationFrameAuditLogUnavailable: "审计日志暂未接线",
    installationFrameStopInstallation: "停止项目",
    installationFrameStop: "停止",
    installationFrameMore: "更多",
    installationFrameMoreUnavailable: "更多项目操作暂未接线",
    installationFrameState: (state) => state.toUpperCase(),
    installationFrameLoadingSurface: "正在加载项目界面…",
    installationFrameStoppedTitle: "项目已停止",
    installationFrameStoppedBody: "可以关闭这个标签页。需要继续时，从首页重新打开项目。",
    installationFrameConsoleTitle: "项目控制台",
    installationFrameConsoleBody: "项目正在由当前 Plurora 标签页托管。项目自己的界面会在独立项目标签页打开，这里保留返回、停止和审计控制。",
    installationFrameOpenInstallationTab: "打开项目界面",
    installationFrameInstallationTabBlockedTitle: "项目标签页被浏览器拦截",
    installationFrameInstallationTabBlockedBody: "请允许本站弹出窗口，然后从控制台重新打开项目界面。",
    installationFrameRefresh: "刷新",
    installationFrameRefreshing: "正在刷新…",
    installationFrameRefreshDiagnostics: "刷新诊断",
    installationFrameUpdateInstallation: "更新项目",
    installationFrameUpdating: "正在更新…",
    installationFrameUpdateCompleteTitle: "项目已更新",
    installationFrameUpdateCompleteBody: (count) => `已更新 ${count} 个包。`,
    installationFrameUpdateCurrentTitle: "项目已是最新",
    installationFrameUpdateCurrentBody: "没有可用的包更新。",
    installationFrameUpdateFailedTitle: "更新失败",
    installationFrameStopConfirm: "停止会终止当前运行会话和项目包。项目界面里尚未保存的工作可能丢失。确定停止这个项目吗？",
    installationFrameDiagnosticsLoading: "正在加载诊断…",
    installationFrameStatus: "状态",
    installationFramePackages: "包",
    installationFrameUpdates: "更新",
    installationFrameActivity: "活动",
    installationFramePackageHealth: (healthy, total) => `${healthy}/${total} 健康`,
    installationFrameRecentEvents: (count) => `${count} 条近期事件`,
    installationFrameUpdatesAvailable: (count) => `${count} 个更新可用`,
    installationFrameUpdatesCurrent: "已是最新",
    installationFrameUpdateUnavailable: "更新检查不可用",
    installationFrameInstallationId: "项目 ID",
    installationFrameInstallationType: "类型",
    installationFrameSession: "会话",
    installationFrameActiveSession: "运行中",
    installationFrameStorage: "存储",
    installationFrameInterfaceSection: "项目界面",
    installationFrameInterfaceDescription: "独立项目标签页，以及 iframe 主机解析到的 surface bundle。",
    installationFrameEntrySurface: "入口 surface",
    installationFrameBundleUrl: "Bundle URL",
    installationFrameBundleFingerprint: "指纹",
    installationFrameBundleUnavailable: "无法解析 bundle",
    installationFrameLastResolved: "最近解析",
    installationFrameUpdatesSection: "更新",
    installationFrameUpdatesDescription: "通过 capability.invoke 调用 plurora/install-lab 能力进行检查。",
    installationFrameNoUpdateRecords: "该项目没有返回更新记录。",
    installationFramePackagesSection: "包健康",
    installationFramePackagesDescription: "运行时包状态、计数，以及可用的脱敏失败/日志摘要。",
    installationFrameNoPackages: "主机未报告项目包。",
    installationFramePackageCounts: (capabilities, hooks) => `${capabilities} 个能力 · ${hooks} 个 hook`,
    installationFrameActivitySection: "近期活动",
    installationFrameActivityDescription: "尽力展示运行中项目会话的事件尾部。",
    installationFrameNoEvents: "该会话暂无近期事件。",
    installationFrameNoSession: "尚无运行中的会话。",
    installationFrameDiagnosticsWarnings: "诊断警告",
    installationFrameDevelopmentSection: "开发",
    installationFrameDevelopmentDescription: "草拟内容寻址 ChangeSet，显式审批，在隔离 scratch 中验证，并且只提升 Host 拥有的工作树。",
    installationFrameDevelopmentLoadFailed: "无法读取开发历史",
    installationFrameDevelopmentGoal: "目标",
    installationFrameDevelopmentGoalPlaceholder: "描述这次源码变更要实现什么",
    installationFrameDevelopmentTarget: "相对文件路径",
    installationFrameDevelopmentOperation: "操作",
    installationFrameDevelopmentWrite: "写入文件",
    installationFrameDevelopmentDelete: "删除文件",
    installationFrameDevelopmentExecutable: "可执行文件",
    installationFrameDevelopmentDockerBuild: "使用 Docker 构建验证",
    installationFrameDevelopmentAllowNetwork: "允许 Docker 构建出网",
    installationFrameDevelopmentDockerfile: "Dockerfile 路径",
    installationFrameDevelopmentContent: "UTF-8 源码内容",
    installationFrameDevelopmentSafetyHint: "草稿把源码按摘要存为对象，journal 不保存源码明文；Docker scratch 默认断网并删除验证镜像。managed external 可提升，native 只验证；linked-local 必须先导入为 managed 才能由 Host 验证。",
    installationFrameDevelopmentDrafting: "正在草拟…",
    installationFrameDevelopmentDraft: "草拟 ChangeSet",
    installationFrameDevelopmentHistory: "变更历史",
    installationFrameDevelopmentHistoryDescription: "持久保存 intent、策略、审批、验证、commit 与 effect receipt。",
    installationFrameDevelopmentRefresh: "刷新",
    installationFrameDevelopmentEmpty: "该项目尚无开发 ChangeSet。",
    installationFrameDevelopmentDrafted: "开发 ChangeSet 已草拟",
    installationFrameDevelopmentDraftFailed: "无法草拟开发 ChangeSet",
    installationFrameDevelopmentApproveConfirm: "批准这个精确 ChangeSet 及其列出的文件系统、Docker 与网络权限？",
    installationFrameDevelopmentDecisionFailed: "无法记录审批决定",
    installationFrameDevelopmentExecuteConfirm: "现在在 Host 拥有的 scratch 工作区执行已批准的 ChangeSet？",
    installationFrameDevelopmentExecutionStarted: "开发执行已开始",
    installationFrameDevelopmentExecutionAlreadyActive: "开发执行已在进行，状态已刷新",
    installationFrameDevelopmentExecutionFailed: "无法启动开发执行",
    installationFrameDevelopmentRecovered: "开发 promotion 已完成对账",
    installationFrameDevelopmentRecoveryFailed: "开发 promotion 仍需要恢复",
    installationFrameDevelopmentExportFailed: "无法导出 patch bundle",
    installationFrameDevelopmentLinkedHint: "linked-local 项目仅允许提案。请先导入 managed 副本再进行 Host 验证；Plurora 永不写入链接的用户源目录。",
    installationFrameDevelopmentReviewOperations: "精确操作",
    installationFrameDevelopmentReviewVerification: "验证方式",
    installationFrameDevelopmentReviewAuthority: "所需权限",
    installationFrameDevelopmentReviewEffects: "预期效果",
    installationFrameDevelopmentApprovalRecord: "已记录的审批决定",
    installationFrameDevelopmentRecoveryTarget: "恢复对账对象",
    installationFrameDevelopmentVerificationStatic: "静态验证（不执行代码）",
    installationFrameDevelopmentVerificationDocker: "Docker 构建验证",
    installationFrameDevelopmentStatusDrafted: "草稿",
    installationFrameDevelopmentStatusApproved: "已批准",
    installationFrameDevelopmentStatusRejected: "已拒绝",
    installationFrameDevelopmentStatusStaging: "正在暂存",
    installationFrameDevelopmentStatusVerifying: "正在验证",
    installationFrameDevelopmentStatusPromoting: "正在提升",
    installationFrameDevelopmentStatusVerified: "已验证",
    installationFrameDevelopmentStatusCommitted: "已提交",
    installationFrameDevelopmentStatusRecoveryRequired: "需要恢复",
    installationFrameDevelopmentStatusFailed: "失败",
    installationFrameDevelopmentExport: "导出 bundle",
    installationFrameDevelopmentReject: "拒绝",
    installationFrameDevelopmentApprove: "批准",
    installationFrameDevelopmentExecute: "执行",
    installationFrameDevelopmentRecover: "对账恢复",
    installationFrameStatusReady: "已就绪",
    installationFrameStatusNotReady: "未就绪",
    installationFrameShowLogs: "显示日志",
    installationFrameHideLogs: "隐藏日志",
    installationFrameLogsLoading: "正在加载日志…",
    installationFrameNoLogs: "该执行没有返回限定日志。",
    installationFrameCopyAddress: "复制地址",
    installationFrameCopyUrl: "复制 URL",
    installationFrameOpenUrl: "打开 URL",
    installationFramePublicUrl: "公共 URL",
    installationFrameIframeUrl: "Iframe URL",

    powerboxTitle: "连接提供方",
    powerboxDescription: "Host 已针对这个准确的 Installation、根导入 Port 和绑定阶段计算当前可见且兼容的 Exposure；界面不会自动选择。",
    powerboxLoading: "正在重新计算提供方候选…",
    powerboxRetry: "重新计算候选",
    powerboxClose: "关闭选择器",
    powerboxConsumer: "消费方 Installation",
    powerboxConsumerRun: "消费方 Run",
    powerboxImportPort: "根导入 Port",
    powerboxPhase: "绑定阶段",
    powerboxPreferenceHint: "已保存偏好——仅作界面提示",
    powerboxChooseProvider: "选择此提供方",
    powerboxProviderInstallation: "提供方 Installation",
    powerboxProviderWork: "提供方 Work",
    powerboxProviderRun: "提供方 Run",
    powerboxProviderPort: "提供方 Port",
    powerboxOrigin: "来源 / provenance",
    powerboxDeclaration: "声明",
    powerboxClaim: "声明状态",
    powerboxBoundaries: "实际强制边界",
    powerboxEvidence: "已验证证据",
    powerboxTrust: "运行时信任边界",
    powerboxProtocol: "协议",
    powerboxInterface: "接口",
    powerboxVersion: "版本",
    powerboxProfile: "Profile",
    powerboxInteraction: "交互模型",
    powerboxTransport: "传输",
    powerboxAudience: "受众",
    powerboxScope: "资源作用域",
    powerboxExpiry: "租约到期",
    powerboxDuration: "剩余时长",
    powerboxDataRisk: "数据风险",
    powerboxEffectRisk: "效果风险",
    powerboxNotDeclared: "未声明",
    powerboxUnbounded: "没有固定到期时间",
    powerboxExpired: "已过期",
    powerboxRiskConfirm: "我已检查提供方、信任边界、数据风险和效果风险。",
    powerboxDurationConfirm: "我已检查 Exposure 的受众和租约到期时间。",
    powerboxSelect: "选择并继续",
    powerboxSelecting: "正在选择…",
    powerboxSelected: "Binding 已选择。继续前正在重新检查 Run 状态。",
    powerboxAuthorityDenied: "当前 Host 身份缺少准确的 Powerbox 权限。",
    powerboxNextStep: "下一步",
    powerboxEmptyAbsent: "这个导入 Port 尚无提供方 Exposure。",
    powerboxEmptyForbidden: "可能存在兼容提供方，但当前身份无权查看。",
    powerboxEmptyUnsupported: "Host 不支持所需协议、交互模型或传输。",
    powerboxEmptyUnavailable: "存在兼容提供方，但当前不可用。",
    powerboxEmptyStale: "候选证据已陈旧或过期；选择前必须重新计算。",
    powerboxBindingsTitle: "活动 Binding",
    powerboxBindingsEmpty: "这个 Installation 当前没有活动的已选 Binding。",
    powerboxRevoke: "撤销 Binding",
    powerboxRevoking: "正在撤销…",
    powerboxExposuresTitle: "已验证导出 Exposure",
    powerboxExposuresDescription: "只暴露活动 Run 中已验证的导出 Port，并明确指定受众和租约。",
    powerboxExportPort: "根导出 Port ID",
    powerboxAudienceKind: "受众资源类型",
    powerboxAudienceId: "受众资源 ID",
    powerboxExpiresAt: "到期时间",
    powerboxExposureConfirm: "我确认这个准确的导出 Port、受众、Run 和租约。",
    powerboxCreateExposure: "创建 Exposure",
    powerboxCreatingExposure: "正在创建…",
    powerboxExposuresEmpty: "这个 Installation 尚无 Exposure 记录。",
    powerboxTypedPortNotice: "当前公开 Installation 视图不枚举导出 Port 详情。请显式输入已验证的根 Port ID；Host 仍是权威，并会拒绝无效输入。",
    powerboxStartRunFirst: "创建 Run 作用域的 Exposure 前，请先启动一个 Run。",
    powerboxRuntimeContextNext: "请刷新活动 Run，并输入准确的 Runtime 导入 Port。",

    settingsTitle: "设置",
    settingsHelper: "设置保存在本机。无 SaaS 同步。",
    settingsApiConnections: "API 连接",
    settingsHostAccess: "Host 访问",
    settingsInstalledPackages: "已安装包",
    settingsProfiles: "配置档",
    settingsStorage: "存储",
    settingsAbout: "关于",

    accessEyebrow: "Host 控制平面",
    accessTitle: "设备与访问",
    accessDescription:
      "无需复制 Host 根令牌即可配对手机和浏览器。每台设备都有明确的权限范围、到期时间和可撤销授权，并持久记录在 Host 日志中。",
    accessIdentityUnknown: "未知 Host 身份",
    accessRootIdentity: "Host 根权限",
    accessDeviceIdentity: "已配对设备会话",
    accessRefresh: "刷新",
    accessLimitedTitle: "此设备仅有受限权限",
    accessLimitedBody: "它可以使用 Host，但不能创建、查看或撤销其他设备授权。",
    accessCreateTitle: "配对另一台设备",
    accessCreateBody:
      "创建一个短时有效的邀请。邀请只能使用一次，生成的设备会话不会暴露 Host 根凭据。",
    accessDeviceName: "设备名称",
    accessDevicePlaceholder: "例如：林的手机",
    accessGrantDays: "授权有效期（天）",
    accessGrantDaysHelper: "可设置 1 到 365 天。",
    accessPublicHostUrl: "Host HTTPS 地址",
    accessPublicHostUrlHelper: "手机必须能够访问这个准确的 Host 地址。",
    accessHttpsRequired: "请输入此 Host 的公网 HTTPS 源地址，不要包含路径。",
    accessPermissions: "设备能力",
    accessInstallationResources: "项目访问范围",
    accessInstallationResourcesBody: "可授权全部项目，或输入以英文逗号分隔的准确项目 ID。",
    accessAllInstallations: "全部项目",
    accessInstallationIdsPlaceholder: "installation-a, installation-b",
    accessTargetResources: "执行目标范围",
    accessTargetResourcesBody: "可授权全部 target，或输入以英文逗号分隔的准确 target ID。",
    accessAllTargets: "全部 target",
    accessTargetIdsPlaceholder: "local, preview-server",
    accessInstallationResource: (id) => `项目 · ${id}`,
    accessTargetResource: (id) => `Target · ${id}`,
    accessRunResources: "Run 范围",
    accessRunResourcesBody: "可授权全部 Run，或输入 Runtime Binding 与 Exposure 使用的准确 Run ID。",
    accessAllRuns: "全部 Run",
    accessRunIdsPlaceholder: "run-a, run-b",
    accessRunResource: (id) => `Run · ${id}`,
    accessPortResources: "Powerbox Port 范围",
    accessPortResourcesBody: "可授权全部 Port，或输入以英文逗号分隔的准确 InstallationID/root-PortID selector。",
    accessAllPorts: "全部 Port",
    accessPortIdsPlaceholder: "installation-id/save-import, installation-id/lobby-export",
    accessPortResource: (id) => `Port · ${id}`,
    accessExposureResources: "Exposure 范围",
    accessExposureResourcesBody: "可授权全部 Exposure，或输入以英文逗号分隔的准确 Exposure ID。",
    accessAllExposures: "全部 Exposure",
    accessExposureIdsPlaceholder: "exposure-a, exposure-b",
    accessExposureResource: (id) => `Exposure · ${id}`,
    accessBindingResources: "Binding 范围",
    accessBindingResourcesBody: "可授权全部 Binding，或输入以英文逗号分隔的准确 Binding ID。",
    accessAllBindings: "全部 Binding",
    accessBindingIdsPlaceholder: "binding-a, binding-b",
    accessBindingResource: (id) => `Binding · ${id}`,
    accessCreateValidation: "请填写设备名称、1–365 天有效期和有效的 Host HTTPS 地址。",
    accessCreating: "正在创建…",
    accessCreateButton: "创建配对链接",
    accessOneTimeEyebrow: "一次性邀请",
    accessOneTimeTitle: "把此链接发送到目标设备",
    accessOneTimeBody: "链接将在 10 分钟后过期，并在首次成功领取后永久失效。请把它视为短期秘密。",
    accessCopyLink: "复制链接",
    accessCopied: "已复制",
    accessShareLink: "分享…",
    accessPairingShareText: "将此设备与我的 Plurora Host 配对。",
    accessDevicesTitle: "设备授权",
    accessDevicesBody: "此 Host 已知的持久、受限设备会话。",
    accessNoDevices: "尚无已配对的设备授权。",
    accessCurrentDevice: "当前设备",
    accessStatusActive: "有效",
    accessStatusRevoked: "已撤销",
    accessStatusExpired: "已过期",
    accessExpires: (date) => `到期于 ${date}`,
    accessRevoke: "撤销",
    accessRevokeConfirm: (name) => `撤销 ${name} 的 Host 访问权限？`,
    accessPendingTitle: "待领取邀请",
    accessPendingBody: "尚未领取且未过期的配对链接。",
    accessNoPending: "没有待领取邀请。",
    accessTicketExpires: (time) => `邀请将在 ${time} 过期`,
    accessCancelTicket: "取消",
    accessScopeObserve: "查看",
    accessScopeObserveBody: "读取 Host、项目、部署和诊断状态。",
    accessScopeInstallationOperate: "操作项目",
    accessScopeInstallationOperateBody: "启动、停止、打开和分叉项目会话。",
    accessScopeBindingManage: "管理 Binding",
    accessScopeBindingManageBody: "针对准确的消费方 Installation、Port、Exposure 和 Binding 显式选择或撤销提供方。",
    accessScopeExposureManage: "管理 Exposure",
    accessScopeExposureManageBody: "针对准确的提供方 Installation、Run、导出 Port、受众和 Exposure 创建或撤销暴露。",
    accessScopeRealization: "Realization",
    accessScopeRealizationBody: "规划、应用、对账、回滚和停止托管 Realization。",
    accessScopeDevelopPropose: "提出变更",
    accessScopeDevelopProposeBody: "起草并查看受控项目变更集。",
    accessScopeDevelopApprove: "审批变更",
    accessScopeDevelopApproveBody: "批准或拒绝已经审阅的变更集。",
    accessScopeDevelopExecute: "执行变更",
    accessScopeDevelopExecuteBody: "运行、提升并恢复已批准的变更集。",
    accessScopeManage: "管理访问",
    accessScopeManageBody: "创建和撤销设备授权。仅应授予可信管理员。",

    apiEyebrowLoading: "API 连接 · 加载中…",
    apiEyebrowCount: (count) => `API 连接 · 已存储 ${count} 个密钥`,
    apiTitle: "本地密钥存储",
    apiDescription:
      "密钥保留在本机，并使用平台密钥加密。Plurora 不会传输原始密钥——项目通过可审计的能力调用请求它们。",
    apiStoredSecrets: "已存密钥",
    apiAddSecret: "添加密钥",
    apiLoadErrorTitle: "无法加载密钥",
    apiLoadErrorBody: "密钥元数据暂不可用。请从本地 UI 重试。",
    apiEmptyTitle: "尚未存储密钥",
    apiEmptyBody: "添加第一个密钥。Plurora 会使用平台密钥加密它。",
    apiStoreStatus: "存储状态",
    apiEncryption: "加密",
    apiMasterKey: "主密钥",
    apiStorage: "存储",
    apiTotal: "总计",
    apiConfigured: "已配置",
    apiNotCreated: "未创建",
    apiSecretsCount: (count) => `${count} 个密钥`,
    apiHowUsed: "使用方式",
    apiHowUsedBody: "主机会代表项目把原始值注入到外部请求中。",
    apiOpenAuditLog: "打开审计日志 →",
    apiBackup: "备份",
    apiExportFile: "导出到文件",
    apiImportFile: "从文件导入",
    apiExportToast: "请在 CLI 使用 plurora secrets export",
    apiImportToast: "请在 CLI 使用 plurora secrets import",
    apiRemoved: (name) => `已移除 ${name}`,
    apiDeleteFailedTitle: "删除失败",
    apiDeleteFailedBody: "无法移除该密钥。请检查本地主机后重试。",
    apiCopiedSecretName: "已复制密钥名称",
    apiStored: (name) => `已存储 ${name}`,
    apiSaveFailedTitle: "保存失败",
    apiSaveFailedBody: "无法存储该密钥。请检查本地主机后重试。",
    apiHideName: "隐藏名称",
    apiRevealName: "显示名称",
    apiToggleReveal: "切换显示",
    apiCopyName: "复制名称",
    apiCopy: "复制",
    apiMore: "更多",
    apiRotate: "轮换",
    apiDelete: "删除…",
    apiAddContentLabel: "添加密钥",
    apiAddEyebrow: "API 连接 · 添加",
    apiAddTitle: "存储新密钥",
    apiAddDescription: "Plurora 会使用平台密钥加密该值，且永远不会把原始密钥发送给任何项目。",
    apiProvider: "提供商",
    apiSecretName: "密钥名称",
    apiSecretNameHelper: "约定：PROVIDER_API_KEY（大写，下划线）",
    apiValue: "值",
    apiValueHelper: "原始密钥永远不会离开本机。",
    apiScope: "作用域",
    apiScopePlatform: "全平台",
    apiScopeInstallation: "仅项目（在首页配置）",
    cancel: "取消",
    apiSaveKey: "保存密钥",

    packagesEyebrowLoading: "已安装包 · 加载中…",
    packagesEyebrowCount: (count) => `已安装包 · ${count} 个包`,
    packagesTitle: "工作台清单",
    packagesDescription: "此工作台中安装的项目、第一方 Package 和依赖。刷新会检查上游来源。",
    packagesFilterPlaceholder: "筛选包…",
    packagesFilterAll: "全部",
    packagesFilterInstallations: "项目",
    packagesFilterPlurora: "Plurora",
    packagesFilterThirdParty: "第三方",
    packagesRefreshing: "正在刷新清单…",
    packagesRefresh: "刷新",
    packagesLoadErrorTitle: "无法加载包",
    packagesLoadErrorBody: "包清单暂不可用。请从本地 UI 重试。",
    packagesEmptyTitle: "尚未安装包",
    packagesNoMatchTitle: "没有包匹配此筛选",
    packagesEmptyBody: "从首页安装项目，或在 CLI 运行 plurora install。",
    packagesNoMatchBody: "尝试其他筛选，或清空搜索。",
    packagesTablePackage: "包",
    packagesTableVersion: "版本",
    packagesTableKind: "类型",
    packagesTableCapabilities: "能力",
    packagesTableState: "状态",
    packagesCopyId: "复制包 ID",
    packagesViewPermissions: "查看权限",
    packagesViewLogs: "查看日志",
    packagesUninstall: "卸载…",
    packagesShowing: (visible, total) => `正在显示 ${visible} / ${total}`,
    packagesShowAll: "显示全部 →",
    packagesCopiedId: "已复制包 ID",
    packagesLogsTitle: (packageId) => `${packageId} 的脱敏日志`,
    packagesNoLogsTitle: "没有可用日志",
    packagesNoLogsBody: "内核没有返回该包的限定脱敏日志尾部。",
    packagesLogsLoadErrorTitle: "无法加载日志",
    packagesLogsLoadErrorBody: "诊断暂不可用。请重试或检查本地 CLI 日志。",

    profilesEyebrowLoading: "配置档 · 加载中…",
    profilesEyebrowActive: (name) => `配置档 · 当前：${name}`,
    profilesEyebrowNone: "配置档 · 没有当前配置档",
    profilesTitle: "工作台配置档",
    profilesDescriptionPrefix: "配置档会打包主机配置：自动加载哪些包、允许哪些外部主机、密钥解析设置。配置档是传给",
    profilesDescriptionSuffix: "的 YAML 文件。",
    profilesOnMachine: "本机配置档",
    profilesNew: "新建配置档",
    profilesCreateTitle: "创建配置档",
    profilesCreateBody: "创建一个 YAML 配置档，并用 --profile <path> 启动主机。",
    profilesDiagnosticsErrorTitle: "无法读取主机诊断",
    profilesDiagnosticsErrorBody: "主机诊断暂不可用。请从本地 UI 重试。",
    profilesEmptyTitle: "没有使用配置档",
    profilesEmptyBody: "用 --profile <path> 启动主机以启用配置档相关功能。",
    profilesActive: "当前",
    profilesLoadedPackages: "已加载包",
    profilesLoadedPackagesHint: "定义在配置档的 packages 列表中。",
    profilesNetworkAllowlist: "网络允许列表",
    profilesOutboundBlocked: "所有外联均被阻止。",
    profilesSwitch: "切换配置档…",
    profilesSwitchHint: "切换会重启主机。项目状态会保留。",
    profilesSwitchRequiresRestart: "切换配置档需要重启",
    profilesSwitchBody: (id) => `请在 CLI 使用 plurora host serve --profile profiles/${id}.yaml 激活。`,
    profilesSwitchViaCli: "通过 CLI 切换配置档",
    profilesDefaultDescription: (packages, hosts) =>
      `当前配置档 · 已加载 ${packages} 个包 · 允许 ${hosts} 个主机`,

    hostConnectionsEyebrowActive: (name) => `Host 连接 · ${name}`,
    hostConnectionsTitle: "Host 连接",
    hostConnectionsDescription:
      "可以继续使用承载当前客户端的托管 Host，也可以让同一套 Web、PWA 与桌面界面显式连接远程 Host。每个连接使用独立凭据。",
    hostConnectionsSaved: "已保存的 Host 连接",
    hostConnectionsCurrent: "当前托管 Host",
    hostConnectionsNew: "添加 Host",
    hostConnectionsNewTitle: "连接另一台 Host",
    hostConnectionsNewBody: "远程 Host 必须使用 HTTPS；明文 HTTP 仅允许 loopback 开发地址。",
    hostConnectionsName: "连接名称",
    hostConnectionsEndpoint: "Host 地址",
    hostConnectionsEndpointHint: "只填写 Host origin；凭据、路径、查询参数和 fragment 都会被拒绝。",
    hostConnectionsConnect: "保存并连接",
    hostConnectionsRemove: "移除",
    hostConnectionsRemoveConfirm: (name) => `移除 ${name} 及其保存在本机的凭据？`,
    hostConnectionsReturnCurrent: "返回托管 Host",
    hostConnectionsCredentialHint:
      "连接配置只保存地址和显示名称。访问令牌仍仅保存在浏览器本地，并按 Host 隔离。",

    storageTitleEyebrow: "存储",
    storageTitle: "数据存放位置",
    storageDescription: "Plurora 默认把数据保存在本机。UI 会概述存储区域，但不暴露主机特定的绝对路径。",
    storageAreas: "存储区域",
    storageAreaInstallationData: "项目数据",
    storageAreaInstallationDataDesc: "项目元数据、检查点、包状态和运行记录。",
    storageAreaPackageStore: "包存储",
    storageAreaPackageStoreDesc: "已安装包源码和由锁文件管理的修订版本。",
    storageAreaProfiles: "配置档",
    storageAreaProfilesDesc: "传给 plurora host serve --profile 的主机配置档。",
    storageAreaSecrets: "密钥",
    storageAreaSecretsDesc: "加密的平台和项目密钥存储。这里永远不会显示原始值。",
    storageAreaCache: "缓存",
    storageAreaCacheDesc: "生成的 bundle、分词器缓存和其他可重建数据。",
    storageEventStore: "事件存储",
    storageSqliteDesc: "本地文件后端，单主机工作台的默认选项。",
    storagePostgresDesc: "PostgreSQL 后端，适合共享/团队主机。",
    storageMemoryDesc: "内存后端，重启后不持久化。",
    storageCustomDesc: "自定义后端。",
    storageBackendNeutrality: "后端中立",
    storageBackendBody:
      "Plurora 的存储层保持后端中立。SQLite 是本地单主机工作台的默认选项。PostgreSQL 保留给共享/团队主机。多模态检索提供方（TDB、pgvector 等）会作为普通能力包暴露，而不是内核原语。",

    aboutEyebrow: "关于",
    aboutSubtitle: "面向游玩与创作的开放平台。",
    aboutVersion: "版本",
    aboutBuild: "构建",
    aboutReleased: "发布日期",
    aboutChannel: "通道",
    aboutWhat: "Plurora 是什么",
    aboutPara1:
      "Plurora 是一个内核，也是一份契约。内核在沙盒中托管你的项目；契约让任何项目——Plurora、社区或自建——都能成为一等公民。",
    aboutPara2:
      "它运行在你的机器上，使用你的密钥、你的文件和你的网络。没有 SaaS 账号、没有中心注册表、没有遥测。你安装的项目保存在本地平台数据目录中，直到你移除它们。",
    aboutPara3: "你现在看到的 shell 只是众多可能 UI 之一。任何人都可以编写另一个。平台是契约，而不是这个窗口。",
    aboutCredits: "致谢",
    aboutBuiltOn: "构建于",
    aboutFonts: "字体",
    aboutIcons: "图标",
    aboutLicense: "许可证",
    aboutLicenseBody: "可自由使用、修改和运行。网络使用需要披露源码。",
    aboutReadLicense: "阅读完整许可证 →",
    aboutLinks: "链接",
    aboutSourceCode: "源代码",
    aboutDocumentation: "文档",
    aboutReportIssue: "报告问题",
    aboutCommunity: "社区",
    aboutChangelog: "变更日志",
    aboutGratitude: "感谢",
    aboutGratitudeBody: "感谢 SillyTavern 社区，其资产格式和扩展 API 模式为 YdlTavern 兼容工作提供了参考。",
  },
} satisfies Record<SupportedLocale, LocaleDictionary>;

export type LabelKey = keyof LocaleDictionary;
