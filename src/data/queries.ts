// What the pages read, cached with TanStack Query. The cache follows the core's change notices:
// any write, from this window, another window, or ULTRON (CP5), refreshes what is on screen. The
// data is on this machine, so a refresh is cheap; only mounted queries fetch again.

import { QueryClient, keepPreviousData, useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { toHubError, useBackend } from "./backend";
import type { HubError } from "./backend";
import { useApi } from "./api";
import { useSound } from "../sound/SoundProvider";
import type { FileQuery } from "./generated/FileQuery";
import type { LearningQuery } from "./generated/LearningQuery";
import type { Settings } from "./generated/Settings";
import type { SettingsPatch } from "./generated/SettingsPatch";
import type { TaskCard } from "./generated/TaskCard";
import type { TaskView } from "./generated/TaskView";
import type { TodayView } from "./generated/TodayView";
import type { TrailQuery } from "./generated/TrailQuery";

export const keys = {
  settings: ["settings"] as const,
  shell: ["shell"] as const,
  info: ["info"] as const,
  api: ["api"] as const,
  today: ["today"] as const,
  tasks: ["tasks"] as const,
  task: ["task"] as const,
  counts: ["task-counts"] as const,
  projects: ["projects"] as const,
  project: ["project"] as const,
  areas: ["areas"] as const,
  calendar: ["calendar"] as const,
  routines: ["routines"] as const,
  reminders: ["reminders"] as const,
  review: ["review"] as const,
  files: ["files"] as const,
  file: ["file"] as const,
  fileCounts: ["file-counts"] as const,
  collections: ["collections"] as const,
  thumb: ["thumb"] as const,
  tracks: ["tracks"] as const,
  learning: ["learning"] as const,
  learningItem: ["learning-item"] as const,
  learningOverview: ["learning-overview"] as const,
  ctf: ["ctf"] as const,
  trail: ["trail"] as const,
  week: ["week-insight"] as const,
  search: ["search"] as const,
  gemini: ["gemini"] as const,
  geminiModels: ["gemini-models"] as const,
  studio: ["studio"] as const,
  claudeCode: ["claude-code"] as const,
  claudeSignals: ["claude-signals"] as const,
  focus: ["focus"] as const,
};

export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      // Local data: a failure will not fix itself by asking three times.
      queries: { retry: 1, refetchOnWindowFocus: true },
      mutations: { retry: false },
    },
  });
}

/**
 * Whether a page should give way to its error view: when it has nothing to show, or when what it
 * shows is gone (removed meanwhile, by ULTRON say). A refresh that fails in the background for any
 * other reason keeps the page, and whatever the owner was typing, on screen.
 */
export function failedForGood(query: { isError: boolean; error: unknown; data: unknown }): boolean {
  if (!query.isError) return false;
  return query.data === undefined || toHubError(query.error).view.kind === "not_found";
}

export function useSettings() {
  const api = useApi();
  return useQuery({ queryKey: keys.settings, queryFn: api.getSettings, staleTime: Infinity });
}

/** The running Sesi Fokus (Lentera CP02), or null. Each window works out the time left itself. */
export function useFocus() {
  const api = useApi();
  return useQuery({ queryKey: keys.focus, queryFn: api.getFocus });
}

export function useShell() {
  const api = useApi();
  return useQuery({ queryKey: keys.shell, queryFn: api.getShell, staleTime: Infinity });
}

/** The local API's state. Asked again now and then: "last used" moves without a change notice. */
export function useApiStatus() {
  const api = useApi();
  return useQuery({ queryKey: keys.api, queryFn: api.getApiStatus, refetchInterval: 30_000 });
}

export function useRotateApiToken() {
  const api = useApi();
  const client = useQueryClient();
  return useMutation({
    mutationFn: api.rotateApiToken,
    onSuccess: (status) => client.setQueryData(keys.api, status),
  });
}

export function useExportData() {
  const api = useApi();
  return useMutation({ mutationFn: api.exportData });
}

