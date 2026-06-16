export type ContinuationBriefMessage = {
  id: string;
  role: string;
  content: string;
  timestamp?: string;
};

export type ContinuationBriefFileChange = {
  path: string;
  change_type: string;
};

export type ContinuationBriefConversation = {
  id: string;
  source_agent: string;
  summary: string | null;
  resume_command?: string | null;
  storage_path?: string | null;
  messages: ContinuationBriefMessage[];
  file_changes: ContinuationBriefFileChange[];
};

export type ContinuationBriefInput = {
  repoRoot: string;
  conversation: ContinuationBriefConversation;
  checkpointId?: string | null;
  handoffId?: string | null;
};

type MessageCandidate = {
  message: ContinuationBriefMessage;
  index: number;
  content: string;
};

const ACTIVE_WORKLINE_WORDS = [
  "当前",
  "现在",
  "最终",
  "主线",
  "工作线",
  "改投",
  "投稿",
  "JoH",
  "JOH",
  "最终整理包",
  "PNG与代码",
  "current",
  "active",
  "canonical",
  "final",
  "submission",
];

const OBSOLETE_CONTEXT_WORDS = [
  "拒稿",
  "归档",
  "历史",
  "旧版",
  "过时",
  "不用",
  "删除",
  "删掉",
  "EI",
  "obsolete",
  "archived",
  "superseded",
  "deprecated",
];

const COMPLETED_ACTION_WORDS = [
  "已经",
  "已完成",
  "已生成",
  "已更新",
  "已同步",
  "已删除",
  "已处理",
  "已复制",
  "已迁移",
  "已写完",
  "已修复",
  "已提交",
  "完成",
  "整理好了",
  "归档完成",
  "改好了",
  "处理完",
  "同步",
  "生成",
  "删除",
  "删掉",
  "核对",
  "implemented",
  "completed",
  "updated",
  "fixed",
  "done",
  "created",
  "copied",
  "committed",
];

const DELIVERY_WORDS = [
  "新文件",
  "文件在这里",
  "新版本",
  "这版",
  "在这里",
  "最新版",
  "最终投稿包",
  "交付",
  "输出",
  "导出",
  "生成",
  "written to",
  "saved to",
];

const RISK_WORDS = [
  "风险",
  "硬伤",
  "必须",
  "不建议",
  "问题",
  "risk",
  "must",
  "blocker",
  "caution",
];

const LOW_SIGNAL_WORDS = [
  "我会先",
  "我先",
  "先看",
  "先定位",
  "接下来",
  "收到",
  "看一下",
  "定位",
  "I'll first",
  "I will first",
];

const PROCESS_NOISE_WORDS = [
  "`rg`",
  "rg ",
  "扫到了",
  "缩小到",
  "来找",
  "路径确认",
  "精确命中",
  "现在删除这个文件夹",
  "定位到具体脚本",
  "直接改坐标",
  "检查一下当前图件目录",
  "只删除这些生成物",
  "按文件名前缀精确删除",
  "不碰原始数据",
  "我再导出",
  "再看一个",
  "确认没有排版问题",
  "我来做成“投稿最终包”",
  "我会把最终包放到",
  "我再做一下完整性核对",
  "完整性核对",
  "没有明显压盖",
  "保证包里不是旧版本",
];

const EPHEMERAL_FILE_WORDS = [
  "codex-clipboard",
  "appdata/local/temp",
  "appdata\\local\\temp",
];

const CANONICAL_FILE_WORDS = [
  "最终",
  "当前",
  "主线",
  "JoH",
  "JOH",
  "投稿",
  "PNG与代码",
  "Main_Figures",
  "AGENTS.md",
  "manuscript",
  "supplementary",
  "figure",
  "fig",
  "script",
  "analysis",
  "current",
  "canonical",
  "final",
];

function cleanLine(value: string | null | undefined) {
  return value?.replace(/\s+/g, " ").trim() || "";
}

function stripCodexAttachmentWrapper(value: string | null | undefined) {
  const content = value || "";
  const requestMarker = /##\s*My request for Codex:\s*/i.exec(content);
  if (requestMarker) {
    return content.slice(requestMarker.index + requestMarker[0].length);
  }
  return content;
}

