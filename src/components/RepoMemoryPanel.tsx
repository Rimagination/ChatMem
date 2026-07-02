import { useEffect, useRef } from "react";
import type { ApprovedMemory } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type RepoMemoryPanelProps = {
  memories: ApprovedMemory[];
  loading: boolean;
  locale: Locale;
  onRetire: (memoryId: string) => void;
  onRetireMany?: (memoryIds: string[]) => void;
  autoFocusFirstMemory?: boolean;
  onAutoFocusHandled?: () => void;
};

function formatFreshnessLabel(status: string, locale: Locale) {
  const isEnglish = locale === "en";

  if (status === "fresh") {
    return isEnglish ? "fresh" : "\u6709\u6548";
  }

  if (status === "needs_review") {
    return isEnglish ? "needs agent review" : "\u9700 agent \u5904\u7406";
  }

  if (status === "stale") {
    return isEnglish ? "stale" : "\u8fc7\u671f";
  }

  return isEnglish ? "unknown" : "\u672a\u77e5";
}

function isAutoQuarantinedMemory(memory: ApprovedMemory) {
  return memory.verified_by === "auto_quarantine" && memory.freshness_status === "needs_review";
}

function formatStatusLabel(status: string, locale: Locale) {
  const isEnglish = locale === "en";

  if (status === "active") {
    return isEnglish ? "active" : "\u542f\u7528";
  }

  if (status === "retired") {
    return isEnglish ? "deleted" : "\u5df2\u5220\u9664";
  }

  return status;
}

function formatVerifiedLabel(memory: ApprovedMemory, locale: Locale) {
  const isEnglish = locale === "en";

  if (isAutoQuarantinedMemory(memory)) {
    return isEnglish ? "Waiting for agent handling" : "\u7b49\u5f85 agent \u5904\u7406";
  }

  if (!memory.last_verified_at) {
    return isEnglish
      ? "Last verified: not yet verified"
      : "\u6700\u8fd1\u9a8c\u8bc1\uff1a\u5c1a\u672a\u9a8c\u8bc1";
  }

  const byline = memory.verified_by ? ` by ${memory.verified_by}` : "";
  if (isEnglish) {
    return `Last verified: ${memory.last_verified_at}${byline}`;
  }

  const verifier = memory.verified_by ? `\uff0c\u9a8c\u8bc1\u8005\uff1a${memory.verified_by}` : "";
  return `\u6700\u8fd1\u9a8c\u8bc1\uff1a${memory.last_verified_at}${verifier}`;
}