/** Whether a Gemini key is saved, and the model. */
export function useGeminiStatus() {
  const api = useApi();
  return useQuery({ queryKey: keys.gemini, queryFn: api.geminiStatus });
}

/** The models the saved key can use. This one asks Google, so it is asked once when shown and
 * again only after the key changes, never on focus or on Hub's own change notices. */
export function useGeminiModels(enabled: boolean) {
  const api = useApi();
  return useQuery({
    queryKey: keys.geminiModels,
    queryFn: api.geminiModels,
    enabled,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
}

export function useSaveGeminiKey() {
  const api = useApi();
  const client = useQueryClient();
  return useMutation<undefined, HubError, string>({
    mutationFn: (key) => api.saveGeminiKey(key).catch((error: unknown) => Promise.reject(toHubError(error))),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: keys.gemini });
      void client.resetQueries({ queryKey: keys.geminiModels });
    },
  });
}

export function useDeleteGeminiKey() {
  const api = useApi();
  const client = useQueryClient();
  return useMutation<undefined, HubError>({
    mutationFn: () => api.deleteGeminiKey().catch((error: unknown) => Promise.reject(toHubError(error))),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: keys.gemini });
      client.removeQueries({ queryKey: keys.geminiModels });
    },
  });
}

/** "Uji koneksi": a mutation, because it asks Google each time it is pressed. */
export function useCheckGemini() {
  const api = useApi();
  return useMutation({ mutationFn: api.checkGemini });
}

export function useStudioProjects() {
  const api = useApi();
  return useQuery({ queryKey: keys.studio, queryFn: api.listStudioProjects });
}

export function useStudioProject(id: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "project", id], queryFn: () => api.getStudioProject(id) });
}

export function useStudioMessages(projectId: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "messages", projectId], queryFn: () => api.studioMessages(projectId) });
}

export function useStudioLedger(projectId: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "ledger", projectId], queryFn: () => api.studioLedger(projectId) });
}

export function useStudioPrd(projectId: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "prd", projectId], queryFn: () => api.studioPrd(projectId) });
}

export function usePrdVersion(id: string | null) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "prd-version", id], queryFn: () => api.getPrdVersion(id ?? ""), enabled: id !== null });
}

export function useStudioStack(projectId: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "stack", projectId], queryFn: () => api.studioStack(projectId) });
}

export function useStudioRoadmap(projectId: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "roadmap", projectId], queryFn: () => api.studioRoadmap(projectId) });
}

/** Asks the `claude` command, which takes a moment: refreshed only when settings change. */
/** Runs the `claude` command (up to a few seconds), so Bangun and Pengaturan share one answer for
 * five minutes; a settings change (the switch, Pasang, Lepas) asks again. */
export function useClaudeCodeStatus() {
  const api = useApi();
  return useQuery({ queryKey: keys.claudeCode, queryFn: api.claudeCodeStatus, staleTime: 5 * 60_000 });
}

/** Claude Code's session and last call (R5). Asked again every 30 s as well, since a session that
 * goes quiet sends no notice. */
export function useClaudeSignals(projectId: string | null) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.claudeSignals, projectId ?? "all"], queryFn: () => api.claudeSignals(projectId), refetchInterval: 30_000 });
}

export function useStudioRequests(projectId: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "requests", projectId], queryFn: () => api.studioRequests(projectId, false) });
}

export function useCheckpointReports(id: string | null) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "reports", id], queryFn: () => api.checkpointReports(id ?? ""), enabled: id !== null });
}

/** The Studio project a Hub Proyek is built in, if any. */
export function useStudioLink(projectId: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.studio, "link", projectId], queryFn: () => api.studioLink(projectId) });
}

/** A Studio write: refused input comes back as a HubError the page can name. */
export function useStudioAct<A, R>(act: (args: A) => Promise<R>) {
  return useMutation<R, HubError, A>({
    mutationFn: async (args) => {
      try {
        return await act(args);
      } catch (error) {
        throw toHubError(error);
      }
    },
  });
}

