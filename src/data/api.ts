// Typed calls to the core. Each function is one command; argument names follow the Rust
// parameters (Tauri maps camelCase here to snake_case there).

import { useMemo } from "react";

import { useBackend } from "./backend";
import type { ClaudeSignals } from "./generated/ClaudeSignals";
import type { Backend } from "./backend";
import type { Added } from "./generated/Added";
import type { Area } from "./generated/Area";
import type { AreaInput } from "./generated/AreaInput";
import type { CalendarDay } from "./generated/CalendarDay";
import type { CaptureKind } from "./generated/CaptureKind";
import type { CheckReport } from "./generated/CheckReport";
import type { CloseInput } from "./generated/CloseInput";
import type { Collection } from "./generated/Collection";
import type { CollectionInput } from "./generated/CollectionInput";
import type { CtfEvent } from "./generated/CtfEvent";
import type { CtfInput } from "./generated/CtfInput";
import type { DayReview } from "./generated/DayReview";
import type { EventInput } from "./generated/EventInput";
import type { FileCard } from "./generated/FileCard";
import type { FileCounts } from "./generated/FileCounts";
import type { FileEdit } from "./generated/FileEdit";
import type { FileQuery } from "./generated/FileQuery";
import type { Info } from "./generated/Info";
import type { ModelInfo } from "./generated/ModelInfo";
import type { LearningCard } from "./generated/LearningCard";
import type { LearningInput } from "./generated/LearningInput";
import type { LearningOverview } from "./generated/LearningOverview";
import type { LearningQuery } from "./generated/LearningQuery";
import type { LedgerEdit } from "./generated/LedgerEdit";
import type { LedgerInput } from "./generated/LedgerInput";
import type { LedgerItem } from "./generated/LedgerItem";
import type { LearningStatus } from "./generated/LearningStatus";
import type { LinkTarget } from "./generated/LinkTarget";
import type { ProjectDetail } from "./generated/ProjectDetail";
import type { ProjectInput } from "./generated/ProjectInput";
import type { ProjectStatus } from "./generated/ProjectStatus";
import type { ProjectSummary } from "./generated/ProjectSummary";
import type { Reading } from "./generated/Reading";
import type { ReminderInput } from "./generated/ReminderInput";
import type { ReminderItem } from "./generated/ReminderItem";
import type { Routine } from "./generated/Routine";
import type { RoutineInput } from "./generated/RoutineInput";
import type { Saved } from "./generated/Saved";
import type { SearchHit } from "./generated/SearchHit";
import type { Session } from "./generated/Session";
import type { SessionInput } from "./generated/SessionInput";
import type { Settings } from "./generated/Settings";
import type { SettingsPatch } from "./generated/SettingsPatch";
import type { Slot } from "./generated/Slot";
import type { MessageState } from "./generated/MessageState";
import type { Prd } from "./generated/Prd";
import type { PrdSection } from "./generated/PrdSection";
import type { PrdView } from "./generated/PrdView";
import type { ApproveRoadmap } from "./generated/ApproveRoadmap";
import type { ClaudeAccess } from "./generated/ClaudeAccess";
import type { FolderCheck } from "./generated/FolderCheck";
import type { PlanCheckpoint } from "./generated/PlanCheckpoint";
import type { RoadmapView } from "./generated/RoadmapView";
import type { StackBody } from "./generated/StackBody";
import type { StackMode } from "./generated/StackMode";
import type { StudioStack } from "./generated/StudioStack";
import type { ReportInput } from "./generated/ReportInput";
import type { StudioLink } from "./generated/StudioLink";
import type { StudioReport } from "./generated/StudioReport";
import type { StudioRequest } from "./generated/StudioRequest";
import type { StudioTaskStatus } from "./generated/StudioTaskStatus";
import type { Problem } from "./generated/Problem";
import type { StudioMessage } from "./generated/StudioMessage";
import type { StudioProject } from "./generated/StudioProject";
import type { StudioStage } from "./generated/StudioStage";
import type { TaskCard } from "./generated/TaskCard";
import type { TaskCounts } from "./generated/TaskCounts";
import type { TaskDetail } from "./generated/TaskDetail";
import type { TaskInput } from "./generated/TaskInput";
import type { TaskView } from "./generated/TaskView";
import type { TodayView } from "./generated/TodayView";
import type { Track } from "./generated/Track";
import type { TrackInput } from "./generated/TrackInput";
import type { TrailPage } from "./generated/TrailPage";
import type { TrailQuery } from "./generated/TrailQuery";
import type { WeekInsight } from "./generated/WeekInsight";

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** A file's picture (src-tauri/src/thumbs.rs). */
export interface Thumb {
  image: string;
  kind: "thumbnail" | "icon";
}

