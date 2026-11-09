// How the UI reaches the Rust core. Two implementations: the Tauri bridge (the app) and an
// in-memory mock (browser development and tests). Components get one through BackendProvider and
// call it through the typed functions in api.ts, never by command name.

import { createContext, useContext } from "react";

import type { Change } from "./generated/Change";
import type { ErrorView } from "./generated/ErrorView";

/** Every command the core offers (src-tauri/build.rs). */
export type Command =
  | "get_settings"
  | "update_settings"
  | "get_api_status"
  | "rotate_api_token"
  | "export_data"
  | "gemini_status"
  | "gemini_save_key"
  | "gemini_delete_key"
  | "gemini_check"
  | "gemini_models"
  | "list_studio_projects"
  | "start_studio_project"
  | "get_studio_project"
  | "rename_studio_project"
  | "delete_studio_project"
  | "advance_studio_stage"
  | "studio_messages"
  | "studio_ledger"
  | "add_studio_ledger"
  | "edit_studio_ledger"
  | "send_studio_message"
  | "retry_studio_reply"
  | "stop_studio_reply"
  | "studio_prd"
  | "get_prd_version"
  | "generate_prd"
  | "update_prd_section"
  | "revise_prd_section"
  | "accept_prd_revision"
  | "save_prd_version"
  | "approve_prd"
  | "apply_stack_to_prd"
  | "export_prd"
  | "studio_stack"
  | "save_studio_stack"
  | "approve_studio_stack"
  | "recommend_studio_stack"
  | "studio_roadmap"
  | "generate_roadmap"
  | "update_roadmap_checkpoint"
  | "add_roadmap_checkpoint"
  | "remove_roadmap_checkpoint"
  | "move_roadmap_checkpoint"
  | "edit_roadmap"
  | "discard_roadmap_draft"
  | "append_roadmap_checkpoint"
  | "pick_repo_folder"
  | "check_repo_folder"
  | "approve_roadmap"
  | "set_repo_path"
  | "set_claude_access"
  | "studio_requests"
  | "checkpoint_reports"
  | "make_checkpoint_ready"
  | "start_checkpoint"
  | "block_checkpoint"
  | "resume_checkpoint"
  | "answer_studio_request"
  | "submit_checkpoint_report"
  | "check_checkpoint_report"
  | "approve_checkpoint"
  | "request_checkpoint_changes"
  | "set_studio_task_status"
  | "implementation_prompt"
  | "standalone_prompt"
  | "studio_link"
  | "claude_code_status"
  | "install_claude_code"
  | "remove_claude_code"
  | "test_claude_bridge"
  | "rotate_claude_token"
  | "claude_signals"
  | "get_info"
  | "get_shell"
  | "window_ready"
  | "set_caption"
  | "hide_capture"
  | "take_start_path"
  | "get_today"
  | "list_tasks"
  | "task_counts"
  | "get_task"
  | "create_task"
  | "update_task"
  | "complete_task"
  | "reopen_task"
  | "delete_task"
  | "restore_task"
  | "reorder_task"
  | "set_task_do_date"
  | "set_focus"
  | "list_projects"
  | "get_project"
  | "create_project"
  | "update_project"
  | "set_project_status"
  | "delete_project"
  | "add_milestone"
  | "update_milestone"
  | "set_milestone_done"
  | "delete_milestone"
  | "list_areas"
  | "create_area"
  | "update_area"
  | "delete_area"
  | "get_calendar"
  | "get_event"
  | "create_event"
  | "update_event"
  | "reschedule_event"
  | "delete_event"
  | "restore_event"
  | "list_routines"
  | "create_routine"
  | "update_routine"
  | "delete_routine"
  | "skip_occurrence"
  | "move_occurrence"
  | "restore_occurrence"
  | "list_reminders"
  | "create_reminder"
  | "update_reminder"
  | "snooze_reminder"
  | "done_reminder"
  | "cancel_reminder"
  | "read_capture"
  | "save_capture"
  | "get_review"
  | "close_day"
  | "keep_drop"
  | "pick_files"
  | "relocate_file"
  | "open_file"
  | "reveal_file"
  | "check_file"
  | "file_thumbnail"
  | "copy_to_vault"
  | "remove_vault_copy"
  | "list_files"
  | "file_counts"
  | "get_file"
  | "update_file"
  | "delete_file"
  | "restore_file"
  | "attach_file"
  | "detach_file"
  | "set_file_collections"
  | "list_collections"
  | "create_collection"
  | "update_collection"
  | "delete_collection"
  | "list_tracks"
  | "create_track"
  | "update_track"
  | "delete_track"
  | "restore_track"
  | "list_learning"
  | "get_learning"
  | "create_learning"
  | "update_learning"
  | "set_learning_status"
  | "delete_learning"
  | "restore_learning"
  | "start_timer"
  | "stop_timer"
  | "discard_timer"
  | "add_session"
  | "delete_session"
  | "list_ctf"
  | "create_ctf"
  | "update_ctf"
  | "delete_ctf"
  | "get_learning_overview"
  | "get_trail"
  | "get_week_insight"
  | "search"
  | "open_link";

/** Events the native side sends to the windows. */
export type HubEvent = "hub://changed" | "hub://navigate" | "hub://capture-shown" | "hub://drag";

export interface Backend {
  call: <T>(command: Command, args?: Record<string, unknown>) => Promise<T>;
  /** A command that reports progress through events (a Tauri channel, `onEvent`) before it
   * resolves: Gemini's reply as it streams. */
  stream: <T>(command: Command, args: Record<string, unknown>, onEvent: (event: unknown) => void) => Promise<T>;
  /** Called after any committed write, by whoever made it (the owner here, ULTRON later). */
  onChanged: (listener: (change: Change) => void) => () => void;
  /** Other native events: a page to go to, the capture window shown again, files dragged over. */
  on: (event: Exclude<HubEvent, "hub://changed">, listener: (payload: unknown) => void) => () => void;
}

/** A refusal or failure from the core, with its kind, field, and reason code. */
export class HubError extends Error {
  readonly view: ErrorView;

  constructor(view: ErrorView) {
    super(view.message);
    this.name = "HubError";
    this.view = view;
  }
}

/** Anything thrown by a backend call, as a HubError. */
export function toHubError(error: unknown): HubError {
  if (error instanceof HubError) return error;
  if (typeof error === "object" && error !== null && "kind" in error && "message" in error) {
    const view = error as Partial<ErrorView>;
    return new HubError({
      kind: view.kind ?? "internal",
      field: view.field ?? null,
      reason: view.reason ?? null,
      message: String(view.message),
    });
  }
  return new HubError({ kind: "internal", field: null, reason: null, message: String(error) });
}

export const BackendContext = createContext<Backend | null>(null);

export function useBackend(): Backend {
  const backend = useContext(BackendContext);
  if (!backend) throw new Error("useBackend outside BackendContext");
  return backend;
}