function cleanMessageContent(value: string | null | undefined) {
  return cleanLine(stripCodexAttachmentWrapper(value).replace(/<image\b[\s\S]*?<\/image>/giu, ""));
}

function extractHandoffResumeGoal(content: string) {
  if (!includesAny(content, ["交接文档", "handoff"])) {
    return null;
  }
  const match = /继续[^，。；;,.!?！？\n]+/u.exec(content);
  return match ? cleanLine(match[0]) : null;
}

function conversationKey(conversation: Pick<ContinuationBriefConversation, "id" | "source_agent">) {
  return `${conversation.source_agent}:${conversation.id}`;
}

function estimateTokens(text: string) {
  return Math.max(1, Math.ceil(text.length / 4));
}

function listLines(values: string[], fallback: string) {
  if (values.length === 0) {
    return [`- ${fallback}`];
  }
  return values.map((value) => `- ${value}`);
}

function truncateLine(value: string, maxLength = 180) {
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, Math.max(0, maxLength - 3))}...`;
}

function includesAny(value: string, words: string[]) {
  const lower = value.toLowerCase();
  return words.some((word) => lower.includes(word.toLowerCase()));
}

function isProcessNoise(content: string) {
  return includesAny(content, PROCESS_NOISE_WORDS);
}

function isLowSignal(content: string) {
  return (
    isProcessNoise(content) ||
    (includesAny(content, LOW_SIGNAL_WORDS) && !includesAny(content, COMPLETED_ACTION_WORDS))
  );
}

function isDeliveryMessage(content: string) {
  return extractFileReferences(content).length > 0 && includesAny(content, DELIVERY_WORDS);
}

function isCompletedAssistantAction(candidate: MessageCandidate) {
  return (
    roleOf(candidate).toLowerCase() === "assistant" &&
    !isProcessNoise(candidate.content) &&
    (includesAny(candidate.content, COMPLETED_ACTION_WORDS) || isDeliveryMessage(candidate.content))
  );
}

function deriveHandoffResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
) {
  if (!latestUser || !latestCompletedAction) {
    return null;
  }
  const goal = extractHandoffResumeGoal(latestUser.content);
  const handoffFile = extractFileReferences(latestCompletedAction.content).find((path) =>
    includesAny(path, ["交接", "handoff"]),
  );

  if (!goal || !handoffFile) {
    return null;
  }

  return {
    currentGoal: goal,
    whereToResume: `${goal}；先打开交接文档：${handoffFile}`,
  };
}

function deriveSubmissionPackageResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
) {
  if (!latestUser || !latestCompletedAction) {
    return null;
  }
  if (
    !includesAny(latestUser.content, ["manuscript", "supplementary", "正文", "补充材料"]) ||
    !includesAny(latestCompletedAction.content, ["最终投稿包", "submission_final"])
  ) {
    return null;
  }

  const files = extractFileReferences(latestCompletedAction.content);
  const manuscriptFile = files.find((path) => pathKey(path).includes("manuscript/manuscript.docx"));
  const supplementaryFile = files.find((path) =>
    pathKey(path).includes("supplementary/supplementary.docx"),
  );

  if (!manuscriptFile || !supplementaryFile) {
    return null;
  }

  return {
    currentGoal: "最终投稿包已整理完成",
    whereToResume: `核对或使用最终投稿包：${manuscriptFile}；${supplementaryFile}`,
  };
}

function deriveFigureReviewResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
  canonicalFiles: string[] = [],
) {
  if (!latestUser || !latestCompletedAction) {
    return null;
  }
  if (
    !includesAny(latestUser.content, ["A4", "压盖", "字体", "图片", "标注"]) ||
    !includesAny(latestCompletedAction.content, ["A4", "可读性", "重排", "没有明显压盖", "最新版"])
  ) {
    return null;
  }

  const figureFiles = [
    ...extractFileReferences(latestCompletedAction.content),
    ...canonicalFiles,
  ].filter((path) => /\.(?:png|pdf|svg|tiff?)$/iu.test(path));
  if (figureFiles.length === 0) {
    return null;
  }
  const selectedFiles = uniquePathReferences(figureFiles).slice(0, 2).map(fileNameFromPath);

  return {
    currentGoal: "A4 版图件已重排完成",
    whereToResume: `核对或使用最新版 A4 图件：${selectedFiles.join("；")}（完整路径见 Canonical files）`,
  };
}

function deriveUiLayoutCompletionResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
) {
  if (!latestUser || !latestCompletedAction) {
    return null;
  }
  if (
    !includesAny(latestUser.content, ["数据预览", "预览部分", "大小位置", "大小", "位置"]) ||
    !includesAny(latestCompletedAction.content, ["固定", "稳定", "右侧面板", "重启"])
  ) {
    return null;
  }

  return {
    currentGoal: "数据预览固定布局已完成",
    whereToResume: "核对数据预览固定大小与位置；重点查看上传页空状态和有数据状态是否保持稳定。",
  };
}

function deriveTestingCompletionResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
) {
  if (!latestUser || !latestCompletedAction) {
    return null;
  }
  if (
    !includesAny(latestUser.content, ["完整测试", "测试下", "测完"]) ||
    !includesAny(latestCompletedAction.content, ["完整测完", "测试全过", "验证结果", "回归测试"])
  ) {
    return null;
  }

  return {
    currentGoal: "完整测试与导出修复已完成",
    whereToResume: "复核完整测试结果；重点查看导出页链路、已生成图表列表和新增回归测试。",
  };
}

function deriveGitCommitResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
) {
  if (!latestUser || !latestCompletedAction) {
    return null;
  }
  if (
    !includesAny(latestUser.content, ["git保存", "git 保存", "git提交", "git 提交", "提交一下"]) ||
    !includesAny(latestCompletedAction.content, ["已提交", "commit"])
  ) {
    return null;
  }

  const commitMatch = /commit\s*(?:是|:|：)?\s*`?([0-9a-f]{7,40})`?/iu.exec(
    latestCompletedAction.content,
  );
  const commitText = commitMatch ? ` ${commitMatch[1]}` : "";
  const remoteText = includesAny(latestCompletedAction.content, ["还没推远程", "未推远程", "not pushed"])
    ? "当前还没推远程。"
    : "按需确认是否需要推远程。";

  return {
    currentGoal: "本地 git 已提交",
    whereToResume: `从本地 commit${commitText} 之后继续；${remoteText}`,
  };
}

function deriveDuplicateCopyCleanupResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
) {
  if (!latestUser || !latestCompletedAction) {
    return null;
  }
  if (
    !includesAny(latestUser.content, ["重复", "箭头标", "箭头"]) ||
    !includesAny(latestCompletedAction.content, ["重复", "只保留", "不再重复", "已处理", "改成"])
  ) {
    return null;
  }

  return {
    currentGoal: "重复提示已清理",
    whereToResume: "核对数据概览重复提示；重点查看顶部蓝条和 AI 辅助摘要卡片是否不再重复说明同一件事。",
  };
}

function deriveGenericCompletedResume(
  latestUser: MessageCandidate | null,
  latestCompletedAction: MessageCandidate | null,
) {
  if (!latestUser || !latestCompletedAction || latestCompletedAction.index <= latestUser.index) {
    return null;
  }

  return {
    currentGoal: "最新请求已处理完成",
    whereToResume: "从最新完成动作继续；优先核对 Latest completed action 和 Canonical files，必要时再打开 Evidence。",
  };
}

function isObsoleteContextCandidate(
  candidate: MessageCandidate,
  latestCompletedAction: MessageCandidate | null,
  latestWorkline: MessageCandidate | null,
) {
  if (isProcessNoise(candidate.content)) {
    return false;
  }
  if (candidate.index === latestCompletedAction?.index || candidate.index === latestWorkline?.index) {
    return false;
  }
  if (isDeliveryMessage(candidate.content)) {
    return false;
  }
  return includesAny(candidate.content, OBSOLETE_CONTEXT_WORDS);
}

function isSystemishMessage(content: string) {
  return (
    content.startsWith("<environment_context>") ||
    content.startsWith("<permissions instructions>") ||
    content.startsWith("<apps_instructions>") ||
    content.startsWith("<skills_instructions>") ||
    content.startsWith("<plugins_instructions>") ||
    content.startsWith("<collaboration_mode>") ||
    content.startsWith("# AGENTS.md instructions")
  );
}

function messageCandidates(messages: ContinuationBriefMessage[]) {
  return messages
    .map((message, index) => ({
      message,
      index,
      content: cleanMessageContent(message.content),
    }))
    .filter((candidate) => candidate.content && !isSystemishMessage(candidate.content));
}

function roleOf(candidate: MessageCandidate) {
  return candidate.message.role || "message";
}

function messageLine(candidate: MessageCandidate, maxLength = 180) {
  return `${roleOf(candidate)}: ${truncateLine(candidate.content, maxLength)}`;
}

function lastMatching(
  candidates: MessageCandidate[],
  predicate: (candidate: MessageCandidate) => boolean,
) {
  for (let index = candidates.length - 1; index >= 0; index -= 1) {
    if (predicate(candidates[index])) {
      return candidates[index];
    }
  }
  return null;
}

function cleanPathReference(value: string) {
  return value
    .trim()
    .replace(/^[("'\[]+|[)"'\]，。；;:]+$/gu, "")
    .replace(/^[/\\]([A-Za-z]:[\\/])/u, "$1");
}

function uniquePathReferences(paths: string[]) {
  const uniquePaths: string[] = [];
  const seenPaths = new Set<string>();
  for (const path of paths) {
    if (!path.includes("/") && !path.includes("\\")) {
      continue;
    }
    const key = pathKey(path);
    if (seenPaths.has(key)) {
      continue;
    }
    seenPaths.add(key);
    uniquePaths.push(path);
  }
  return uniquePaths;
}

function fileNameFromPath(path: string) {
  const normalizedPath = path.replace(/\\/g, "/");
  return normalizedPath.slice(normalizedPath.lastIndexOf("/") + 1) || path;
}

function extractFileReferences(content: string) {
  const paths: string[] = [];
  const fileExtensionPattern = /\.(?:md|docx|xlsx|csv|py|r|tsx?|jsx?|png|pdf|drawio)$/iu;
  const markdownLinkPattern = /\[[^\]]+\]\((?:<([^>]+)>|([^)]+))\)/gu;
  const backtickPattern = /`([^`]+\.(?:md|docx|xlsx|csv|py|r|tsx?|jsx?|png|pdf|drawio))`/giu;
  const loosePattern = /(?:[A-Za-z]:)?[^\s`，。；;:]*[\\/][^\s`，。；;:]+?\.(?:md|docx|xlsx|csv|py|r|tsx?|jsx?|png|pdf|drawio)/giu;

  for (const match of content.matchAll(markdownLinkPattern)) {
    const path = cleanPathReference(match[1] || match[2] || "");
    if (fileExtensionPattern.test(path)) {
      paths.push(path);
    }
  }
  for (const match of content.matchAll(backtickPattern)) {
    paths.push(cleanPathReference(match[1]));
  }
  for (const match of content.matchAll(loosePattern)) {
    paths.push(cleanPathReference(match[0]));
  }

  return uniquePathReferences(paths);
}

function pathKey(path: string) {
  return path.replace(/\\/g, "/").toLowerCase();
}

function isEphemeralFileReference(path: string) {
  return includesAny(path.replace(/\\/g, "/"), EPHEMERAL_FILE_WORDS);
}

function rankCanonicalFiles(
  fileChanges: ContinuationBriefFileChange[],
  candidates: MessageCandidate[],
) {
  const latestUser = lastMatching(
    candidates,
    (candidate) => roleOf(candidate).toLowerCase() === "user",
  );
  const latestCompletedAction = lastMatching(
    candidates,
    (candidate) => isCompletedAssistantAction(candidate),
  );
  const pathScores = new Map<string, { path: string; score: number }>();

  const addPath = (path: string, score: number) => {
    const cleanedPath = cleanPathReference(path);
    if (!cleanedPath || isEphemeralFileReference(cleanedPath)) {
      return;
    }
    const key = pathKey(cleanedPath);
    const existing = pathScores.get(key);
    if (!existing || score > existing.score) {
      pathScores.set(key, { path: cleanedPath, score });
    }
  };

  fileChanges.forEach((change, index) => {
    addPath(change.path, index);
  });

  candidates.forEach((candidate) => {
    let score = 500 + candidate.index;
    if (candidate.index === latestUser?.index) {
      score += 500;
    }
    if (candidate.index === latestCompletedAction?.index) {
      score += 1000;
    }
    extractFileReferences(candidate.content).forEach((path) => addPath(path, score));
  });

  return Array.from(pathScores.values())
    .filter(Boolean)
    .map((item) => {
      let score = item.score;
      const path = item.path;
      if (includesAny(path, CANONICAL_FILE_WORDS)) {
        score += 100;
      }
      if (includesAny(path, OBSOLETE_CONTEXT_WORDS) && !includesAny(path, ACTIVE_WORKLINE_WORDS)) {
        score -= 60;
      }
      if (/\.(md|docx|xlsx|csv|py|r|tsx?|jsx?|png|pdf)$/iu.test(path)) {
        score += 10;
      }
      return { path, score };
    })
    .sort((left, right) => right.score - left.score || left.path.localeCompare(right.path))
    .map((item) => item.path)
    .slice(0, 8);
}

function scoreEvidence(candidate: MessageCandidate, latestUserIndex: number | null) {
  let score = candidate.index;
  const role = roleOf(candidate).toLowerCase();

  if (candidate.index === latestUserIndex) {
    score += 120;
  }
  if (role === "assistant" && includesAny(candidate.content, COMPLETED_ACTION_WORDS)) {
    score += 100;
  }
  if (includesAny(candidate.content, ACTIVE_WORKLINE_WORDS)) {
    score += 80;
  }
  if (includesAny(candidate.content, OBSOLETE_CONTEXT_WORDS)) {
    score += 60;
  }
  if (includesAny(candidate.content, RISK_WORDS)) {
    score += 40;
  }
  if (/[\\/].+\.(md|docx|xlsx|csv|py|r|tsx?|jsx?|png|pdf)/iu.test(candidate.content)) {
    score += 40;
  }
  if (isLowSignal(candidate.content)) {
    score -= 120;
  }

  return score;
}

function isEvidenceFillerCandidate(
  candidate: MessageCandidate,
  latestUserIndex: number | null,
  latestCompletedAction: MessageCandidate | null,
  latestWorkline: MessageCandidate | null,
) {
  if (isProcessNoise(candidate.content)) {
    return false;
  }
  if (latestUserIndex === null || candidate.index >= latestUserIndex) {
    return true;
  }

  return (
    isCompletedAssistantAction(candidate) ||
    isObsoleteContextCandidate(candidate, latestCompletedAction, latestWorkline) ||
    includesAny(candidate.content, ACTIVE_WORKLINE_WORDS) ||
    includesAny(candidate.content, RISK_WORDS) ||
    extractFileReferences(candidate.content).length > 0
  );
}

function selectEvidence(candidates: MessageCandidate[]) {
  const latestUser = lastMatching(
    candidates,
    (candidate) => roleOf(candidate).toLowerCase() === "user",
  );
  const latestUserIndex = latestUser?.index ?? null;
  const latestCompletedAction = lastMatching(
    candidates,
    (candidate) => isCompletedAssistantAction(candidate),
  );
  const latestWorkline = lastMatching(candidates, (candidate) =>
    includesAny(candidate.content, ACTIVE_WORKLINE_WORDS),
  );
  const latestObsoleteContext = lastMatching(candidates, (candidate) =>
    isObsoleteContextCandidate(candidate, latestCompletedAction, latestWorkline),
  );
  const selected: MessageCandidate[] = [];
  const maxObsoleteEvidenceCount = 1;

  const selectedObsoleteCount = () =>
    selected.filter((item) =>
      isObsoleteContextCandidate(item, latestCompletedAction, latestWorkline),
    ).length;

  const addEvidence = (candidate: MessageCandidate | null) => {
    if (!candidate) {
      return;
    }
    if (selected.some((item) => item.index === candidate.index)) {
      return;
    }
    if (
      isObsoleteContextCandidate(candidate, latestCompletedAction, latestWorkline) &&
      selectedObsoleteCount() >= maxObsoleteEvidenceCount
    ) {
      return;
    }
    selected.push(candidate);
  };

  addEvidence(latestUser);
  addEvidence(latestCompletedAction);
  addEvidence(latestObsoleteContext);

  const ranked = candidates
    .filter((candidate) =>
      isEvidenceFillerCandidate(candidate, latestUserIndex, latestCompletedAction, latestWorkline),
    )
    .sort((left, right) => {
      const scoreDiff = scoreEvidence(right, latestUserIndex) - scoreEvidence(left, latestUserIndex);
      return scoreDiff || right.index - left.index;
    });

  for (const candidate of ranked) {
    if (selected.length >= 5) {
      break;
    }
    addEvidence(candidate);
  }

  return selected
    .sort((left, right) => left.index - right.index)
    .map((candidate) => messageLine(candidate, 220));
}

function deriveResumeState(conversation: ContinuationBriefConversation) {
  const candidates = messageCandidates(conversation.messages);
  const summary = cleanLine(conversation.summary);
  const latestUser = lastMatching(
    candidates,
    (candidate) => roleOf(candidate).toLowerCase() === "user",
  );
  const latestAssistant = lastMatching(
    candidates,
    (candidate) => roleOf(candidate).toLowerCase() === "assistant",
  );
  const latestCompletedAction = lastMatching(
    candidates,
    (candidate) => isCompletedAssistantAction(candidate),
  );
  const latestWorkline = lastMatching(candidates, (candidate) =>
    includesAny(candidate.content, ACTIVE_WORKLINE_WORDS),
  );
  const hasStateShift = candidates.some(
    (candidate) =>
      includesAny(candidate.content, ACTIVE_WORKLINE_WORDS) ||
      includesAny(candidate.content, OBSOLETE_CONTEXT_WORDS),
  );
  const userMessageCount = candidates.filter(
    (candidate) => roleOf(candidate).toLowerCase() === "user",
  ).length;
  const obsoleteContexts = candidates
    .filter((candidate) =>
      isObsoleteContextCandidate(candidate, latestCompletedAction, latestWorkline),
    )
    .slice(-3)
    .map((candidate) => messageLine(candidate, 220));
  const canonicalFiles = rankCanonicalFiles(conversation.file_changes, candidates);
  const handoffResume = deriveHandoffResume(latestUser, latestCompletedAction);
  const submissionPackageResume = deriveSubmissionPackageResume(
    latestUser,
    latestCompletedAction || latestAssistant,
  );
  const figureReviewResume = deriveFigureReviewResume(
    latestUser,
    latestCompletedAction || latestAssistant,
    canonicalFiles,
  );
  const uiLayoutCompletionResume = deriveUiLayoutCompletionResume(
    latestUser,
    latestCompletedAction || latestAssistant,
  );
  const testingCompletionResume = deriveTestingCompletionResume(
    latestUser,
    latestCompletedAction || latestAssistant,
  );
  const gitCommitResume = deriveGitCommitResume(
    latestUser,
    latestCompletedAction || latestAssistant,
  );
  const duplicateCopyCleanupResume = deriveDuplicateCopyCleanupResume(
    latestUser,
    latestCompletedAction || latestAssistant,
  );
  const genericCompletedResume = deriveGenericCompletedResume(latestUser, latestCompletedAction);
  const genericCompletedWorkline = genericCompletedResume ? latestCompletedAction?.content : null;

  return {
    currentGoal:
      handoffResume
        ? truncateLine(handoffResume.currentGoal, 180)
        : submissionPackageResume
        ? truncateLine(submissionPackageResume.currentGoal, 180)
        : figureReviewResume
        ? truncateLine(figureReviewResume.currentGoal, 180)
        : uiLayoutCompletionResume
        ? truncateLine(uiLayoutCompletionResume.currentGoal, 180)
        : testingCompletionResume
        ? truncateLine(testingCompletionResume.currentGoal, 180)
        : gitCommitResume
        ? truncateLine(gitCommitResume.currentGoal, 180)
        : duplicateCopyCleanupResume
        ? truncateLine(duplicateCopyCleanupResume.currentGoal, 180)
        : genericCompletedResume
        ? truncateLine(genericCompletedResume.currentGoal, 180)
        : (hasStateShift || userMessageCount > 1) && latestUser
        ? truncateLine(latestUser.content, 180)
        : truncateLine(summary || latestUser?.content || conversation.id, 180),
    currentWorkline: truncateLine(
      latestWorkline?.content || genericCompletedWorkline || summary || conversation.id,
      220,
    ),
    latestCompletedAction: truncateLine(
      latestCompletedAction?.content || latestAssistant?.content || "No completed action was found in the captured messages.",
      240,
    ),
    whereToResume: truncateLine(
      handoffResume?.whereToResume ||
        submissionPackageResume?.whereToResume ||
        figureReviewResume?.whereToResume ||
        uiLayoutCompletionResume?.whereToResume ||
        testingCompletionResume?.whereToResume ||
        gitCommitResume?.whereToResume ||
        duplicateCopyCleanupResume?.whereToResume ||
        genericCompletedResume?.whereToResume ||
        latestUser?.content ||
        summary ||
        "Continue from the latest available project context.",
      220,
    ),
    obsoleteContexts,
    evidence: selectEvidence(candidates),
  };
}

export function buildContinuationBriefPrompt({
  repoRoot,
  conversation,
  checkpointId,
  handoffId,
}: ContinuationBriefInput) {
  const source = conversationKey(conversation);
  const resumeState = deriveResumeState(conversation);
  const resumeCommand = cleanLine(conversation.resume_command);
  const canonicalFiles = rankCanonicalFiles(
    conversation.file_changes,
    messageCandidates(conversation.messages),
  );
  const rawTokenEstimate = estimateTokens(
    conversation.messages.map((message) => message.content).join("\n"),
  );

  const lines = [
    "# Continuation Brief",
    "",
    "Use ChatMem to continue this project from a compact, source-backed brief.",
    "Treat the original conversation as evidence to inspect on demand, not as startup context.",
    "",
    "## Scope",
    `- repo: ${repoRoot}`,
    `- conversation: ${source}`,
    `- source agent: ${conversation.source_agent}`,
    `- Current goal: ${resumeState.currentGoal}`,
  ];

  if (resumeCommand) {
    lines.push(`- resume command: ${resumeCommand}`);
  }
  if (checkpointId) {
    lines.push(`- checkpoint: ${checkpointId}`);
  }
  if (handoffId) {
    lines.push(`- handoff: ${handoffId}`);
  }

  lines.push(
    "",
    "## Current workline",
    `- ${resumeState.currentWorkline}`,
    "",
    "## Latest completed action",
    `- ${resumeState.latestCompletedAction}`,
    "",
    "## Where to resume",
    `- Start from the latest user request: ${resumeState.whereToResume}`,
    "- Treat older or archived work as background unless focused evidence proves it is active again.",
    "",
    "## Canonical files",
    ...listLines(canonicalFiles, "No file changes were captured for this conversation."),
    "",
    "## Obsolete or archived context",
    ...listLines(resumeState.obsoleteContexts, "No obsolete or archived context was detected."),
    "",
    "## Evidence",
    `- Evidence source: ${source}`,
    ...listLines(resumeState.evidence, "Use search_repo_history before expanding the conversation."),
    "",
    "## Token posture:",
    `- Estimated raw transcript tokens: ${rawTokenEstimate}`,
    "- Start from this brief instead of the raw transcript.",
    "- Open focused evidence windows only when the brief and project context are insufficient.",
    "",
    "## Continuation Protocol",
    '1. First call get_project_context with intent="continue_work" and limit=3.',
    "2. Prefer approved memories, recent checkpoints/handoffs, wiki, and relevant_history summaries.",
    "3. If evidence is missing, call search_repo_history with limit<=3.",
    "4. Read the original conversation only through read_history_conversation for a focused window.",
    "5. Do not replay the full transcript or tool logs unless the focused evidence is insufficient.",
  );

  return lines.join("\n");
}