/** Files dragged over the main window (src-tauri/src/library.rs `Drag`), in logical pixels. */
export interface Drag {
  phase: "enter" | "over" | "drop" | "leave";
  x: number;
  y: number;
  dropId: number | null;
  count: number;
  names: string[];
}

/** One learning item with its sessions and files (src-tauri/src/belajar.rs). */
export interface LearningDetail {
  card: LearningCard;
  sessions: Session[];
  files: FileCard[];
}

/** How a file was opened: by its app, its Vault copy, or shown in Explorer (a program). */
export type Opened = "opened" | "vault" | "revealed";

/** The local API for ULTRON, as Pengaturan shows it (src-tauri/src/api.rs `ApiStatus`). */
export interface ApiStatus {
  enabled: boolean;
  running: boolean;
  port: number;
  /** Why it is not running although switched on: "port" (taken) or "token". */
  problem: string | null;
  lastUsedAt: string | null;
  /** Event streams open now: ULTRON listening for reminders. */
  listeners: number;
  /** Claude Code's switch. */
  claudeEnabled: boolean;
}

/** Pengaturan > Integrasi AI > Claude Code (src-tauri/src/claude_code.rs `ClaudeCodeStatus`). */
export interface ClaudeCodeStatus {
  enabled: boolean;
  /** hub-mcp.exe beside Hub, when it is there. */
  bridge: string | null;
  /** The `claude` command, when it is found. */
  cli: string | null;
  registration: "missing" | "ours" | "other" | "unknown";
  skill: "missing" | "ours" | "other";
  skillPath: string;
  dataDir: string;
  /** What Pasang runs. */
  commands: string[];
}

/** One step of Uji jembatan (claude_code.rs `BridgeStep`). */
export interface BridgeStep {
  step: "bridge" | "hub" | "token";
  ok: boolean;
  detail: string;
}

/** Gemini as Pengaturan and Studio see it (src-tauri/src/studio.rs `GeminiStatus`). The key itself
 * never reaches the page: only whether one is saved. */
export interface GeminiStatus {
  hasKey: boolean;
  model: string;
}

/** What the page hears while Gemini answers (src-tauri/src/studio.rs `ReplyEvent`). */
export type ReplyEvent =
  | { kind: "started"; messageId: string }
  | { kind: "delta"; text: string }
  | { kind: "parts"; parts: unknown }
  | { kind: "finished"; state: MessageState; errorKind: string | null };

/** What the page hears while the PRD is written (src-tauri/src/studio.rs `PrdEvent`). */
export type PrdEvent =
  | { kind: "step"; part: number; total: number; repair: boolean }
  | { kind: "finished"; state: MessageState; errorKind: string | null; problems: number };

/** Gemini's revision of one section, not saved until accepted (studio.rs `PrdProposal`). */
export interface PrdProposal {
  section: PrdSection;
  value: unknown;
  changeNote: string;
  impacts: string[];
  problems: Problem[];
}

/** What the page hears while Gemini recommends a stack (src-tauri/src/stack.rs `StackEvent`). */
export type StackEvent = { kind: "step"; repair: boolean } | { kind: "finished"; state: MessageState; errorKind: string | null };

