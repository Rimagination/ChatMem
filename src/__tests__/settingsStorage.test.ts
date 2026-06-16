import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, normalizeAppSettings } from "../settings/storage";

describe("settings storage", () => {
  it("keeps automatic recovery checkpoints opt-in", () => {
    expect(DEFAULT_SETTINGS.autoCaptureMemory).toBe(false);
    expect(normalizeAppSettings({ locale: "en" }).autoCaptureMemory).toBe(false);
    expect(normalizeAppSettings({ locale: "en", autoCaptureMemory: false }).autoCaptureMemory).toBe(false);
    expect(normalizeAppSettings({ locale: "en", autoCaptureMemory: true }).autoCaptureMemory).toBe(true);
  });
});
