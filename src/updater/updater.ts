import { relaunch } from "@tauri-apps/api/process";
import { checkUpdate, installUpdate } from "@tauri-apps/api/updater";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up-to-date" }
  | { kind: "available"; version: string; notes: string | null; publishedAt: string | null }
  | { kind: "installing"; version: string }
  | { kind: "error"; message: string };

export async function runUpdateCheck(): Promise<UpdateState> {
  const result = await checkUpdate();

  if (!result.shouldUpdate) {
    return { kind: "up-to-date" };
  }

  return {
    kind: "available",
    version: result.manifest?.version ?? "",
    notes: result.manifest?.body ?? null,
    publishedAt: result.manifest?.date ?? null,
  };
}

export async function installAvailableUpdate(version: string) {
  await installUpdate();
  await relaunch();

  return { kind: "installing", version } as const;
}