/** What the page hears while the roadmap is written (src-tauri/src/roadmap.rs `RoadmapEvent`). */
export type RoadmapEvent =
  | { kind: "step"; repair: boolean }
  | { kind: "finished"; state: MessageState; errorKind: string | null; problems: number };

/** A checkpoint added at the end of the approved roadmap (roadmap.rs `Appended`): saved with its
 * id, or refused with its problems. */
export interface Appended {
  id: string | null;
  problems: Problem[];
}

/** What the windows need to know about the app itself (commands.rs `Shell`). */
export interface Shell {
  hotkeyOk: boolean;
  hotkey: string;
}

export function api(b: Backend) {
  return {
    // settings and the app
    getSettings: () => b.call<Settings>("get_settings"),
    updateSettings: (patch: SettingsPatch) => b.call<Settings>("update_settings", { patch }),
    getInfo: () => b.call<Info>("get_info"),
    getShell: () => b.call<Shell>("get_shell"),
    getApiStatus: () => b.call<ApiStatus>("get_api_status"),
    rotateApiToken: () => b.call<ApiStatus>("rotate_api_token"),
    /** The path written, or null when the owner cancelled the Save dialog. */
    exportData: () => b.call<string | null>("export_data"),
    // Gemini and Studio
    geminiStatus: () => b.call<GeminiStatus>("gemini_status"),
    saveGeminiKey: (key: string) => b.call<undefined>("gemini_save_key", { key }),
    deleteGeminiKey: () => b.call<undefined>("gemini_delete_key"),
    checkGemini: () => b.call<CheckReport>("gemini_check"),
    geminiModels: () => b.call<ModelInfo[]>("gemini_models"),
    listStudioProjects: () => b.call<StudioProject[]>("list_studio_projects"),
    startStudioProject: (idea: string) => b.call<string>("start_studio_project", { idea }),
    getStudioProject: (id: string) => b.call<StudioProject>("get_studio_project", { id }),
    renameStudioProject: (id: string, name: string) => b.call<undefined>("rename_studio_project", { id, name }),
    deleteStudioProject: (id: string) => b.call<undefined>("delete_studio_project", { id }),
    advanceStudioStage: (id: string, to: StudioStage) => b.call<undefined>("advance_studio_stage", { id, to }),
    studioMessages: (projectId: string) => b.call<StudioMessage[]>("studio_messages", { projectId }),
    studioLedger: (projectId: string) => b.call<LedgerItem[]>("studio_ledger", { projectId }),
    addStudioLedger: (projectId: string, input: LedgerInput) => b.call<string>("add_studio_ledger", { projectId, input }),
    editStudioLedger: (id: string, edit: LedgerEdit) => b.call<undefined>("edit_studio_ledger", { id, edit }),
    /** The owner's message, then Gemini's reply, heard as it streams. Resolves when it ended. */
    sendStudioMessage: (projectId: string, text: string, onEvent: (event: ReplyEvent) => void) =>
      b.stream<undefined>("send_studio_message", { projectId, text }, (event) => onEvent(event as ReplyEvent)),
    retryStudioReply: (projectId: string, onEvent: (event: ReplyEvent) => void) =>
      b.stream<undefined>("retry_studio_reply", { projectId }, (event) => onEvent(event as ReplyEvent)),
    stopStudioReply: (projectId: string) => b.call<undefined>("stop_studio_reply", { projectId }),
    studioPrd: (projectId: string) => b.call<PrdView>("studio_prd", { projectId }),
    getPrdVersion: (id: string) => b.call<Prd>("get_prd_version", { id }),
    generatePrd: (projectId: string, onEvent: (event: PrdEvent) => void) =>
      b.stream<undefined>("generate_prd", { projectId }, (event) => onEvent(event as PrdEvent)),
    updatePrdSection: (projectId: string, section: PrdSection, value: unknown) =>
      b.call<undefined>("update_prd_section", { projectId, section, value }),
    revisePrdSection: (projectId: string, section: PrdSection, ask: string) =>
      b.call<PrdProposal>("revise_prd_section", { projectId, section, ask }),
    acceptPrdRevision: (projectId: string, section: PrdSection, value: unknown) =>
      b.call<undefined>("accept_prd_revision", { projectId, section, value }),
    savePrdVersion: (projectId: string, note: string) => b.call<number>("save_prd_version", { projectId, note }),
    approvePrd: (projectId: string) => b.call<number>("approve_prd", { projectId }),
    /** The path written, or null when the owner cancelled the Save dialog. */
    exportPrd: (projectId: string, versionId: string | null) => b.call<string | null>("export_prd", { projectId, versionId }),
    /** "Perbarui dari tech stack": the draft's technologies follow the approved stack (R4). */
    applyStackToPrd: (projectId: string) => b.call<undefined>("apply_stack_to_prd", { projectId }),
    studioStack: (projectId: string) => b.call<StudioStack | null>("studio_stack", { projectId }),
    saveStudioStack: (projectId: string, body: StackBody, mode: StackMode) => b.call<undefined>("save_studio_stack", { projectId, body, mode }),
    approveStudioStack: (projectId: string) => b.call<undefined>("approve_studio_stack", { projectId }),
    /** Gemini recommends a stack; with `ask`, it changes the current one. */
    recommendStudioStack: (projectId: string, ask: string, onEvent: (event: StackEvent) => void) =>
      b.stream<undefined>("recommend_studio_stack", { projectId, ask }, (event) => onEvent(event as StackEvent)),
    studioRoadmap: (projectId: string) => b.call<RoadmapView>("studio_roadmap", { projectId }),
    generateRoadmap: (projectId: string, onEvent: (event: RoadmapEvent) => void) =>
      b.stream<undefined>("generate_roadmap", { projectId }, (event) => onEvent(event as RoadmapEvent)),
    updateRoadmapCheckpoint: (id: string, value: PlanCheckpoint) => b.call<undefined>("update_roadmap_checkpoint", { id, value }),
    addRoadmapCheckpoint: (projectId: string, after: string | null) => b.call<string>("add_roadmap_checkpoint", { projectId, after }),
    removeRoadmapCheckpoint: (id: string) => b.call<undefined>("remove_roadmap_checkpoint", { id }),
    moveRoadmapCheckpoint: (id: string, to: number) => b.call<undefined>("move_roadmap_checkpoint", { id, to }),
    editRoadmap: (projectId: string) => b.call<undefined>("edit_roadmap", { projectId }),
    discardRoadmapDraft: (projectId: string) => b.call<undefined>("discard_roadmap_draft", { projectId }),
    appendRoadmapCheckpoint: (projectId: string, value: PlanCheckpoint) => b.call<Appended>("append_roadmap_checkpoint", { projectId, value }),
    /** Windows' folder dialog; null when the owner cancelled. */
    pickRepoFolder: () => b.call<FolderCheck | null>("pick_repo_folder"),
    checkRepoFolder: (path: string) => b.call<FolderCheck>("check_repo_folder", { path }),
    approveRoadmap: (projectId: string, input: ApproveRoadmap) => b.call<undefined>("approve_roadmap", { projectId, input }),
    setRepoPath: (projectId: string, path: string) => b.call<undefined>("set_repo_path", { projectId, path }),
    setClaudeAccess: (projectId: string, access: ClaudeAccess) => b.call<undefined>("set_claude_access", { projectId, access }),
    studioRequests: (projectId: string, openOnly: boolean) => b.call<StudioRequest[]>("studio_requests", { projectId, openOnly }),
    checkpointReports: (id: string) => b.call<StudioReport[]>("checkpoint_reports", { id }),
    makeCheckpointReady: (id: string) => b.call<undefined>("make_checkpoint_ready", { id }),
    startCheckpoint: (id: string) => b.call<undefined>("start_checkpoint", { id }),
    blockCheckpoint: (id: string, reason: string) => b.call<undefined>("block_checkpoint", { id, reason }),
    resumeCheckpoint: (id: string, answer: string | null) => b.call<undefined>("resume_checkpoint", { id, answer }),
    answerStudioRequest: (id: string, answer: string) => b.call<undefined>("answer_studio_request", { id, answer }),
    submitCheckpointReport: (id: string, report: ReportInput) => b.call<undefined>("submit_checkpoint_report", { id, report }),
    checkCheckpointReport: (id: string, report: ReportInput) => b.call<Problem[]>("check_checkpoint_report", { id, report }),
    approveCheckpoint: (id: string) => b.call<undefined>("approve_checkpoint", { id }),
    requestCheckpointChanges: (id: string, note: string) => b.call<undefined>("request_checkpoint_changes", { id, note }),
    setStudioTaskStatus: (id: string, status: StudioTaskStatus, note: string | null) => b.call<undefined>("set_studio_task_status", { id, status, note }),
    implementationPrompt: (id: string) => b.call<string>("implementation_prompt", { id }),
    /** "Mandiri" (R4): the whole of it in one text, for working without the MCP bridge. */
    standalonePrompt: (id: string) => b.call<string>("standalone_prompt", { id }),
    studioLink: (projectId: string) => b.call<StudioLink | null>("studio_link", { projectId }),
    claudeCodeStatus: () => b.call<ClaudeCodeStatus>("claude_code_status"),
    installClaudeCode: () => b.call<ClaudeCodeStatus>("install_claude_code"),
    removeClaudeCode: () => b.call<ClaudeCodeStatus>("remove_claude_code"),
    testClaudeBridge: () => b.call<BridgeStep[]>("test_claude_bridge"),
    rotateClaudeToken: () => b.call<ClaudeCodeStatus>("rotate_claude_token"),
    /** R5: Claude Code's session and last call, for a project or (null) for all. */
    claudeSignals: (projectId: string | null) => b.call<ClaudeSignals>("claude_signals", { projectId }),
    windowReady: () => b.call<undefined>("window_ready"),
    setCaption: (background: Rgb, text: Rgb) => b.call<undefined>("set_caption", { caption: { background, text } }),
    hideCapture: () => b.call<undefined>("hide_capture"),
    takeStartPath: () => b.call<string | null>("take_start_path"),
    // today
    getToday: () => b.call<TodayView>("get_today"),
    // tasks
    listTasks: (view: TaskView) => b.call<TaskCard[]>("list_tasks", { view }),
    taskCounts: () => b.call<TaskCounts>("task_counts"),
    getTask: (id: string) => b.call<TaskDetail>("get_task", { id }),
    createTask: (input: TaskInput) => b.call<string>("create_task", { input }),
    updateTask: (id: string, input: TaskInput) => b.call<undefined>("update_task", { id, input }),
    completeTask: (id: string) => b.call<undefined>("complete_task", { id }),
    reopenTask: (id: string) => b.call<undefined>("reopen_task", { id }),
    deleteTask: (id: string) => b.call<undefined>("delete_task", { id }),
    restoreTask: (id: string) => b.call<undefined>("restore_task", { id }),
    reorderTask: (id: string, before: string | null, after: string | null) => b.call<undefined>("reorder_task", { id, before, after }),
    setTaskDoDate: (id: string, date: string | null) => b.call<undefined>("set_task_do_date", { id, date }),
    setFocus: (date: string, taskIds: string[]) => b.call<undefined>("set_focus", { date, taskIds }),
    // projects and areas
    listProjects: () => b.call<ProjectSummary[]>("list_projects"),
    getProject: (id: string) => b.call<ProjectDetail>("get_project", { id }),
    createProject: (input: ProjectInput) => b.call<string>("create_project", { input }),
    updateProject: (id: string, input: ProjectInput) => b.call<undefined>("update_project", { id, input }),
    setProjectStatus: (id: string, status: ProjectStatus) => b.call<undefined>("set_project_status", { id, status }),
    deleteProject: (id: string) => b.call<undefined>("delete_project", { id }),
    addMilestone: (projectId: string, title: string, dueDate: string | null) =>
      b.call<string>("add_milestone", { projectId, title, dueDate }),
    updateMilestone: (id: string, title: string, dueDate: string | null) => b.call<undefined>("update_milestone", { id, title, dueDate }),
    setMilestoneDone: (id: string, done: boolean) => b.call<undefined>("set_milestone_done", { id, done }),
    deleteMilestone: (id: string) => b.call<undefined>("delete_milestone", { id }),
    listAreas: () => b.call<Area[]>("list_areas"),
    createArea: (input: AreaInput) => b.call<string>("create_area", { input }),
    updateArea: (id: string, input: AreaInput) => b.call<undefined>("update_area", { id, input }),
    deleteArea: (id: string) => b.call<undefined>("delete_area", { id }),
    // calendar
    getCalendar: (from: string, days: number) => b.call<CalendarDay[]>("get_calendar", { from, days }),
    getEvent: (id: string) => b.call<EventInput>("get_event", { id }),
    createEvent: (input: EventInput) => b.call<string>("create_event", { input }),
    updateEvent: (id: string, input: EventInput) => b.call<undefined>("update_event", { id, input }),
    rescheduleEvent: (id: string, slot: Slot) => b.call<undefined>("reschedule_event", { id, slot }),
    deleteEvent: (id: string) => b.call<undefined>("delete_event", { id }),
    restoreEvent: (id: string) => b.call<undefined>("restore_event", { id }),
    listRoutines: () => b.call<Routine[]>("list_routines"),
    createRoutine: (input: RoutineInput) => b.call<string>("create_routine", { input }),
    updateRoutine: (id: string, input: RoutineInput) => b.call<undefined>("update_routine", { id, input }),
    deleteRoutine: (id: string) => b.call<undefined>("delete_routine", { id }),
    skipOccurrence: (routineId: string, date: string) => b.call<undefined>("skip_occurrence", { routineId, date }),
    moveOccurrence: (routineId: string, date: string, slot: Slot) => b.call<undefined>("move_occurrence", { routineId, date, slot }),
    restoreOccurrence: (routineId: string, date: string) => b.call<undefined>("restore_occurrence", { routineId, date }),
    // reminders
    listReminders: () => b.call<ReminderItem[]>("list_reminders"),
    createReminder: (input: ReminderInput) => b.call<string>("create_reminder", { input }),
    updateReminder: (id: string, input: ReminderInput) => b.call<undefined>("update_reminder", { id, input }),
    snoozeReminder: (id: string, minutes: number) => b.call<undefined>("snooze_reminder", { id, minutes }),
    doneReminder: (id: string) => b.call<undefined>("done_reminder", { id }),
    cancelReminder: (id: string) => b.call<undefined>("cancel_reminder", { id }),
    // quick capture
    readCapture: (text: string, kind: CaptureKind) => b.call<Reading>("read_capture", { text, kind }),
    saveCapture: (text: string, kind: CaptureKind) => b.call<Saved>("save_capture", { text, kind }),
    // Tutup Hari
    getReview: (date: string | null) => b.call<DayReview>("get_review", { date }),
    closeDay: (input: CloseInput) => b.call<undefined>("close_day", { input }),
    // the Library
    keepDrop: (dropId: number, target: LinkTarget | null) => b.call<Added[]>("keep_drop", { dropId, target }),
    pickFiles: (target: LinkTarget | null) => b.call<Added[]>("pick_files", { target }),
    relocateFile: (id: string) => b.call<boolean>("relocate_file", { id }),
    openFile: (id: string) => b.call<Opened>("open_file", { id }),
    revealFile: (id: string) => b.call<undefined>("reveal_file", { id }),
    checkFile: (id: string) => b.call<undefined>("check_file", { id }),
    fileThumbnail: (id: string) => b.call<Thumb | null>("file_thumbnail", { id }),
    copyToVault: (id: string) => b.call<undefined>("copy_to_vault", { id }),
    removeVaultCopy: (id: string) => b.call<undefined>("remove_vault_copy", { id }),
    listFiles: (query: FileQuery) => b.call<FileCard[]>("list_files", { query }),
    fileCounts: () => b.call<FileCounts>("file_counts"),
    getFile: (id: string) => b.call<FileCard>("get_file", { id }),
    updateFile: (id: string, edit: FileEdit) => b.call<undefined>("update_file", { id, edit }),
    deleteFile: (id: string) => b.call<undefined>("delete_file", { id }),
    restoreFile: (id: string) => b.call<undefined>("restore_file", { id }),
    attachFile: (id: string, target: LinkTarget) => b.call<undefined>("attach_file", { id, target }),
    detachFile: (id: string, target: LinkTarget) => b.call<undefined>("detach_file", { id, target }),
    setFileCollections: (id: string, collectionIds: string[]) => b.call<undefined>("set_file_collections", { id, collectionIds }),
    listCollections: () => b.call<Collection[]>("list_collections"),
    createCollection: (input: CollectionInput) => b.call<string>("create_collection", { input }),
    updateCollection: (id: string, input: CollectionInput) => b.call<undefined>("update_collection", { id, input }),
    deleteCollection: (id: string) => b.call<undefined>("delete_collection", { id }),
    // Belajar
    listTracks: () => b.call<Track[]>("list_tracks"),
    createTrack: (input: TrackInput) => b.call<string>("create_track", { input }),
    updateTrack: (id: string, input: TrackInput) => b.call<undefined>("update_track", { id, input }),
    deleteTrack: (id: string) => b.call<undefined>("delete_track", { id }),
    restoreTrack: (id: string) => b.call<undefined>("restore_track", { id }),
    listLearning: (query: LearningQuery) => b.call<LearningCard[]>("list_learning", { query }),
    getLearning: (id: string) => b.call<LearningDetail>("get_learning", { id }),
    createLearning: (input: LearningInput) => b.call<string>("create_learning", { input }),
    updateLearning: (id: string, input: LearningInput) => b.call<undefined>("update_learning", { id, input }),
    setLearningStatus: (id: string, status: LearningStatus) => b.call<undefined>("set_learning_status", { id, status }),
    deleteLearning: (id: string) => b.call<undefined>("delete_learning", { id }),
    restoreLearning: (id: string) => b.call<undefined>("restore_learning", { id }),
    startTimer: (itemId: string) => b.call<undefined>("start_timer", { itemId }),
    stopTimer: () => b.call<number | null>("stop_timer"),
    discardTimer: () => b.call<undefined>("discard_timer"),
    addSession: (input: SessionInput) => b.call<string>("add_session", { input }),
    deleteSession: (id: string) => b.call<undefined>("delete_session", { id }),
    listCtf: () => b.call<CtfEvent[]>("list_ctf"),
    createCtf: (input: CtfInput) => b.call<string>("create_ctf", { input }),
    updateCtf: (id: string, input: CtfInput) => b.call<undefined>("update_ctf", { id, input }),
    deleteCtf: (id: string) => b.call<undefined>("delete_ctf", { id }),
    getLearningOverview: () => b.call<LearningOverview>("get_learning_overview"),
    // Jejak and search
    getTrail: (query: TrailQuery) => b.call<TrailPage>("get_trail", { query }),
    getWeekInsight: () => b.call<WeekInsight>("get_week_insight"),
    search: (query: string) => b.call<SearchHit[]>("search", { query }),
    openLink: (kind: "track" | "learning_item", id: string) => b.call<undefined>("open_link", { kind, id }),
  };
}

export type Api = ReturnType<typeof api>;

/** The typed core calls, for components. */
export function useApi(): Api {
  const backend = useBackend();
  return useMemo(() => api(backend), [backend]);
}