export function useInfo() {
  const api = useApi();
  return useQuery({ queryKey: keys.info, queryFn: api.getInfo });
}

/** Today's view. The local date is part of the key, so the page moves to the new day at midnight. */
export function useToday(date: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.today, date], queryFn: api.getToday });
}

export function useTasks(view: TaskView, date: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.tasks, view, date], queryFn: () => api.listTasks(view) });
}

export function useTaskCounts(date: string) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.counts, date], queryFn: api.taskCounts });
}

export function useTask(id: string | null) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.task, id], queryFn: () => api.getTask(id ?? ""), enabled: id !== null });
}

export function useProjects() {
  const api = useApi();
  return useQuery({ queryKey: keys.projects, queryFn: api.listProjects });
}

export function useProject(id: string, enabled = true) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.project, id], queryFn: () => api.getProject(id), enabled });
}

export function useAreas() {
  const api = useApi();
  return useQuery({ queryKey: keys.areas, queryFn: api.listAreas });
}

export function useCalendar(from: string, days: number) {
  const api = useApi();
  return useQuery({
    queryKey: [...keys.calendar, from, days],
    queryFn: () => api.getCalendar(from, days),
    placeholderData: (previous) => previous,
  });
}

export function useRoutines() {
  const api = useApi();
  return useQuery({ queryKey: keys.routines, queryFn: api.listRoutines });
}

export function useReminders() {
  const api = useApi();
  return useQuery({ queryKey: keys.reminders, queryFn: api.listReminders });
}

export function useReview(date: string, enabled: boolean) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.review, date], queryFn: () => api.getReview(date), enabled });
}

export function useUpdateSettings() {
  const api = useApi();
  const client = useQueryClient();
  return useMutation<Settings, HubError, SettingsPatch>({
    mutationFn: async (patch) => {
      try {
        return await api.updateSettings(patch);
      } catch (error) {
        throw toHubError(error);
      }
    },
    onSuccess: (settings) => client.setQueryData(keys.settings, settings),
  });
}

interface DoneInput {
  id: string;
  done: boolean;
}

type Snapshot = [readonly unknown[], unknown][];

function markDone(list: readonly TaskCard[], id: string, done: boolean): TaskCard[] {
  return list.map((task) =>
    task.id === id ? { ...task, status: done ? ("done" as const) : ("todo" as const), overdue: done ? false : task.overdue } : task,
  );
}

/** Check or uncheck a task. The check shows at once everywhere; a refusal puts it back. */
export function useSetTaskDone() {
  const api = useApi();
  const client = useQueryClient();
  const sound = useSound();
  return useMutation<undefined, HubError, DoneInput, { saved: Snapshot }>({
    mutationFn: async ({ id, done }) => {
      try {
        await (done ? api.completeTask(id) : api.reopenTask(id));
        return undefined;
      } catch (error) {
        throw toHubError(error);
      }
    },
    onMutate: async ({ id, done }) => {
      // P2: the check is heard at once; the last of the day's focus tasks is a small celebration.
      const today = client.getQueriesData<TodayView>({ queryKey: keys.today }).find(([, view]) => view)?.[1];
      const focus = today?.focus ?? [];
      const lastFocus = done && focus.some((task) => task.id === id) && focus.every((task) => task.id === id || task.status === "done");
      sound.play(done ? (lastFocus ? "hero" : "complete") : "reopen");
      await client.cancelQueries({ queryKey: keys.today });
      await client.cancelQueries({ queryKey: keys.tasks });
      const saved: Snapshot = [...client.getQueriesData({ queryKey: keys.today }), ...client.getQueriesData({ queryKey: keys.tasks })];
      client.setQueriesData<TodayView>({ queryKey: keys.today }, (view) =>
        view ? { ...view, focus: markDone(view.focus, id, done), also: markDone(view.also, id, done) } : view,
      );
      client.setQueriesData<TaskCard[]>({ queryKey: keys.tasks }, (list) => (list ? markDone(list, id, done) : list));
      return { saved };
    },
    onError: (_error, _input, context) => {
      for (const [key, value] of context?.saved ?? []) client.setQueryData(key, value);
    },
  });
}

