# Windows Implementation Guide: Summary-Style Migration

This document describes the v1.2.1 summary-style migration feature so the Windows build can implement the same behavior without depending on macOS-specific assumptions.

## Goal

Add a second migration mode inside the existing `Migrate` dialog:

- `完整对话迁移`: existing full conversation migration.
- `总结式迁移`: copies a compact continuation card to the clipboard.

The new mode must not replace the existing full migration or the existing toolbar low-token continuation prompt.

## Expected User Flow

1. User selects a conversation.
2. User clicks `Migrate`.
3. Modal opens with `完整对话迁移` selected by default.
4. User selects `总结式迁移`.
5. Modal hides target agent and copy/cut controls.
6. Primary button changes to `复制继续卡片`.
7. User clicks `复制继续卡片`.
8. App copies the continuation brief to the clipboard.
9. App shows a success toast: `继续卡片已复制`.
10. App closes the modal automatically.

If clipboard copy fails:

1. Keep the modal open.
2. Show an error toast: `继续卡片复制失败`.
3. Do not call `migrate_conversation`.

## Existing Behavior That Must Remain

### Full Migration

`完整对话迁移` remains the default. It still uses the existing native command:

```ts
invoke("migrate_conversation", {
  source,
  target,
  id,
  mode: "copy" | "cut",
});
```

The full migration path still needs:

- target platform selection
- `复制` / `移动` mode selection
- read-back verification after writing to the target platform
- source deletion only after verified move success

### Toolbar Low-Token Prompt

The conversation toolbar keeps the existing low-token prompt behavior:

- English label: `Copy low-token prompt`
- Chinese label: `复制省 token 续接提示`

Do not replace this toolbar action with `Continuation Brief`. The continuation brief belongs in the `Migrate` modal only.

## Frontend Component Contract

The modal needs two independent concepts:

```ts
type MigrateMode = "copy" | "cut";
type MigrateKind = "full" | "brief";
```

Recommended props:

```ts
interface MigrateModalProps {
  sourceAgent: AgentType;
  onMigrate: (targetAgent: AgentType, mode: MigrateMode) => void;
  onCopyContinuationBrief: () => void;
  continuationBriefAvailable: boolean;
  onClose: () => void;
}
```

State inside the modal:

```ts
const [kind, setKind] = useState<MigrateKind>("full");
const [targetAgent, setTargetAgent] = useState<AgentType>(firstTargetFor(sourceAgent));
const [mode, setMode] = useState<MigrateMode>("copy");
```

Render logic:

- Always render `迁移内容` radio group.
- If `kind === "full"`:
  - render target platform selector
  - render copy/cut mode selector
  - primary button calls `onMigrate(targetAgent, mode)`
- If `kind === "brief"`:
  - hide target platform selector
  - hide copy/cut selector
  - show helper text: `总结式迁移不会写入目标平台，也不会删除原对话；它只把继续卡片复制到剪贴板。`
  - primary button calls `onCopyContinuationBrief()`
  - primary button is disabled if `continuationBriefAvailable === false`

## Clipboard And Feedback Logic

The parent app should own clipboard behavior because it already owns app-level toasts and selected conversation state.

Recommended parent helper:

```ts
const handleCopy = async (target: CopyTarget, value: string | null | undefined) => {
  if (!value) {
    return false;
  }

  try {
    if (!navigator.clipboard?.writeText) {
      throw new Error("Clipboard unavailable");
    }
    await navigator.clipboard.writeText(value);
    setCopyState({ target, status: "success" });
    return true;
  } catch (error) {
    console.error(`Failed to copy ${target}:`, error);
    setCopyState({ target, status: "error" });
    return false;
  } finally {
    window.setTimeout(() => {
      setCopyState((current) =>
        current.target === target ? { target: null, status: "idle" } : current,
      );
    }, COPY_RESET_DELAY_MS);
  }
};
```

Recommended modal-specific handler:

