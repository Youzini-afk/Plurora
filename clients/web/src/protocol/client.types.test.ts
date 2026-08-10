import type {
  AcquisitionRecord as GeneratedAcquisitionRecord,
  InstallationCreateRequest as GeneratedInstallationCreateRequest,
  InstallationDiff as GeneratedInstallationDiff,
  InstallationMutationResult as GeneratedInstallationMutationResult,
  InstallationRecord as GeneratedInstallationRecord,
  InstallationRemoveRequest as GeneratedInstallationRemoveRequest,
  InstallationRollbackPointer as GeneratedInstallationRollbackPointer,
  InstallationSecretPolicy as GeneratedInstallationSecretPolicy,
  InstallationStateAction as GeneratedInstallationStateAction,
  InstallationUpdateRequest as GeneratedInstallationUpdateRequest,
  InstallationView as GeneratedInstallationView,
  StateAction as GeneratedStateAction,
  StateBindingRecord as GeneratedStateBindingRecord,
  WorkId as GeneratedWorkId,
} from "../../../../sdk/typescript/contract-sdk/src/types";
import type {
  AcquisitionRecord,
  InstallationCreateRequest,
  InstallationDiff,
  InstallationMutationResult,
  InstallationRecord,
  InstallationRemoveRequest,
  InstallationRollbackPointer,
  InstallationSecretPolicy,
  InstallationStateAction,
  InstallationUpdateRequest,
  InstallationView,
  StateAction,
  StateBindingRecord,
  WorkId,
} from "./client";

type Assert<T extends true> = T;
type Extends<From, To> = [From] extends [To] ? true : false;

type WebToGenerated = [
  Assert<Extends<AcquisitionRecord, GeneratedAcquisitionRecord>>,
  Assert<Extends<InstallationCreateRequest, GeneratedInstallationCreateRequest>>,
  Assert<Extends<InstallationDiff, GeneratedInstallationDiff>>,
  Assert<Extends<InstallationMutationResult, GeneratedInstallationMutationResult>>,
  Assert<Extends<InstallationRecord, GeneratedInstallationRecord>>,
  Assert<Extends<InstallationRemoveRequest, GeneratedInstallationRemoveRequest>>,
  Assert<Extends<InstallationRollbackPointer, GeneratedInstallationRollbackPointer>>,
  Assert<Extends<InstallationSecretPolicy, GeneratedInstallationSecretPolicy>>,
  Assert<Extends<InstallationStateAction, GeneratedInstallationStateAction>>,
  Assert<Extends<InstallationUpdateRequest, GeneratedInstallationUpdateRequest>>,
  Assert<Extends<InstallationView, GeneratedInstallationView>>,
  Assert<Extends<StateAction, GeneratedStateAction>>,
  Assert<Extends<StateBindingRecord, GeneratedStateBindingRecord>>,
  Assert<Extends<WorkId, GeneratedWorkId>>,
];

type GeneratedToWeb = [
  Assert<Extends<GeneratedAcquisitionRecord, AcquisitionRecord>>,
  Assert<Extends<GeneratedInstallationCreateRequest, InstallationCreateRequest>>,
  Assert<Extends<GeneratedInstallationDiff, InstallationDiff>>,
  Assert<Extends<GeneratedInstallationMutationResult, InstallationMutationResult>>,
  Assert<Extends<GeneratedInstallationRecord, InstallationRecord>>,
  Assert<Extends<GeneratedInstallationRemoveRequest, InstallationRemoveRequest>>,
  Assert<Extends<GeneratedInstallationRollbackPointer, InstallationRollbackPointer>>,
  Assert<Extends<GeneratedInstallationSecretPolicy, InstallationSecretPolicy>>,
  Assert<Extends<GeneratedInstallationStateAction, InstallationStateAction>>,
  Assert<Extends<GeneratedInstallationUpdateRequest, InstallationUpdateRequest>>,
  Assert<Extends<GeneratedInstallationView, InstallationView>>,
  Assert<Extends<GeneratedStateAction, StateAction>>,
  Assert<Extends<GeneratedStateBindingRecord, StateBindingRecord>>,
  Assert<Extends<GeneratedWorkId, WorkId>>,
];

// Keep these aliases evaluated by the compiler even though this test has no runtime assertions.
export type InstallationDtoWebToGenerated = WebToGenerated;
export type InstallationDtoGeneratedToWeb = GeneratedToWeb;
