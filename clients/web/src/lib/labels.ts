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
  topbarProject: (projectId: string) => string;
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
  homeProjectPopupBlockedTitle: string;
  homeProjectPopupBlockedBody: string;

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
  installCompleteBody: (count: number, projectId?: string) => string;
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
  installProjectSection: string;
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
  installActivityRegisteredProject: (projectId: string) => string;
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

  failureProjectFallback: string;
  failureContentLabel: (projectName: string) => string;
  failureEyebrow: (projectName: string) => string;
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
  failureRestartProject: string;

  projectFrameStartFailedTitle: string;
  projectFrameStartFailedBody: string;
  projectFrameMountFailedTitle: string;
  projectFrameMountFailedBody: string;
  projectFrameStopped: (title: string) => string;
  projectFrameStopFailedTitle: string;
  projectFrameStopFailedBody: string;
  projectFrameBackHome: string;
  projectFrameAuditLog: string;
  projectFrameAuditLogUnavailable: string;
  projectFrameStopProject: string;
  projectFrameStop: string;
  projectFrameMore: string;
  projectFrameMoreUnavailable: string;
  projectFrameState: (state: string) => string;
  projectFrameLoadingSurface: string;
  projectFrameStoppedTitle: string;
  projectFrameStoppedBody: string;
  projectFrameConsoleTitle: string;
  projectFrameConsoleBody: string;
  projectFrameOpenProjectTab: string;
  projectFrameProjectTabBlockedTitle: string;
  projectFrameProjectTabBlockedBody: string;
  projectFrameRefresh: string;
  projectFrameRefreshing: string;
  projectFrameRefreshDiagnostics: string;
  projectFrameUpdateProject: string;
  projectFrameUpdating: string;
  projectFrameUpdateCompleteTitle: string;
  projectFrameUpdateCompleteBody: (count: number) => string;
  projectFrameUpdateCurrentTitle: string;
  projectFrameUpdateCurrentBody: string;
  projectFrameUpdateFailedTitle: string;
  projectFrameStopConfirm: string;
  projectFrameDiagnosticsLoading: string;
  projectFrameStatus: string;
  projectFramePackages: string;
  projectFrameUpdates: string;
  projectFrameActivity: string;
  projectFramePackageHealth: (healthy: number, total: number) => string;
  projectFrameRecentEvents: (count: number) => string;
  projectFrameUpdatesAvailable: (count: number) => string;
  projectFrameUpdatesCurrent: string;
  projectFrameUpdateUnavailable: string;
  projectFrameDeploymentMetric: string;
  projectFrameDeploymentSummary: (targets: number, executions: number, running: number, ports: number, proxies: number) => string;
  projectFrameProjectId: string;
  projectFrameProjectType: string;
  projectFrameSession: string;
  projectFrameActiveSession: string;
  projectFrameStorage: string;
  projectFrameInterfaceSection: string;
  projectFrameInterfaceDescription: string;
  projectFrameEntrySurface: string;
  projectFrameBundleUrl: string;
  projectFrameBundleFingerprint: string;
  projectFrameBundleUnavailable: string;
  projectFrameLastResolved: string;
  projectFrameUpdatesSection: string;
  projectFrameUpdatesDescription: string;
  projectFrameNoUpdateRecords: string;
  projectFramePackagesSection: string;
  projectFramePackagesDescription: string;
  projectFrameNoPackages: string;
  projectFramePackageCounts: (capabilities: number, hooks: number) => string;
  projectFrameActivitySection: string;
  projectFrameActivityDescription: string;
  projectFrameNoEvents: string;
  projectFrameNoSession: string;
  projectFrameDiagnosticsWarnings: string;
  projectFrameDevelopmentSection: string;
  projectFrameDevelopmentDescription: string;
  projectFrameDevelopmentLoadFailed: string;
  projectFrameDevelopmentGoal: string;
  projectFrameDevelopmentGoalPlaceholder: string;
  projectFrameDevelopmentTarget: string;
  projectFrameDevelopmentOperation: string;
  projectFrameDevelopmentWrite: string;
  projectFrameDevelopmentDelete: string;
  projectFrameDevelopmentExecutable: string;
  projectFrameDevelopmentDockerBuild: string;
  projectFrameDevelopmentAllowNetwork: string;
  projectFrameDevelopmentDockerfile: string;
  projectFrameDevelopmentContent: string;
  projectFrameDevelopmentSafetyHint: string;
  projectFrameDevelopmentDrafting: string;
  projectFrameDevelopmentDraft: string;
  projectFrameDevelopmentHistory: string;
  projectFrameDevelopmentHistoryDescription: string;
  projectFrameDevelopmentRefresh: string;
  projectFrameDevelopmentEmpty: string;
  projectFrameDevelopmentDrafted: string;
  projectFrameDevelopmentDraftFailed: string;
  projectFrameDevelopmentApproveConfirm: string;
  projectFrameDevelopmentDecisionFailed: string;
  projectFrameDevelopmentExecuteConfirm: string;
  projectFrameDevelopmentExecutionStarted: string;
  projectFrameDevelopmentExecutionAlreadyActive: string;
  projectFrameDevelopmentExecutionFailed: string;
  projectFrameDevelopmentRecovered: string;
  projectFrameDevelopmentRecoveryFailed: string;
  projectFrameDevelopmentExportFailed: string;
  projectFrameDevelopmentLinkedHint: string;
  projectFrameDevelopmentReviewOperations: string;
  projectFrameDevelopmentReviewVerification: string;
  projectFrameDevelopmentReviewAuthority: string;
  projectFrameDevelopmentReviewEffects: string;
  projectFrameDevelopmentApprovalRecord: string;
  projectFrameDevelopmentRecoveryTarget: string;
  projectFrameDevelopmentVerificationStatic: string;
  projectFrameDevelopmentVerificationDocker: string;
  projectFrameDevelopmentStatusDrafted: string;
  projectFrameDevelopmentStatusApproved: string;
  projectFrameDevelopmentStatusRejected: string;
  projectFrameDevelopmentStatusStaging: string;
  projectFrameDevelopmentStatusVerifying: string;
  projectFrameDevelopmentStatusPromoting: string;
  projectFrameDevelopmentStatusVerified: string;
  projectFrameDevelopmentStatusCommitted: string;
  projectFrameDevelopmentStatusRecoveryRequired: string;
  projectFrameDevelopmentStatusFailed: string;
  projectFrameDevelopmentExport: string;
  projectFrameDevelopmentReject: string;
  projectFrameDevelopmentApprove: string;
  projectFrameDevelopmentExecute: string;
  projectFrameDevelopmentRecover: string;
  projectFrameDevelopmentDeploymentTitle: string;
  projectFrameDevelopmentDeploymentDescription: string;
  projectFrameDevelopmentDeploymentStatusPreparing: string;
  projectFrameDevelopmentDeploymentStatusBuilding: string;
  projectFrameDevelopmentDeploymentStatusPreviewing: string;
  projectFrameDevelopmentDeploymentStatusActivating: string;
  projectFrameDevelopmentDeploymentInvalidConfig: string;
  projectFrameDevelopmentDeploymentPreview: string;
  projectFrameDevelopmentDeploymentPreviewing: string;
  projectFrameDevelopmentDeploymentPreviewConfirm: (targetId: string, routeId: string) => string;
  projectFrameDevelopmentDeploymentPreviewStarted: string;
  projectFrameDevelopmentDeploymentPreviewFailed: string;
  projectFrameDevelopmentDeploymentSourceTree: string;
  projectFrameDevelopmentDeploymentBuildContext: string;
  projectFrameDevelopmentDeploymentCandidate: string;
  projectFrameDevelopmentDeploymentOpenPreview: string;
  projectFrameDevelopmentDeploymentApproveConfirm: (targetId: string, routeId: string, exposure: string) => string;
  projectFrameDevelopmentDeploymentApproved: string;
  projectFrameDevelopmentDeploymentRejected: string;
  projectFrameDevelopmentDeploymentActivate: string;
  projectFrameDevelopmentDeploymentActivating: string;
  projectFrameDevelopmentDeploymentActivateConfirm: (targetId: string, routeId: string, exposure: string) => string;
  projectFrameDevelopmentDeploymentActivated: string;
  projectFrameDevelopmentDeploymentActivationFailed: string;
  projectFrameDevelopmentDeploymentRecoveryHint: string;
  projectFrameDevelopmentDeploymentReconcile: string;
  projectFrameDevelopmentDeploymentReconciling: string;
  projectFrameDevelopmentDeploymentReconcileConfirm: string;
  projectFrameDevelopmentDeploymentReconciled: string;
  projectFrameDevelopmentDeploymentReconciliationFailed: string;
  projectFrameDevelopmentDeploymentOpenProduction: string;
  projectFrameDeploymentSection: string;
  projectFrameDeploymentDescription: string;
  projectFrameRemoteTargetOperationsOnly: string;
  projectFrameTargetOperations: string;
  projectFrameTargetOperationEmpty: string;
  projectFrameTargetOperationShowDetails: string;
  projectFrameTargetOperationHideDetails: string;
  projectFrameTargetOperationRevision: string;
  projectFrameTargetOperationExecution: string;
  projectFrameTargetOperationUpdatedAt: string;
  projectFrameTargetOperationRequestDigest: string;
  projectFrameTargetOperationAuthorityDigest: string;
  projectFrameTargetOperationOutput: string;
  projectFrameTargetOperationDiagnostics: string;
  projectFrameTargetOperationNoDiagnostics: string;
  projectFrameTargetOperationReceiptPending: string;
  projectFrameTargetOperationShowingRecent: (shown: number, total: number) => string;
  projectFrameDeploymentExecutions: string;
  projectFrameDeploymentPortLeases: string;
  projectFrameDeploymentProxyRoutes: string;
  projectFrameDeploymentEmpty: string;
  projectFrameDeploymentReady: string;
  projectFrameDeploymentExitCode: string;
  projectFrameDeploymentHistoryTitle: string;
  projectFrameDeploymentHistoryDescription: string;
  projectFrameDeploymentActiveRevision: string;
  projectFrameDeploymentRecoveryRequired: string;
  projectFrameDeploymentRecover: string;
  projectFrameDeploymentRecovering: string;
  projectFrameDeploymentRecoverConfirm: string;
  projectFrameDeploymentRecoveredTitle: string;
  projectFrameDeploymentRecoveryFailedTitle: string;
  projectFrameDeploymentRevisionHistory: string;
  projectFrameDeploymentRollback: string;
  projectFrameDeploymentRollingBack: string;
  projectFrameDeploymentRollbackConfirm: (revisionId: string) => string;
  projectFrameDeploymentRolledBackTitle: string;
  projectFrameDeploymentRollbackFailedTitle: string;
  projectFrameDeploymentNotRecoverable: string;
  projectFrameDeploymentNoActiveOnTarget: (targetId: string) => string;
  projectFrameDeploymentJobHistory: string;
  projectFrameDeploymentCreatedAt: string;
  projectFrameDeploymentSourceCommit: string;
  projectFrameDeploymentBuildDigest: string;
  projectFrameDeploymentParentRevision: string;
  projectFrameRouteExposure: string;
  projectFrameRoutePrivate: string;
  projectFrameRoutePrivateBody: string;
  projectFrameRoutePublic: string;
  projectFrameRoutePublicBody: string;
  projectFrameRoutePublicWarning: string;
  projectFrameDeployActionTitle: string;
  projectFrameDeployActionDescription: string;
  projectFrameDeployInvalidTitle: string;
  projectFrameDeploy: string;
  projectFrameDeploying: string;
  projectFrameStopDeployment: string;
  projectFrameStoppingDeployment: string;
  projectFrameDeployImage: string;
  projectFrameDeployContainerPort: string;
  projectFrameDeployRouteId: string;
  projectFrameDeployPortName: string;
  projectFrameDeployHealthPath: string;
  projectFrameDeployContainerId: string;
  projectFrameDeployPullWarning: string;
  projectFrameDeployActiveHint: string;
  projectFrameDeployConfirm: (image: string) => string;
  projectFrameStopDeploymentConfirm: (routeId: string) => string;
  projectFrameDeploySuccessTitle: string;
  projectFrameDeploySuccessBody: (routeId: string) => string;
  projectFrameDeployFailedTitle: string;
  projectFrameDeployFailedBody: string;
  projectFrameStopDeploymentSuccessTitle: string;
  projectFrameStopDeploymentPartialTitle: string;
  projectFrameBuildDeployTitle: string;
  projectFrameBuildDeployDescription: string;
  projectFrameBuildDeployInvalidTitle: string;
  projectFrameBuildDeployButton: string;
  projectFrameBuildDeployRunning: string;
  projectFrameBuildDeployCancel: string;
  projectFrameBuildDeployCancelConfirmTitle: string;
  projectFrameBuildDeployCancelConfirmBody: string;
  projectFrameBuildDeployStrategy: string;
  projectFrameBuildDeploySource: string;
  projectFrameBuildDeployRef: string;
  projectFrameBuildDeployEnv: string;
  projectFrameBuildDeploySecretRef: string;
  projectFrameBuildDeployPlainEnv: string;
  projectFrameBuildDeployMounts: string;
  projectFrameBuildDeployApproveMount: string;
  projectFrameBuildDeployApproveRisk: string;
  projectFrameBuildDeployHighRiskBadge: string;
  projectFrameBuildDeployJob: string;
  projectFrameBuildDeployTracker: string;
  projectFrameBuildDeployCopyLogs: string;
  projectFrameBuildDeployNoLogs: string;
  projectFrameBuildDeployResultReady: string;
  projectFrameBuildDeployResultFailed: string;
  projectFrameBuildDeployResultCancelled: string;
  projectFrameBuildDeployNewBuild: string;
  projectFrameBuildDeployRestart: string;
  projectFrameBuildDeployEditConfig: string;
  projectFrameBuildDeployConfirm: (routeId: string) => string;
  projectFrameBuildDeployStartedTitle: string;
  projectFrameBuildDeployStartedBody: (jobId: string) => string;
  projectFrameBuildDeployFailedTitle: string;
  projectFrameBuildDeployCancelTitle: string;
  projectFrameBuildDeployReady: (url: string) => string;
  projectFrameStatusReady: string;
  projectFrameStatusNotReady: string;
  projectFrameShowLogs: string;
  projectFrameHideLogs: string;
  projectFrameLogsLoading: string;
  projectFrameNoLogs: string;
  projectFrameCopyAddress: string;
  projectFrameCopyUrl: string;
  projectFrameOpenUrl: string;
  projectFramePublicUrl: string;
  projectFrameIframeUrl: string;

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
  accessProjectResources: string;
  accessProjectResourcesBody: string;
  accessAllProjects: string;
  accessProjectIdsPlaceholder: string;
  accessTargetResources: string;
  accessTargetResourcesBody: string;
  accessAllTargets: string;
  accessTargetIdsPlaceholder: string;
  accessProjectResource: (id: string) => string;
  accessTargetResource: (id: string) => string;
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
  accessScopeProjectOperate: string;
  accessScopeProjectOperateBody: string;
  accessScopeDeploy: string;
  accessScopeDeployBody: string;
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
  apiScopeProject: string;
  cancel: string;
  apiSaveKey: string;

  packagesEyebrowLoading: string;
  packagesEyebrowCount: (count: number) => string;
  packagesTitle: string;
  packagesDescription: string;
  packagesFilterPlaceholder: string;
  packagesFilterAll: string;
  packagesFilterProjects: string;
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
  storageAreaProjectData: string;
  storageAreaProjectDataDesc: string;
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
    topbarProject: (projectId) => `Projects / ${projectId}`,
    topbarNotifications: "Notifications",
    topbarThemeSystem: (theme) => `System (${theme === "dark" ? "Dark" : "Light"})`,
    topbarThemeLight: "Light mode",
    topbarThemeDark: "Dark mode",
    topbarThemeAria: (preference) => `Theme preference: ${preference}`,
    topbarLogout: "Log out",

    homeGreeting: "Welcome back",
    homeEmptyWorkshop: "Your workshop is empty. Install a project to begin.",
    homeReading: "Reading your workshop…",
    homeShelfSummary: (all, running, stopped, failed) =>
      `${all} projects on the shelf. ${running} running, ${stopped} idle. ${failed > 0 ? `${failed} need attention.` : "No pending updates."}`,
    homeInstalledEyebrow: (count) => `Projects — ${count.toString().padStart(2, "0")} installed`,
    homeEmptyTitle: "No projects installed yet",
    homeEmptyBody:
      "Plurora is your workshop. Install a project to begin — projects can be a Plurora-native source like YdlTavern, or any external git/local repo.",
    homeInstallLabel: "Install a project",
    homeInstallHint: "Paste a GitHub URL or local path",
    homeErrorTitle: "Couldn't reach the host",
    homeErrorBody: "Project inventory is unavailable. Try again from the local UI.",
    retry: "Retry",
    homeSearchPlaceholder: "Search projects, packages...",
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
    homeNoStorageMeasured: "No project storage measured.",
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
    homeProjectPopupBlockedTitle: "Project tab was blocked",
    homeProjectPopupBlockedBody: "Allow pop-ups for this site, then open the project interface from the console.",

    homeStoppedToast: (title) => `Stopped ${title}`,
    homeStopFailedTitle: "Stop failed",
    homeStopFailedBody: "The project could not be stopped. Check the local host and try again.",
    homeUninstallTitle: (title) => `Uninstall ${title}`,
    homeUninstallingBody: "Removing installed packages from the active profile and archiving project data.",
    homeUninstalledTitle: (title) => `Uninstalled ${title}`,
    homeUninstalledBody: "Project data was archived locally. Reinstall the project to use it again.",
    homeUninstallFailedTitle: "Uninstall failed",
    homeUninstallFailedBody: "The project could not be uninstalled from the web shell. Check host diagnostics and retry.",
    homeLoadingDiagnostics: "Loading diagnostics…",
    homeLoadingDiagnosticsSummary: "Reading bounded package failure details from the kernel.",
    homeDescriptorNoPackages: "Project descriptor does not list packages.",
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
    homeContinueEmptyTitle: "No project opened yet",
    homeContinueEmptyBody: "Install a project to continue from here.",
    homeContinueEmptyInstall: "Install project",
    homeContinueEmptyTryYdltavern: "Try YdlTavern",
    homeContinuePickInstalled: "Pick one of your installed projects",

    close: "Close",
    back: "Back",
    continue: "Continue",

    uiModalClose: "Close",
    uiToastDismiss: "Dismiss",

    installModalContentLabel: "Install project",
    installUrlEyebrow: "Install — Step 1 of 3",
    installUrlTitle: "Where is the project?",
    installUrlDescription:
      "Plurora installs from public Git repositories or local folders. We'll review the project before anything runs.",
    installSourceLabel: "Source URL or path",
    installSourceHelper:
      "Public HTTPS Git only in the web shell. Local folders use the CLI or a native file picker flow.",
    installShortcuts: "Shortcuts",
    installResolveErrorTitle: "Could not resolve install plan",
    installKeyboardHint: "Press ⌘V to paste · ↵ to continue · Esc to cancel",
    installResolving: "Resolving…",
    installPlanFailedTitle: "Install plan failed",
    installCompleteTitle: "Install complete",
    installCompleteBody: (count, projectId) =>
      `${count} package${count === 1 ? "" : "s"} installed${projectId ? ` · project ${projectId}` : ""}`,
    installFailedTitle: "Install failed",
    installListMore: (count) => `+${count} more`,
    installKindNative: "Native project",
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
      "The package plan is real, but the web UI will not execute it without a project descriptor.",
    installProjectSection: "Project",
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
    installProgressTitleInstalling: "Installing project",
    installPhaseResolvedPlan: "Resolved install plan",
    installPhasePackageCount: (count) => `${count} package${count === 1 ? "" : "s"}`,
    installPhaseComplete: "complete",
    installPhaseDetectedKind: "Detected project kind",
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
    installActivityRegisteredProject: (projectId) => `registered project ${projectId}`,
    installActivityProfileUpdated: "profile updated · lockfile refreshed",

    installExternalEyebrow: "Install — External project",
    installExternalTitle: "External adapter generation is CLI-only",
    installExternalDescription:
      "This source does not declare a Plurora project descriptor. The web UI will not execute the package install without one.",
    installExternalStatus: "EXTERNAL",
    installExternalInfo:
      "Use the CLI to generate a descriptor for wrap/workspace mode, then install the declared project from web.",
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
    installExternalHelp: "Generate a project descriptor with the CLI, then return here.",
    installRecommended: "RECOMMENDED",
    installContinueDisabled: "Continue disabled",

    failureProjectFallback: "Project",
    failureContentLabel: (projectName) => `${projectName} failure details`,
    failureEyebrow: (projectName) => `Failure — ${projectName.toUpperCase()}`,
    failureTitle: "Project failed",
    failureDescription: "Project state is preserved. See the log below for the failure.",
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
    failureRestartProject: "Restart project",

    projectFrameStartFailedTitle: "Failed to start project",
    projectFrameStartFailedBody: "The project frame could not be started. Check the local host and try again.",
    projectFrameMountFailedTitle: "Project surface failed to mount",
    projectFrameMountFailedBody: "The project is running, but its browser surface could not be loaded. Check the local host and surface bundle.",
    projectFrameStopped: (title) => `Stopped ${title}`,
    projectFrameStopFailedTitle: "Stop failed",
    projectFrameStopFailedBody: "The project could not be stopped. Check the local host and try again.",
    projectFrameBackHome: "Back to Home",
    projectFrameAuditLog: "Audit log",
    projectFrameAuditLogUnavailable: "Audit log is not wired yet",
    projectFrameStopProject: "Stop project",
    projectFrameStop: "Stop",
    projectFrameMore: "More",
    projectFrameMoreUnavailable: "More project actions are not wired yet",
    projectFrameState: (state) => state.toUpperCase(),
    projectFrameLoadingSurface: "Loading project interface…",
    projectFrameStoppedTitle: "Project stopped",
    projectFrameStoppedBody: "This tab can be closed. Reopen the project from Home when you want to resume.",
    projectFrameConsoleTitle: "Project console",
    projectFrameConsoleBody: "The project is running from this Plurora tab. Its own interface opens in a separate project tab so the console controls remain available here.",
    projectFrameOpenProjectTab: "Open project interface",
    projectFrameProjectTabBlockedTitle: "Project tab was blocked",
    projectFrameProjectTabBlockedBody: "Allow pop-ups for this site, then open the project interface from the console.",
    projectFrameRefresh: "Refresh",
    projectFrameRefreshing: "Refreshing…",
    projectFrameRefreshDiagnostics: "Refresh diagnostics",
    projectFrameUpdateProject: "Update project",
    projectFrameUpdating: "Updating…",
    projectFrameUpdateCompleteTitle: "Project updated",
    projectFrameUpdateCompleteBody: (count) => `Updated ${count} package${count === 1 ? "" : "s"}.`,
    projectFrameUpdateCurrentTitle: "Project is current",
    projectFrameUpdateCurrentBody: "No package updates are available.",
    projectFrameUpdateFailedTitle: "Update failed",
    projectFrameStopConfirm: "Stopping will terminate the running session and project packages. Open work in the project UI may be lost. Stop this project?",
    projectFrameDiagnosticsLoading: "Loading diagnostics…",
    projectFrameStatus: "Status",
    projectFramePackages: "Packages",
    projectFrameUpdates: "Updates",
    projectFrameActivity: "Activity",
    projectFramePackageHealth: (healthy, total) => `${healthy}/${total} healthy`,
    projectFrameRecentEvents: (count) => `${count} recent event${count === 1 ? "" : "s"}`,
    projectFrameUpdatesAvailable: (count) => `${count} update${count === 1 ? "" : "s"} available`,
    projectFrameUpdatesCurrent: "Up to date",
    projectFrameUpdateUnavailable: "Update check unavailable",
    projectFrameDeploymentMetric: "Deployment",
    projectFrameDeploymentSummary: (targets, executions, running, ports, proxies) =>
      `${targets} targets · ${running}/${executions} exec running · ${ports} ports · ${proxies} proxies`,
    projectFrameProjectId: "Project ID",
    projectFrameProjectType: "Type",
    projectFrameSession: "Session",
    projectFrameActiveSession: "Active",
    projectFrameStorage: "Storage",
    projectFrameInterfaceSection: "Project interface",
    projectFrameInterfaceDescription: "Standalone project tab plus the resolved surface bundle used by the iframe host.",
    projectFrameEntrySurface: "Entry surface",
    projectFrameBundleUrl: "Bundle URL",
    projectFrameBundleFingerprint: "Fingerprint",
    projectFrameBundleUnavailable: "Bundle could not be resolved",
    projectFrameLastResolved: "Last resolved",
    projectFrameUpdatesSection: "Updates",
    projectFrameUpdatesDescription: "Checks use plurora/install-lab capabilities through capability.invoke.",
    projectFrameNoUpdateRecords: "No update records returned for this project.",
    projectFramePackagesSection: "Package health",
    projectFramePackagesDescription: "Runtime package state, counts, and redacted failure/log summaries when available.",
    projectFrameNoPackages: "No project packages were reported by the host.",
    projectFramePackageCounts: (capabilities, hooks) => `${capabilities} capabilities · ${hooks} hooks`,
    projectFrameActivitySection: "Recent activity",
    projectFrameActivityDescription: "Best-effort tail of the running project session events.",
    projectFrameNoEvents: "No recent events for this session.",
    projectFrameNoSession: "No running session is available yet.",
    projectFrameDiagnosticsWarnings: "Diagnostic warnings",
    projectFrameDevelopmentSection: "Development",
    projectFrameDevelopmentDescription: "Draft content-addressed ChangeSets, approve them explicitly, verify in an isolated scratch workspace, then promote only Host-owned trees.",
    projectFrameDevelopmentLoadFailed: "Development history unavailable",
    projectFrameDevelopmentGoal: "Goal",
    projectFrameDevelopmentGoalPlaceholder: "Describe the intended source change",
    projectFrameDevelopmentTarget: "Relative file path",
    projectFrameDevelopmentOperation: "Operation",
    projectFrameDevelopmentWrite: "Write file",
    projectFrameDevelopmentDelete: "Delete file",
    projectFrameDevelopmentExecutable: "Executable file",
    projectFrameDevelopmentDockerBuild: "Verify with Docker build",
    projectFrameDevelopmentAllowNetwork: "Allow Docker build egress",
    projectFrameDevelopmentDockerfile: "Dockerfile path",
    projectFrameDevelopmentContent: "UTF-8 source content",
    projectFrameDevelopmentSafetyHint: "Drafts store source blobs by digest, never raw source in the journal. Docker scratch builds default to no network and discard verification images. Managed external trees may be promoted; native trees are verify-only, and linked-local projects must first be imported as managed.",
    projectFrameDevelopmentDrafting: "Drafting…",
    projectFrameDevelopmentDraft: "Draft ChangeSet",
    projectFrameDevelopmentHistory: "Change history",
    projectFrameDevelopmentHistoryDescription: "Durable intent, policy, approval, verification, commit, and effect receipts.",
    projectFrameDevelopmentRefresh: "Refresh",
    projectFrameDevelopmentEmpty: "No development ChangeSets have been drafted for this project.",
    projectFrameDevelopmentDrafted: "Development ChangeSet drafted",
    projectFrameDevelopmentDraftFailed: "Could not draft development ChangeSet",
    projectFrameDevelopmentApproveConfirm: "Approve this exact ChangeSet and its listed filesystem, Docker, and network authority?",
    projectFrameDevelopmentDecisionFailed: "Could not record approval decision",
    projectFrameDevelopmentExecuteConfirm: "Execute the approved ChangeSet in a Host-owned scratch workspace now?",
    projectFrameDevelopmentExecutionStarted: "Development execution started",
    projectFrameDevelopmentExecutionAlreadyActive: "Development execution is already active; status refreshed",
    projectFrameDevelopmentExecutionFailed: "Could not start development execution",
    projectFrameDevelopmentRecovered: "Development promotion reconciled",
    projectFrameDevelopmentRecoveryFailed: "Development promotion still requires recovery",
    projectFrameDevelopmentExportFailed: "Could not export patch bundle",
    projectFrameDevelopmentLinkedHint: "Linked-local projects are proposal-only. Import a managed copy before Host verification; Plurora never writes the linked user source.",
    projectFrameDevelopmentReviewOperations: "Exact operations",
    projectFrameDevelopmentReviewVerification: "Verification",
    projectFrameDevelopmentReviewAuthority: "Required authority",
    projectFrameDevelopmentReviewEffects: "Expected effects",
    projectFrameDevelopmentApprovalRecord: "Recorded approval decision",
    projectFrameDevelopmentRecoveryTarget: "Recovery reconciliation target",
    projectFrameDevelopmentVerificationStatic: "Static validation (no code execution)",
    projectFrameDevelopmentVerificationDocker: "Docker build verification",
    projectFrameDevelopmentStatusDrafted: "Drafted",
    projectFrameDevelopmentStatusApproved: "Approved",
    projectFrameDevelopmentStatusRejected: "Rejected",
    projectFrameDevelopmentStatusStaging: "Staging",
    projectFrameDevelopmentStatusVerifying: "Verifying",
    projectFrameDevelopmentStatusPromoting: "Promoting",
    projectFrameDevelopmentStatusVerified: "Verified",
    projectFrameDevelopmentStatusCommitted: "Committed",
    projectFrameDevelopmentStatusRecoveryRequired: "Recovery required",
    projectFrameDevelopmentStatusFailed: "Failed",
    projectFrameDevelopmentExport: "Export bundle",
    projectFrameDevelopmentReject: "Reject",
    projectFrameDevelopmentApprove: "Approve",
    projectFrameDevelopmentExecute: "Execute",
    projectFrameDevelopmentRecover: "Reconcile",
    projectFrameDevelopmentDeploymentTitle: "Verified deployment",
    projectFrameDevelopmentDeploymentDescription: "Build the committed CAS context on the selected target, inspect its private preview, then approve and activate that exact candidate.",
    projectFrameDevelopmentDeploymentStatusPreparing: "Preparing",
    projectFrameDevelopmentDeploymentStatusBuilding: "Building",
    projectFrameDevelopmentDeploymentStatusPreviewing: "Previewing",
    projectFrameDevelopmentDeploymentStatusActivating: "Activating",
    projectFrameDevelopmentDeploymentInvalidConfig: "Container port, port name, and route ID must be valid.",
    projectFrameDevelopmentDeploymentPreview: "Create private preview",
    projectFrameDevelopmentDeploymentPreviewing: "Preparing preview…",
    projectFrameDevelopmentDeploymentPreviewConfirm: (targetId, routeId) => `Build and run this verified change on ${targetId} as a private preview for route ${routeId}?`,
    projectFrameDevelopmentDeploymentPreviewStarted: "Private deployment preview started",
    projectFrameDevelopmentDeploymentPreviewFailed: "Could not create deployment preview",
    projectFrameDevelopmentDeploymentSourceTree: "Verified source tree",
    projectFrameDevelopmentDeploymentBuildContext: "CAS build context",
    projectFrameDevelopmentDeploymentCandidate: "Private candidate",
    projectFrameDevelopmentDeploymentOpenPreview: "Open private preview",
    projectFrameDevelopmentDeploymentApproveConfirm: (targetId, routeId, exposure) => `Approve the exact candidate and evidence shown here for target ${targetId}, route ${routeId}, with ${exposure} exposure?`,
    projectFrameDevelopmentDeploymentApproved: "Deployment candidate approved",
    projectFrameDevelopmentDeploymentRejected: "Deployment candidate rejected",
    projectFrameDevelopmentDeploymentActivate: "Activate",
    projectFrameDevelopmentDeploymentActivating: "Activating…",
    projectFrameDevelopmentDeploymentActivateConfirm: (targetId, routeId, exposure) => `Atomically switch production route ${routeId} on target ${targetId} to this approved candidate with ${exposure} exposure?`,
    projectFrameDevelopmentDeploymentActivated: "Verified deployment activated",
    projectFrameDevelopmentDeploymentActivationFailed: "Could not activate verified deployment",
    projectFrameDevelopmentDeploymentRecoveryHint: "This candidate requires reconciliation. Inspect its durable target operation and deployment revision before retrying.",
    projectFrameDevelopmentDeploymentReconcile: "Reconcile outcome",
    projectFrameDevelopmentDeploymentReconciling: "Reconciling…",
    projectFrameDevelopmentDeploymentReconcileConfirm: "Reconcile durable target truth now? A matching committed activation will be adopted; otherwise an exactly identified uncertain candidate will be stopped and its Host resources cleaned. Effects without durable identity remain blocked for manual inspection.",
    projectFrameDevelopmentDeploymentReconciled: "Deployment outcome reconciled",
    projectFrameDevelopmentDeploymentReconciliationFailed: "Deployment still requires reconciliation",
    projectFrameDevelopmentDeploymentOpenProduction: "Open production route",
    projectFrameDeploymentSection: "Deployment",
    projectFrameDeploymentDescription: "Read-only target, execution, port, and proxy diagnostics reported by the host.",
    projectFrameRemoteTargetOperationsOnly:
      "Verified artifact deployment, revision recovery, and diagnostics use this selected target. Legacy image and Git deployment actions remain local-only and are hidden here.",
    projectFrameTargetOperations: "Target operations",
    projectFrameTargetOperationEmpty: "No durable target operations have been recorded for this project and target.",
    projectFrameTargetOperationShowDetails: "Show details",
    projectFrameTargetOperationHideDetails: "Hide details",
    projectFrameTargetOperationRevision: "Operation revision",
    projectFrameTargetOperationExecution: "Execution ID",
    projectFrameTargetOperationUpdatedAt: "Updated",
    projectFrameTargetOperationRequestDigest: "Request digest",
    projectFrameTargetOperationAuthorityDigest: "Authority digest",
    projectFrameTargetOperationOutput: "Receipt output",
    projectFrameTargetOperationDiagnostics: "Terminal diagnostics",
    projectFrameTargetOperationNoDiagnostics: "The terminal receipt contains no diagnostics.",
    projectFrameTargetOperationReceiptPending: "No terminal receipt is available yet. Refresh to observe the next durable state.",
    projectFrameTargetOperationShowingRecent: (shown, total) => `Showing the newest ${shown} of ${total} operations.`,
    projectFrameDeploymentExecutions: "Executions",
    projectFrameDeploymentPortLeases: "Port Leases",
    projectFrameDeploymentProxyRoutes: "Proxy Routes",
    projectFrameDeploymentEmpty: "No deployment records reported by the host.",
    projectFrameDeploymentReady: "Ready",
    projectFrameDeploymentExitCode: "Exit code",
    projectFrameDeploymentHistoryTitle: "Durable deployment history",
    projectFrameDeploymentHistoryDescription: "Host-owned revisions and job receipts survive restarts; recovery only replays secret references and other safe inputs.",
    projectFrameDeploymentActiveRevision: "Active revision",
    projectFrameDeploymentRecoveryRequired: "Recovery required",
    projectFrameDeploymentRecover: "Recover",
    projectFrameDeploymentRecovering: "Recovering…",
    projectFrameDeploymentRecoverConfirm: "Recover the active revision from its durable receipt? The existing route resources will be cleaned up first.",
    projectFrameDeploymentRecoveredTitle: "Deployment recovered",
    projectFrameDeploymentRecoveryFailedTitle: "Deployment recovery failed",
    projectFrameDeploymentRevisionHistory: "Revision history",
    projectFrameDeploymentRollback: "Roll back",
    projectFrameDeploymentRollingBack: "Rolling back…",
    projectFrameDeploymentRollbackConfirm: (revisionId) => `Activate deployment revision ${revisionId}? The current deployment will be stopped first.`,
    projectFrameDeploymentRolledBackTitle: "Deployment rolled back",
    projectFrameDeploymentRollbackFailedTitle: "Deployment rollback failed",
    projectFrameDeploymentNotRecoverable: "Manual rebuild required",
    projectFrameDeploymentNoActiveOnTarget: (targetId) => `No active revision is running on target ${targetId}.`,
    projectFrameDeploymentJobHistory: "Recent deployment jobs",
    projectFrameDeploymentCreatedAt: "Created",
    projectFrameDeploymentSourceCommit: "Source commit",
    projectFrameDeploymentBuildDigest: "Build descriptor",
    projectFrameDeploymentParentRevision: "Parent revision",
    projectFrameRouteExposure: "Route exposure",
    projectFrameRoutePrivate: "Host authenticated",
    projectFrameRoutePrivateBody: "Only authenticated Host clients can reach this service through the managed /p route.",
    projectFrameRoutePublic: "Public",
    projectFrameRoutePublicBody: "Allow unauthenticated requests through the configured virtual-host domain.",
    projectFrameRoutePublicWarning: "Public exposure bypasses Host authentication for this application route. Publish only services designed for the internet.",
    projectFrameDeployActionTitle: "Explicit Docker deployment",
    projectFrameDeployActionDescription: "Starts one prebuilt HTTP container using host-owned port and proxy resources.",
    projectFrameDeployInvalidTitle: "Deployment metadata is invalid",
    projectFrameDeploy: "Deploy",
    projectFrameDeploying: "Deploying…",
    projectFrameStopDeployment: "Stop deployment",
    projectFrameStoppingDeployment: "Stopping…",
    projectFrameDeployImage: "Image",
    projectFrameDeployContainerPort: "Container port",
    projectFrameDeployRouteId: "Route ID",
    projectFrameDeployPortName: "Port name",
    projectFrameDeployHealthPath: "Health path",
    projectFrameDeployContainerId: "Container ID",
    projectFrameDeployPullWarning: "This deployment may pull the image if it is missing. Only deploy trusted image references.",
    projectFrameDeployActiveHint: "An active route, port lease, or tracked container is present for this descriptor.",
    projectFrameDeployConfirm: (image) => `Deploy Docker image ${image}? This will lease a local port and start a container on this host.`,
    projectFrameStopDeploymentConfirm: (routeId) => `Stop the host-managed deployment for route ${routeId}? Its active durable revision will be deactivated.`,
    projectFrameDeploySuccessTitle: "Deployment started",
    projectFrameDeploySuccessBody: (routeId) => `Proxy route ${routeId} is registered.`,
    projectFrameDeployFailedTitle: "Deployment failed",
    projectFrameDeployFailedBody: "Docker runtime lab or host resource setup failed.",
    projectFrameStopDeploymentSuccessTitle: "Deployment stopped",
    projectFrameStopDeploymentPartialTitle: "Deployment cleanup partially failed",
    projectFrameBuildDeployTitle: "Build & Deploy",
    projectFrameBuildDeployDescription: "Clone the project workspace, build an image, then start it through the host deployment broker.",
    projectFrameBuildDeployInvalidTitle: "Build-deploy metadata is invalid",
    projectFrameBuildDeployButton: "Build & Deploy",
    projectFrameBuildDeployRunning: "Running…",
    projectFrameBuildDeployCancel: "Cancel job",
    projectFrameBuildDeployCancelConfirmTitle: "Cancel build job?",
    projectFrameBuildDeployCancelConfirmBody: "The host will stop after the current safe checkpoint and roll back any acquired deploy resources.",
    projectFrameBuildDeployStrategy: "Strategy",
    projectFrameBuildDeploySource: "Source",
    projectFrameBuildDeployRef: "Ref",
    projectFrameBuildDeployEnv: "Runtime env",
    projectFrameBuildDeploySecretRef: "secret ref",
    projectFrameBuildDeployPlainEnv: "plain",
    projectFrameBuildDeployMounts: "Runtime mounts",
    projectFrameBuildDeployApproveMount: "I've verified this path is correct",
    projectFrameBuildDeployApproveRisk: "I approve read-write access",
    projectFrameBuildDeployHighRiskBadge: "RW high risk",
    projectFrameBuildDeployJob: "Job",
    projectFrameBuildDeployTracker: "Build thread",
    projectFrameBuildDeployCopyLogs: "Copy logs",
    projectFrameBuildDeployNoLogs: "Waiting for job events…",
    projectFrameBuildDeployResultReady: "Deployment ready",
    projectFrameBuildDeployResultFailed: "Deployment failed",
    projectFrameBuildDeployResultCancelled: "Deployment cancelled",
    projectFrameBuildDeployNewBuild: "New Build",
    projectFrameBuildDeployRestart: "Restart",
    projectFrameBuildDeployEditConfig: "Edit Config",
    projectFrameBuildDeployConfirm: (routeId) => `Build and deploy route ${routeId}? This may run Docker builds on the host.`,
    projectFrameBuildDeployStartedTitle: "Build job started",
    projectFrameBuildDeployStartedBody: (jobId) => `Tracking job ${jobId}.`,
    projectFrameBuildDeployFailedTitle: "Build-deploy failed",
    projectFrameBuildDeployCancelTitle: "Cancel requested",
    projectFrameBuildDeployReady: (url) => `Ready at ${url}`,
    projectFrameStatusReady: "Ready",
    projectFrameStatusNotReady: "Not ready",
    projectFrameShowLogs: "Show logs",
    projectFrameHideLogs: "Hide logs",
    projectFrameLogsLoading: "Loading logs…",
    projectFrameNoLogs: "No bounded logs returned for this execution.",
    projectFrameCopyAddress: "Copy address",
    projectFrameCopyUrl: "Copy URL",
    projectFrameOpenUrl: "Open URL",
    projectFramePublicUrl: "Public URL",
    projectFrameIframeUrl: "Iframe URL",

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
    accessProjectResources: "Project access",
    accessProjectResourcesBody: "Grant every project or enter exact project ids separated by commas.",
    accessAllProjects: "All projects",
    accessProjectIdsPlaceholder: "project-a, project-b",
    accessTargetResources: "Execution targets",
    accessTargetResourcesBody: "Grant every target or enter exact target ids separated by commas.",
    accessAllTargets: "All targets",
    accessTargetIdsPlaceholder: "local, preview-server",
    accessProjectResource: (id) => `Project · ${id}`,
    accessTargetResource: (id) => `Target · ${id}`,
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
    accessScopeObserveBody: "Read Host, project, deployment, and diagnostic state.",
    accessScopeProjectOperate: "Operate projects",
    accessScopeProjectOperateBody: "Start, stop, open, and fork project sessions.",
    accessScopeDeploy: "Deploy",
    accessScopeDeployBody: "Build, deploy, recover, and manage runtime routes.",
    accessScopeDevelopPropose: "Propose changes",
    accessScopeDevelopProposeBody: "Draft and inspect controlled project change sets.",
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
      "Keys stay on this machine, encrypted with your platform key. Plurora never transmits raw keys — projects request them through audited capability calls.",
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
      "The host injects the raw value into outbound requests on the project's behalf.",
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
      "Plurora encrypts the value with your platform key and never sends raw keys to any project.",
    apiProvider: "Provider",
    apiSecretName: "Secret name",
    apiSecretNameHelper: "Convention: PROVIDER_API_KEY (uppercase, underscores)",
    apiValue: "Value",
    apiValueHelper: "The raw key never leaves this machine.",
    apiScope: "Scope",
    apiScopePlatform: "Platform-wide",
    apiScopeProject: "Project-only (configure on Home)",
    cancel: "Cancel",
    apiSaveKey: "Save key",

    packagesEyebrowLoading: "Installed packages · loading…",
    packagesEyebrowCount: (count) => `Installed packages · ${count} packages`,
    packagesTitle: "Workshop inventory",
    packagesDescription:
      "Projects, Plurora packages, and dependencies installed in this workshop. Refresh checks upstream sources.",
    packagesFilterPlaceholder: "Filter packages…",
    packagesFilterAll: "All",
    packagesFilterProjects: "Projects",
    packagesFilterPlurora: "Plurora",
    packagesFilterThirdParty: "Third-party",
    packagesRefreshing: "Refreshing inventory…",
    packagesRefresh: "Refresh",
    packagesLoadErrorTitle: "Couldn't load packages",
    packagesLoadErrorBody: "Package inventory is unavailable. Try again from the local UI.",
    packagesEmptyTitle: "No packages installed yet",
    packagesNoMatchTitle: "No packages match this filter",
    packagesEmptyBody: "Install a project from Home or run plurora install on the CLI.",
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
    profilesSwitchHint: "Switching restarts the host. Project state is preserved.",
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
    storageAreaProjectData: "Project data",
    storageAreaProjectDataDesc: "Project metadata, checkpoints, package state, and run records.",
    storageAreaPackageStore: "Package store",
    storageAreaPackageStoreDesc: "Installed package sources and lockfile-managed revisions.",
    storageAreaProfiles: "Profiles",
    storageAreaProfilesDesc: "Host profiles passed to plurora host serve --profile.",
    storageAreaSecrets: "Secrets",
    storageAreaSecretsDesc: "Encrypted platform and project secret stores. Raw values are never shown here.",
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
      "Plurora is a kernel and a contract. The kernel hosts your projects in sandboxes. The contract lets any project — Plurora, community, or self-built — participate as a first-class citizen.",
    aboutPara2:
      "It runs on your machine, with your keys, your files, your network. There is no SaaS account, no central registry, no telemetry. Projects you install live in the local platform data directory and stay there until you remove them.",
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
    topbarProject: (projectId) => `项目 / ${projectId}`,
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
    homeProjectPopupBlockedTitle: "项目标签页被拦截",
    homeProjectPopupBlockedBody: "请允许此站点打开弹出式窗口，然后从控制台重新打开项目界面。",

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
    installCompleteBody: (count, projectId) => `已安装 ${count} 个包${projectId ? ` · 项目 ${projectId}` : ""}`,
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
    installProjectSection: "项目",
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
    installActivityRegisteredProject: (projectId) => `已注册项目 ${projectId}`,
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

    failureProjectFallback: "项目",
    failureContentLabel: (projectName) => `${projectName} 失败详情`,
    failureEyebrow: (projectName) => `失败 — ${projectName.toUpperCase()}`,
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
    failureRestartProject: "重启项目",

    projectFrameStartFailedTitle: "启动项目失败",
    projectFrameStartFailedBody: "无法启动项目框架。请检查本地主机后重试。",
    projectFrameMountFailedTitle: "项目界面挂载失败",
    projectFrameMountFailedBody: "项目已经运行，但浏览器界面未能加载。请检查本地主机和 surface bundle。",
    projectFrameStopped: (title) => `已停止 ${title}`,
    projectFrameStopFailedTitle: "停止失败",
    projectFrameStopFailedBody: "无法停止该项目。请检查本地主机后重试。",
    projectFrameBackHome: "返回首页",
    projectFrameAuditLog: "审计日志",
    projectFrameAuditLogUnavailable: "审计日志暂未接线",
    projectFrameStopProject: "停止项目",
    projectFrameStop: "停止",
    projectFrameMore: "更多",
    projectFrameMoreUnavailable: "更多项目操作暂未接线",
    projectFrameState: (state) => state.toUpperCase(),
    projectFrameLoadingSurface: "正在加载项目界面…",
    projectFrameStoppedTitle: "项目已停止",
    projectFrameStoppedBody: "可以关闭这个标签页。需要继续时，从首页重新打开项目。",
    projectFrameConsoleTitle: "项目控制台",
    projectFrameConsoleBody: "项目正在由当前 Plurora 标签页托管。项目自己的界面会在独立项目标签页打开，这里保留返回、停止和审计控制。",
    projectFrameOpenProjectTab: "打开项目界面",
    projectFrameProjectTabBlockedTitle: "项目标签页被浏览器拦截",
    projectFrameProjectTabBlockedBody: "请允许本站弹出窗口，然后从控制台重新打开项目界面。",
    projectFrameRefresh: "刷新",
    projectFrameRefreshing: "正在刷新…",
    projectFrameRefreshDiagnostics: "刷新诊断",
    projectFrameUpdateProject: "更新项目",
    projectFrameUpdating: "正在更新…",
    projectFrameUpdateCompleteTitle: "项目已更新",
    projectFrameUpdateCompleteBody: (count) => `已更新 ${count} 个包。`,
    projectFrameUpdateCurrentTitle: "项目已是最新",
    projectFrameUpdateCurrentBody: "没有可用的包更新。",
    projectFrameUpdateFailedTitle: "更新失败",
    projectFrameStopConfirm: "停止会终止当前运行会话和项目包。项目界面里尚未保存的工作可能丢失。确定停止这个项目吗？",
    projectFrameDiagnosticsLoading: "正在加载诊断…",
    projectFrameStatus: "状态",
    projectFramePackages: "包",
    projectFrameUpdates: "更新",
    projectFrameActivity: "活动",
    projectFramePackageHealth: (healthy, total) => `${healthy}/${total} 健康`,
    projectFrameRecentEvents: (count) => `${count} 条近期事件`,
    projectFrameUpdatesAvailable: (count) => `${count} 个更新可用`,
    projectFrameUpdatesCurrent: "已是最新",
    projectFrameUpdateUnavailable: "更新检查不可用",
    projectFrameDeploymentMetric: "部署",
    projectFrameDeploymentSummary: (targets, executions, running, ports, proxies) =>
      `${targets} 个目标 · ${running}/${executions} 个执行运行中 · ${ports} 个端口 · ${proxies} 个代理`,
    projectFrameProjectId: "项目 ID",
    projectFrameProjectType: "类型",
    projectFrameSession: "会话",
    projectFrameActiveSession: "运行中",
    projectFrameStorage: "存储",
    projectFrameInterfaceSection: "项目界面",
    projectFrameInterfaceDescription: "独立项目标签页，以及 iframe 主机解析到的 surface bundle。",
    projectFrameEntrySurface: "入口 surface",
    projectFrameBundleUrl: "Bundle URL",
    projectFrameBundleFingerprint: "指纹",
    projectFrameBundleUnavailable: "无法解析 bundle",
    projectFrameLastResolved: "最近解析",
    projectFrameUpdatesSection: "更新",
    projectFrameUpdatesDescription: "通过 capability.invoke 调用 plurora/install-lab 能力进行检查。",
    projectFrameNoUpdateRecords: "该项目没有返回更新记录。",
    projectFramePackagesSection: "包健康",
    projectFramePackagesDescription: "运行时包状态、计数，以及可用的脱敏失败/日志摘要。",
    projectFrameNoPackages: "主机未报告项目包。",
    projectFramePackageCounts: (capabilities, hooks) => `${capabilities} 个能力 · ${hooks} 个 hook`,
    projectFrameActivitySection: "近期活动",
    projectFrameActivityDescription: "尽力展示运行中项目会话的事件尾部。",
    projectFrameNoEvents: "该会话暂无近期事件。",
    projectFrameNoSession: "尚无运行中的会话。",
    projectFrameDiagnosticsWarnings: "诊断警告",
    projectFrameDevelopmentSection: "开发",
    projectFrameDevelopmentDescription: "草拟内容寻址 ChangeSet，显式审批，在隔离 scratch 中验证，并且只提升 Host 拥有的工作树。",
    projectFrameDevelopmentLoadFailed: "无法读取开发历史",
    projectFrameDevelopmentGoal: "目标",
    projectFrameDevelopmentGoalPlaceholder: "描述这次源码变更要实现什么",
    projectFrameDevelopmentTarget: "相对文件路径",
    projectFrameDevelopmentOperation: "操作",
    projectFrameDevelopmentWrite: "写入文件",
    projectFrameDevelopmentDelete: "删除文件",
    projectFrameDevelopmentExecutable: "可执行文件",
    projectFrameDevelopmentDockerBuild: "使用 Docker 构建验证",
    projectFrameDevelopmentAllowNetwork: "允许 Docker 构建出网",
    projectFrameDevelopmentDockerfile: "Dockerfile 路径",
    projectFrameDevelopmentContent: "UTF-8 源码内容",
    projectFrameDevelopmentSafetyHint: "草稿把源码按摘要存为对象，journal 不保存源码明文；Docker scratch 默认断网并删除验证镜像。managed external 可提升，native 只验证；linked-local 必须先导入为 managed 才能由 Host 验证。",
    projectFrameDevelopmentDrafting: "正在草拟…",
    projectFrameDevelopmentDraft: "草拟 ChangeSet",
    projectFrameDevelopmentHistory: "变更历史",
    projectFrameDevelopmentHistoryDescription: "持久保存 intent、策略、审批、验证、commit 与 effect receipt。",
    projectFrameDevelopmentRefresh: "刷新",
    projectFrameDevelopmentEmpty: "该项目尚无开发 ChangeSet。",
    projectFrameDevelopmentDrafted: "开发 ChangeSet 已草拟",
    projectFrameDevelopmentDraftFailed: "无法草拟开发 ChangeSet",
    projectFrameDevelopmentApproveConfirm: "批准这个精确 ChangeSet 及其列出的文件系统、Docker 与网络权限？",
    projectFrameDevelopmentDecisionFailed: "无法记录审批决定",
    projectFrameDevelopmentExecuteConfirm: "现在在 Host 拥有的 scratch 工作区执行已批准的 ChangeSet？",
    projectFrameDevelopmentExecutionStarted: "开发执行已开始",
    projectFrameDevelopmentExecutionAlreadyActive: "开发执行已在进行，状态已刷新",
    projectFrameDevelopmentExecutionFailed: "无法启动开发执行",
    projectFrameDevelopmentRecovered: "开发 promotion 已完成对账",
    projectFrameDevelopmentRecoveryFailed: "开发 promotion 仍需要恢复",
    projectFrameDevelopmentExportFailed: "无法导出 patch bundle",
    projectFrameDevelopmentLinkedHint: "linked-local 项目仅允许提案。请先导入 managed 副本再进行 Host 验证；Plurora 永不写入链接的用户源目录。",
    projectFrameDevelopmentReviewOperations: "精确操作",
    projectFrameDevelopmentReviewVerification: "验证方式",
    projectFrameDevelopmentReviewAuthority: "所需权限",
    projectFrameDevelopmentReviewEffects: "预期效果",
    projectFrameDevelopmentApprovalRecord: "已记录的审批决定",
    projectFrameDevelopmentRecoveryTarget: "恢复对账对象",
    projectFrameDevelopmentVerificationStatic: "静态验证（不执行代码）",
    projectFrameDevelopmentVerificationDocker: "Docker 构建验证",
    projectFrameDevelopmentStatusDrafted: "草稿",
    projectFrameDevelopmentStatusApproved: "已批准",
    projectFrameDevelopmentStatusRejected: "已拒绝",
    projectFrameDevelopmentStatusStaging: "正在暂存",
    projectFrameDevelopmentStatusVerifying: "正在验证",
    projectFrameDevelopmentStatusPromoting: "正在提升",
    projectFrameDevelopmentStatusVerified: "已验证",
    projectFrameDevelopmentStatusCommitted: "已提交",
    projectFrameDevelopmentStatusRecoveryRequired: "需要恢复",
    projectFrameDevelopmentStatusFailed: "失败",
    projectFrameDevelopmentExport: "导出 bundle",
    projectFrameDevelopmentReject: "拒绝",
    projectFrameDevelopmentApprove: "批准",
    projectFrameDevelopmentExecute: "执行",
    projectFrameDevelopmentRecover: "对账恢复",
    projectFrameDevelopmentDeploymentTitle: "已验证部署",
    projectFrameDevelopmentDeploymentDescription: "在选定目标上构建已提交的 CAS 上下文，检查其私有预览，再批准并激活这个精确候选版本。",
    projectFrameDevelopmentDeploymentStatusPreparing: "正在准备",
    projectFrameDevelopmentDeploymentStatusBuilding: "正在构建",
    projectFrameDevelopmentDeploymentStatusPreviewing: "正在生成预览",
    projectFrameDevelopmentDeploymentStatusActivating: "正在激活",
    projectFrameDevelopmentDeploymentInvalidConfig: "容器端口、端口名和路由 ID 必须有效。",
    projectFrameDevelopmentDeploymentPreview: "创建私有预览",
    projectFrameDevelopmentDeploymentPreviewing: "正在准备预览…",
    projectFrameDevelopmentDeploymentPreviewConfirm: (targetId, routeId) => `在目标 ${targetId} 上构建并运行这个已验证变更，作为路由 ${routeId} 的私有预览？`,
    projectFrameDevelopmentDeploymentPreviewStarted: "私有部署预览已启动",
    projectFrameDevelopmentDeploymentPreviewFailed: "无法创建部署预览",
    projectFrameDevelopmentDeploymentSourceTree: "已验证源码树",
    projectFrameDevelopmentDeploymentBuildContext: "CAS 构建上下文",
    projectFrameDevelopmentDeploymentCandidate: "私有候选版本",
    projectFrameDevelopmentDeploymentOpenPreview: "打开私有预览",
    projectFrameDevelopmentDeploymentApproveConfirm: (targetId, routeId, exposure) => `批准这里展示的精确候选版本与证据，用于目标 ${targetId} 的路由 ${routeId}，暴露范围为“${exposure}”？`,
    projectFrameDevelopmentDeploymentApproved: "部署候选版本已批准",
    projectFrameDevelopmentDeploymentRejected: "部署候选版本已拒绝",
    projectFrameDevelopmentDeploymentActivate: "激活",
    projectFrameDevelopmentDeploymentActivating: "正在激活…",
    projectFrameDevelopmentDeploymentActivateConfirm: (targetId, routeId, exposure) => `将目标 ${targetId} 的生产路由 ${routeId} 原子切换到这个已批准候选版本，暴露范围为“${exposure}”？`,
    projectFrameDevelopmentDeploymentActivated: "已验证部署已激活",
    projectFrameDevelopmentDeploymentActivationFailed: "无法激活已验证部署",
    projectFrameDevelopmentDeploymentRecoveryHint: "该候选版本需要对账。重试前请先检查它的持久化目标操作与部署修订。",
    projectFrameDevelopmentDeploymentReconcile: "对账处理",
    projectFrameDevelopmentDeploymentReconciling: "正在对账…",
    projectFrameDevelopmentDeploymentReconcileConfirm: "现在根据持久化目标事实进行对账？若存在完全匹配且已提交的激活修订，将直接采纳；否则会停止能够被精确识别的不确定候选版本并清理其 Host 资源。缺少持久身份的效果仍会阻断并等待人工核验。",
    projectFrameDevelopmentDeploymentReconciled: "部署结果已完成对账",
    projectFrameDevelopmentDeploymentReconciliationFailed: "部署仍需要对账",
    projectFrameDevelopmentDeploymentOpenProduction: "打开生产路由",
    projectFrameDeploymentSection: "部署",
    projectFrameDeploymentDescription: "主机报告的只读目标、执行、端口和代理诊断。",
    projectFrameRemoteTargetOperationsOnly:
      "已验证制品部署、修订恢复和诊断会使用当前选定目标；旧版镜像与 Git 部署仍仅限本机，因此在此隐藏。",
    projectFrameTargetOperations: "目标操作",
    projectFrameTargetOperationEmpty: "该项目与目标尚无持久化操作记录。",
    projectFrameTargetOperationShowDetails: "显示详情",
    projectFrameTargetOperationHideDetails: "隐藏详情",
    projectFrameTargetOperationRevision: "操作修订",
    projectFrameTargetOperationExecution: "执行 ID",
    projectFrameTargetOperationUpdatedAt: "更新时间",
    projectFrameTargetOperationRequestDigest: "请求摘要",
    projectFrameTargetOperationAuthorityDigest: "授权摘要",
    projectFrameTargetOperationOutput: "回执输出",
    projectFrameTargetOperationDiagnostics: "终态诊断",
    projectFrameTargetOperationNoDiagnostics: "终态回执没有诊断信息。",
    projectFrameTargetOperationReceiptPending: "当前还没有终态回执；刷新后可查看下一项持久状态。",
    projectFrameTargetOperationShowingRecent: (shown, total) => `显示最新 ${shown} 项，共 ${total} 项操作。`,
    projectFrameDeploymentExecutions: "执行",
    projectFrameDeploymentPortLeases: "端口租约",
    projectFrameDeploymentProxyRoutes: "代理路由",
    projectFrameDeploymentEmpty: "主机未报告部署记录。",
    projectFrameDeploymentReady: "就绪",
    projectFrameDeploymentExitCode: "退出码",
    projectFrameDeploymentHistoryTitle: "持久化部署历史",
    projectFrameDeploymentHistoryDescription: "主机拥有的修订与任务回执会跨重启保留；恢复只会重放密钥引用等安全输入。",
    projectFrameDeploymentActiveRevision: "活动修订",
    projectFrameDeploymentRecoveryRequired: "需要恢复",
    projectFrameDeploymentRecover: "恢复",
    projectFrameDeploymentRecovering: "正在恢复…",
    projectFrameDeploymentRecoverConfirm: "根据持久化回执恢复活动修订？主机会先清理现有路由资源。",
    projectFrameDeploymentRecoveredTitle: "部署已恢复",
    projectFrameDeploymentRecoveryFailedTitle: "部署恢复失败",
    projectFrameDeploymentRevisionHistory: "修订历史",
    projectFrameDeploymentRollback: "回滚",
    projectFrameDeploymentRollingBack: "正在回滚…",
    projectFrameDeploymentRollbackConfirm: (revisionId) => `激活部署修订 ${revisionId}？当前部署会先被停止。`,
    projectFrameDeploymentRolledBackTitle: "部署已回滚",
    projectFrameDeploymentRollbackFailedTitle: "部署回滚失败",
    projectFrameDeploymentNotRecoverable: "需要手动重新构建",
    projectFrameDeploymentNoActiveOnTarget: (targetId) => `目标 ${targetId} 当前没有活动修订。`,
    projectFrameDeploymentJobHistory: "近期部署任务",
    projectFrameDeploymentCreatedAt: "创建时间",
    projectFrameDeploymentSourceCommit: "来源提交",
    projectFrameDeploymentBuildDigest: "构建描述摘要",
    projectFrameDeploymentParentRevision: "父修订",
    projectFrameRouteExposure: "路由暴露范围",
    projectFrameRoutePrivate: "需要 Host 认证",
    projectFrameRoutePrivateBody: "只有已认证的 Host 客户端可通过托管的 /p 路由访问此服务。",
    projectFrameRoutePublic: "公开访问",
    projectFrameRoutePublicBody: "允许未经认证的请求通过已配置的虚拟主机域名访问。",
    projectFrameRoutePublicWarning: "公开访问会绕过该应用路由的 Host 认证。请只发布已按互联网暴露场景设计的服务。",
    projectFrameDeployActionTitle: "显式 Docker 部署",
    projectFrameDeployActionDescription: "使用主机拥有的端口和代理资源启动一个预构建 HTTP 容器。",
    projectFrameDeployInvalidTitle: "部署元数据无效",
    projectFrameDeploy: "部署",
    projectFrameDeploying: "正在部署…",
    projectFrameStopDeployment: "停止部署",
    projectFrameStoppingDeployment: "正在停止…",
    projectFrameDeployImage: "镜像",
    projectFrameDeployContainerPort: "容器端口",
    projectFrameDeployRouteId: "路由 ID",
    projectFrameDeployPortName: "端口名",
    projectFrameDeployHealthPath: "健康路径",
    projectFrameDeployContainerId: "容器 ID",
    projectFrameDeployPullWarning: "如果缺少镜像，此部署可能会拉取镜像。请只部署可信镜像引用。",
    projectFrameDeployActiveHint: "该描述符已有活动路由、端口租约或已跟踪容器。",
    projectFrameDeployConfirm: (image) => `部署 Docker 镜像 ${image}？这将在此主机上租用本地端口并启动容器。`,
    projectFrameStopDeploymentConfirm: (routeId) => `停止路由 ${routeId} 的主机托管部署？其活动持久化修订也会被停用。`,
    projectFrameDeploySuccessTitle: "部署已启动",
    projectFrameDeploySuccessBody: (routeId) => `代理路由 ${routeId} 已注册。`,
    projectFrameDeployFailedTitle: "部署失败",
    projectFrameDeployFailedBody: "Docker runtime lab 或主机资源设置失败。",
    projectFrameStopDeploymentSuccessTitle: "部署已停止",
    projectFrameStopDeploymentPartialTitle: "部署清理部分失败",
    projectFrameBuildDeployTitle: "构建并部署",
    projectFrameBuildDeployDescription: "克隆项目工作区、构建镜像，然后通过主机部署代理启动。",
    projectFrameBuildDeployInvalidTitle: "构建部署元数据无效",
    projectFrameBuildDeployButton: "构建并部署",
    projectFrameBuildDeployRunning: "运行中…",
    projectFrameBuildDeployCancel: "取消任务",
    projectFrameBuildDeployCancelConfirmTitle: "取消构建任务？",
    projectFrameBuildDeployCancelConfirmBody: "主机会在下一个安全检查点停止，并回滚已获取的部署资源。",
    projectFrameBuildDeployStrategy: "策略",
    projectFrameBuildDeploySource: "来源",
    projectFrameBuildDeployRef: "引用",
    projectFrameBuildDeployEnv: "运行时环境变量",
    projectFrameBuildDeploySecretRef: "密钥引用",
    projectFrameBuildDeployPlainEnv: "明文",
    projectFrameBuildDeployMounts: "运行时挂载",
    projectFrameBuildDeployApproveMount: "我已确认此路径正确",
    projectFrameBuildDeployApproveRisk: "我批准读写访问",
    projectFrameBuildDeployHighRiskBadge: "读写高风险",
    projectFrameBuildDeployJob: "任务",
    projectFrameBuildDeployTracker: "构建线程",
    projectFrameBuildDeployCopyLogs: "复制日志",
    projectFrameBuildDeployNoLogs: "等待任务事件…",
    projectFrameBuildDeployResultReady: "部署已就绪",
    projectFrameBuildDeployResultFailed: "部署失败",
    projectFrameBuildDeployResultCancelled: "部署已取消",
    projectFrameBuildDeployNewBuild: "新构建",
    projectFrameBuildDeployRestart: "重新启动",
    projectFrameBuildDeployEditConfig: "编辑配置",
    projectFrameBuildDeployConfirm: (routeId) => `构建并部署路由 ${routeId}？这可能会在主机上运行 Docker 构建。`,
    projectFrameBuildDeployStartedTitle: "构建任务已启动",
    projectFrameBuildDeployStartedBody: (jobId) => `正在跟踪任务 ${jobId}。`,
    projectFrameBuildDeployFailedTitle: "构建部署失败",
    projectFrameBuildDeployCancelTitle: "已请求取消",
    projectFrameBuildDeployReady: (url) => `已就绪：${url}`,
    projectFrameStatusReady: "已就绪",
    projectFrameStatusNotReady: "未就绪",
    projectFrameShowLogs: "显示日志",
    projectFrameHideLogs: "隐藏日志",
    projectFrameLogsLoading: "正在加载日志…",
    projectFrameNoLogs: "该执行没有返回限定日志。",
    projectFrameCopyAddress: "复制地址",
    projectFrameCopyUrl: "复制 URL",
    projectFrameOpenUrl: "打开 URL",
    projectFramePublicUrl: "公共 URL",
    projectFrameIframeUrl: "Iframe URL",

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
    accessProjectResources: "项目访问范围",
    accessProjectResourcesBody: "可授权全部项目，或输入以英文逗号分隔的准确项目 ID。",
    accessAllProjects: "全部项目",
    accessProjectIdsPlaceholder: "project-a, project-b",
    accessTargetResources: "执行目标范围",
    accessTargetResourcesBody: "可授权全部 target，或输入以英文逗号分隔的准确 target ID。",
    accessAllTargets: "全部 target",
    accessTargetIdsPlaceholder: "local, preview-server",
    accessProjectResource: (id) => `项目 · ${id}`,
    accessTargetResource: (id) => `Target · ${id}`,
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
    accessScopeProjectOperate: "操作项目",
    accessScopeProjectOperateBody: "启动、停止、打开和分叉项目会话。",
    accessScopeDeploy: "部署",
    accessScopeDeployBody: "构建、部署、恢复并管理运行时路由。",
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
    apiScopeProject: "仅项目（在首页配置）",
    cancel: "取消",
    apiSaveKey: "保存密钥",

    packagesEyebrowLoading: "已安装包 · 加载中…",
    packagesEyebrowCount: (count) => `已安装包 · ${count} 个包`,
    packagesTitle: "工作台清单",
    packagesDescription: "此工作台中安装的项目、官方包和依赖。刷新会检查上游来源。",
    packagesFilterPlaceholder: "筛选包…",
    packagesFilterAll: "全部",
    packagesFilterProjects: "项目",
    packagesFilterPlurora: "官方",
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
    storageAreaProjectData: "项目数据",
    storageAreaProjectDataDesc: "项目元数据、检查点、包状态和运行记录。",
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
      "Plurora 是一个内核，也是一份契约。内核在沙盒中托管你的项目；契约让任何项目——官方、社区或自建——都能成为一等公民。",
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