export default function RepoMemoryPanel({
  memories,
  loading,
  locale,
  onRetire,
  onRetireMany,
  autoFocusFirstMemory = false,
  onAutoFocusHandled,
}: RepoMemoryPanelProps) {
  const firstMemoryRef = useRef<HTMLElement | null>(null);
  const autoFocusHandledRef = useRef(false);
  const isEnglish = locale === "en";
  const copy = {
    empty: isEnglish
      ? "No startup rules yet."
      : "\u6682\u65e0\u542f\u52a8\u89c4\u5219\u3002",
    heading: isEnglish ? "Startup Rules" : "\u542f\u52a8\u89c4\u5219",
    subtitle: isEnglish
      ? "Review or delete existing rules here. Supported agents handle creating and updating rules."
      : "你可以在这里查看或删除已有规则。新增和更新规则由支持的 Agent 完成。",
    freshnessScore: isEnglish ? "Freshness score" : "\u65b0\u9c9c\u5ea6\u5206\u6570",
    retire: isEnglish ? "Delete rule" : "\u5220\u9664\u89c4\u5219",
    retireLegacyAutoRules: (count: number) =>
      isEnglish
        ? `Delete low-confidence rules ${count}`
        : `删除低置信度规则 ${count}`,
    autoQuarantineNote: isEnglish
      ? "This older rule has too little source context. Ask an agent to update it before relying on it."
      : "这条旧规则的来源不足。依赖它之前，请让 Agent 更新一次。",
  };

  useEffect(() => {
    if (!autoFocusFirstMemory) {
      autoFocusHandledRef.current = false;
      return;
    }

    if (autoFocusHandledRef.current || loading) {
      return;
    }

    if (memories.length === 0) {
      autoFocusHandledRef.current = true;
      onAutoFocusHandled?.();
      return;
    }

    const firstMemoryCard = firstMemoryRef.current;
    if (!firstMemoryCard) {
      return;
    }

    firstMemoryCard.scrollIntoView({ block: "nearest" });
    firstMemoryCard.focus();
    autoFocusHandledRef.current = true;
    onAutoFocusHandled?.();
  }, [autoFocusFirstMemory, loading, memories.length, onAutoFocusHandled]);

  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  if (memories.length === 0) {
    return (
      <section className="memory-panel">
        <div className="empty-state">
          <div className="empty-state-icon">M</div>
          <div className="empty-state-text">{copy.empty}</div>
        </div>
      </section>
    );
  }

  const autoQuarantinedMemoryIds = memories
    .filter(isAutoQuarantinedMemory)
    .map((memory) => memory.memory_id);
  const showBulkRetireLegacyAutoRules =
    Boolean(onRetireMany) && autoQuarantinedMemoryIds.length > 0;

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <div className="memory-panel-title">
          <h3>{copy.heading}</h3>
          <p>{copy.subtitle}</p>
        </div>
        {showBulkRetireLegacyAutoRules ? (
          <button
            type="button"
            className="btn btn-danger memory-batch-action"
            onClick={() => onRetireMany?.(autoQuarantinedMemoryIds)}
          >
            {copy.retireLegacyAutoRules(autoQuarantinedMemoryIds.length)}
          </button>
        ) : null}
      </div>
      <div className="memory-card-list">
        {memories.map((memory, index) => {
          const freshnessState = memory.freshness_status || "unknown";
          const freshnessScore = Number.isFinite(memory.freshness_score) ? memory.freshness_score : 0;
          const isFirstCard = index === 0;
          const isAutoQuarantined = isAutoQuarantinedMemory(memory);

          return (
            <article
              key={memory.memory_id}
              ref={isFirstCard ? firstMemoryRef : undefined}
              className="memory-card"
              tabIndex={isFirstCard ? -1 : undefined}
            >
            <div className="memory-card-header">
              <div>
                <strong>{memory.title}</strong>
                <div className="memory-card-kind">{memory.kind}</div>
              </div>
              <div className="memory-card-badges">
                <span className={`memory-freshness memory-freshness-${freshnessState}`}>
                  {formatFreshnessLabel(freshnessState, locale)}
                </span>
                <span className={`memory-status memory-status-${memory.status}`}>
                  {formatStatusLabel(memory.status, locale)}
                </span>
              </div>
            </div>
            <div className="memory-card-value">{memory.value}</div>
            <p className="memory-card-copy">{memory.usage_hint}</p>
            {isAutoQuarantined && <div className="memory-card-warning">{copy.autoQuarantineNote}</div>}
            <div className="memory-card-meta">
              <span>{formatVerifiedLabel(memory, locale)}</span>
              {!isAutoQuarantined && (
                <span>
                  {copy.freshnessScore}: {freshnessScore.toFixed(2)}
                </span>
              )}
            </div>
            {memory.evidence_refs.length > 0 && (
              <div className="memory-evidence-list">
                {memory.evidence_refs.slice(0, 2).map((evidence, index) => (
                  <div key={`${memory.memory_id}-evidence-${index}`} className="memory-evidence-item">
                    {evidence.excerpt}
                  </div>
                ))}
              </div>
            )}
            <div className="memory-card-actions">
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => onRetire(memory.memory_id)}
              >
                {copy.retire}
              </button>
            </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