```ts
const handleCopyContinuationBriefFromMigrate = async () => {
  const copied = await handleCopy("continuation", continuationBriefPrompt);
  if (!copied) {
    setAppNotice({
      kind: "error",
      message: locale === "en" ? "Could not copy continuation card" : "继续卡片复制失败",
    });
    return;
  }

  setAppNotice({
    kind: "success",
    message: locale === "en" ? "Continuation card copied" : "继续卡片已复制",
  });
  setShowMigrateModal(false);
};
```

Important Windows note:

- Tauri WebView clipboard support can vary by WebView2 policy and focus state. Keep the failure path visible and do not close the modal on failure.

## Continuation Brief Generator

The generator is platform-neutral TypeScript:

```ts
buildContinuationBriefPrompt({
  repoRoot,
  conversation,
  checkpointId,
  handoffId,
});
```

Input shape:

```ts
type ContinuationBriefConversation = {
  id: string;
  source_agent: string;
  summary: string | null;
  resume_command?: string | null;
  storage_path?: string | null;
  messages: Array<{
    id: string;
    role: string;
    content: string;
    timestamp?: string;
  }>;
  file_changes: Array<{
    path: string;
    change_type: string;
  }>;
};
```

Output starts with:

```text
# Continuation Brief
```

The output must avoid raw transcript dumping. It should summarize:

- current workline
- latest completed action
- where to resume
- canonical files
- obsolete context
- evidence source
- tool protocol

## Path Handling For Windows

Do not normalize Windows paths into POSIX-only format when displaying evidence or canonical files. The brief generator should preserve meaningful original paths while filtering ephemeral paths.

Filter examples:

- `codex-clipboard-*`
- `AppData/Local/Temp`
- `AppData\\Local\\Temp`

Keep examples:

- `D:\...`
- `C:\Users\...`
- repo-relative files
- manuscript, figure, script, analysis, and final delivery paths

## Tests To Port Or Keep Green

Focused UI tests:

- toolbar still copies low-token prompt
- full migration still calls `migrate_conversation`
- summary-style migration copies `# Continuation Brief`
- summary-style migration does not call `migrate_conversation`
- successful summary-style copy shows toast and closes modal
- failed summary-style copy leaves modal open and shows error toast

Generator tests:

- active workline extraction
- latest completed action extraction
- canonical file selection
- obsolete context filtering
- process-noise filtering
- token budget posture
- no raw transcript path leakage

Eval command:

```bash
npm run eval:continuation-brief
```

Expected v1.2.1 baseline:

- 10 cases
- 100% pass
- 0 P0
- 0 P1

## Windows Build Checklist

Before shipping the Windows build:

```powershell
npm ci
npm run test:run -- src/__tests__/App.test.tsx -t "low-token continuation prompt|summary-style migration|keeps migration working|1.2.1 version"
npm run test:run -- src/__tests__/continuationBrief.test.ts src/__tests__/continuationBriefEval.test.ts src/__tests__/continuationBriefEvalCases.test.ts
npm run eval:continuation-brief
npm run build
npm run tauri build
```

If using GitHub Actions, pushing tag `v1.2.1` runs `.github/workflows/release.yml`, which builds:

- Windows installer
- Windows portable zip
- macOS Intel DMG
- macOS Apple Silicon DMG
- updater assets when `TAURI_PRIVATE_KEY` and `TAURI_KEY_PASSWORD` are configured

## Files To Review

- `src/components/MigrateModal.tsx`
- `src/App.tsx`
- `src/utils/continuationBrief.ts`
- `src/utils/continuationBriefEval.ts`
- `src/utils/continuationBriefEvalCases.ts`
- `src/__tests__/App.test.tsx`
- `src/__tests__/continuationBrief.test.ts`
- `src/__tests__/continuationBriefEval.test.ts`
- `src/__tests__/continuationBriefEvalCases.test.ts`
- `scripts/eval-continuation-brief.mjs`

## Acceptance Criteria

The Windows implementation is equivalent when:

- `完整对话迁移` remains default and unchanged.
- `总结式迁移` is visible inside the `Migrate` modal.
- `复制继续卡片` copies a `# Continuation Brief`.
- Success shows `继续卡片已复制`.
- Success closes the modal.
- Failure leaves the modal open.
- Toolbar low-token prompt still exists and is not replaced.
- `migrate_conversation` is never called by summary-style migration.
