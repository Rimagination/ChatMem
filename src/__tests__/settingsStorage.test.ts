import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, normalizeAppSettings } from "../settings/storage";

describe("settings storage", () => {
  it("keeps automatic recovery checkpoints opt-in", () => {
    expect(DEFAULT_SETTINGS.autoCaptureMemory).toBe(false);
    expect(normalizeAppSettings({ locale: "en" }).autoCaptureMemory).toBe(false);
    expect(normalizeAppSettings({ locale: "en", autoCaptureMemory: false }).autoCaptureMemory).toBe(false);
    expect(normalizeAppSettings({ locale: "en", autoCaptureMemory: true }).autoCaptureMemory).toBe(true);
  });

  it("normalizes saved favorite conversation snapshots", () => {
    expect(DEFAULT_SETTINGS.favoriteConversations).toEqual({});

    const normalized = normalizeAppSettings({
      locale: "en",
      favoriteConversations: {
        "claude:conv-001": {
          id: "conv-001",
          sourceAgent: "claude",
          projectDir: "D:/VSP/demo",
          createdAt: "2026-04-08T08:00:00Z",
          updatedAt: "2026-04-08T09:00:00Z",
          summary: "Debug session",
        },
        broken: {
          id: "missing-source-agent",
        },
      },
    });

    expect(normalized.favoriteConversations).toEqual({
      "claude:conv-001": {
        id: "conv-001",
        sourceAgent: "claude",
        projectDir: "D:/VSP/demo",
        createdAt: "2026-04-08T08:00:00Z",
        updatedAt: "2026-04-08T09:00:00Z",
        summary: "Debug session",
      },
    });
  });
});
