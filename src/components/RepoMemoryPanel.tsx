import { useEffect, useRef } from "react";
import type { ApprovedMemory } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type RepoMemoryPanelProps = {
  memories: ApprovedMemory[];
  loading: boolean;
  locale: Locale;
  onReverify: (memoryId: string) => void;
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
    return isEnglish ? "needs review" : "\u9700\u590d\u6838";
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
    return isEnglish ? "retired" : "\u505c\u7528";
  }

  return status;
}

function formatVerifiedLabel(memory: ApprovedMemory, locale: Locale) {
  const isEnglish = locale === "en";

  if (isAutoQuarantinedMemory(memory)) {
    return isEnglish ? "Waiting for human confirmation" : "\u7b49\u5f85\u4eba\u5de5\u786e\u8ba4";
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
  onReverify,
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
      ? "No approved startup rules yet."
      : "\u6682\u65e0\u5df2\u6279\u51c6\u7684\u542f\u52a8\u89c4\u5219\u3002",
    heading: isEnglish ? "Approved Startup Rules" : "\u5df2\u6279\u51c6\u542f\u52a8\u89c4\u5219",
    subtitle: isEnglish
      ? "These durable rules are injected at task startup. Local history stays available separately through search evidence."
      : "\u8fd9\u4e9b\u662f\u4efb\u52a1\u5f00\u59cb\u65f6\u8981\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219\u3002\u672c\u5730\u5386\u53f2\u4ecd\u7136\u901a\u8fc7\u68c0\u7d22\u5355\u72ec\u63d0\u4f9b\u8bc1\u636e\u3002",
    freshnessScore: isEnglish ? "Freshness score" : "\u65b0\u9c9c\u5ea6\u5206\u6570",
    confirmValid: isEnglish ? "Confirm still valid" : "\u786e\u8ba4\u4ecd\u6709\u6548",
    retire: isEnglish ? "Retire rule" : "\u505c\u7528\u89c4\u5219",
    retireLegacyAutoRules: (count: number) =>
      isEnglish
        ? `Retire legacy auto rules ${count}`
        : `\u5168\u90e8\u505c\u7528\u65e7\u7248\u81ea\u52a8\u89c4\u5219 ${count}`,
    autoQuarantineNote: isEnglish
      ? "Legacy auto-extracted rule with weak evidence. It will be used as a startup rule only after you confirm it."
      : "\u65e7\u7248\u81ea\u52a8\u62bd\u53d6\uff0c\u8bc1\u636e\u4e0d\u8db3\uff1b\u786e\u8ba4\u540e\u624d\u4f1a\u4f5c\u4e3a\u542f\u52a8\u89c4\u5219\u4f7f\u7528\u3002",
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
                className="btn btn-secondary"
                onClick={() => onReverify(memory.memory_id)}
              >
                {copy.confirmValid}
              </button>
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