export function useFiles(query: FileQuery) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.files, query], queryFn: () => api.listFiles(query), placeholderData: keepPreviousData });
}

export function useFileCounts() {
  const api = useApi();
  return useQuery({ queryKey: keys.fileCounts, queryFn: api.fileCounts });
}

export function useFile(id: string | null) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.file, id], queryFn: () => api.getFile(id ?? ""), enabled: id !== null });
}

export function useCollections() {
  const api = useApi();
  return useQuery({ queryKey: keys.collections, queryFn: api.listCollections });
}

/** A file's picture. Pictures are cached by Hub on disk; a change notice does not refetch them. */
export function useThumbnail(id: string, enabled = true) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.thumb, id], queryFn: () => api.fileThumbnail(id), staleTime: Infinity, enabled });
}

export function useTracks() {
  const api = useApi();
  return useQuery({ queryKey: keys.tracks, queryFn: api.listTracks });
}

export function useLearning(query: LearningQuery, enabled = true) {
  const api = useApi();
  return useQuery({
    queryKey: [...keys.learning, query],
    queryFn: () => api.listLearning(query),
    enabled,
    placeholderData: keepPreviousData,
  });
}

export function useLearningItem(id: string | null) {
  const api = useApi();
  return useQuery({ queryKey: [...keys.learningItem, id], queryFn: () => api.getLearning(id ?? ""), enabled: id !== null });
}

export function useLearningOverview() {
  const api = useApi();
  return useQuery({ queryKey: keys.learningOverview, queryFn: api.getLearningOverview });
}

export function useCtfEvents() {
  const api = useApi();
  return useQuery({ queryKey: keys.ctf, queryFn: api.listCtf });
}

/** Jejak, page by page. */
export function useTrail(query: Omit<TrailQuery, "before">) {
  const api = useApi();
  return useInfiniteQuery({
    queryKey: [...keys.trail, query],
    queryFn: ({ pageParam }) => api.getTrail({ ...query, before: pageParam ?? undefined }),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next,
  });
}

export function useWeekInsight() {
  const api = useApi();
  return useQuery({ queryKey: keys.week, queryFn: api.getWeekInsight });
}

export function useSearch(query: string) {
  const api = useApi();
  const text = query.trim();
  return useQuery({
    queryKey: [...keys.search, text],
    queryFn: () => api.search(text),
    enabled: text.length > 0,
    placeholderData: keepPreviousData,
  });
}

/** Follow the core's change notices for as long as the app runs. */
export function useChangeSync(): void {
  const backend = useBackend();
  const client = useQueryClient();
  useEffect(
    () =>
      backend.onChanged((change) => {
        void client.invalidateQueries({
          predicate: (query) => {
            const head = query.queryKey[0];
            if (head === keys.shell[0]) return false;
            // Claude Code's sessions change only what is said about them (every call of Claude's).
            if (change.entity === "claude_session") return head === keys.claudeSignals[0];
            // Asks Google: only a new key refreshes it (useSaveGeminiKey).
            if (head === keys.geminiModels[0]) return false;
            // Runs the `claude` command: only a settings change (the switch, Pasang) refreshes it.
            if (head === keys.claudeCode[0]) return change.entity === "settings";
            // A picture belongs to the file's content, not to its row: it changes only when the
            // file does, which the thumbnail's own cache key already follows.
            if (head === keys.thumb[0]) return change.entity === "file" && change.ids.includes(String(query.queryKey[1]));
            if (head === keys.settings[0]) return change.entity === "settings";
            return true;
          },
        });
      }),
    [backend, client],
  );
}
