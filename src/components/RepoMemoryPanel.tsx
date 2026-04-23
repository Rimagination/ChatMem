import { useEffect, useRef } from "react";
import type { ApprovedMemory } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type RepoMemoryPanelProps = {
  memories: ApprovedMemory[];
  loading: boolean;
  locale: Locale;
  onReverify: (memoryId: string) => void;
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

function formatVerifiedLabel(memory: ApprovedMemory, locale: Locale) {
  const isEnglish = locale === "en";

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
  autoFocusFirstMemory = false,
  onAutoFocusHandled,
}: RepoMemoryPanelProps) {
  const firstMemoryRef = useRef<HTMLElement | null>(null);
  const autoFocusHandledRef = useRef(false);
  const isEnglish = locale === "en";
  const copy = {
    empty: isEnglish
      ? "No approved repository memory yet."
      : "\u6682\u65e0\u5df2\u6279\u51c6\u7684\u4ed3\u5e93\u8bb0\u5fc6\u3002",
    heading: isEnglish ? "Repo Memory" : "\u4ed3\u5e93\u8bb0\u5fc6",
    subtitle: isEnglish
      ? "Approved repository memory that can be used for startup context and handoffs."
      : "\u5df2\u6279\u51c6\u7684\u4ed3\u5e93\u8bb0\u5fc6\uff0c\u53ef\u7528\u4e8e\u542f\u52a8\u4e0a\u4e0b\u6587\u548c\u4ea4\u63a5\u3002",
    freshnessScore: isEnglish ? "Freshness score" : "\u65b0\u9c9c\u5ea6\u5206\u6570",
    reverify: isEnglish ? "Re-verify" : "\u91cd\u65b0\u9a8c\u8bc1",
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

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>{copy.heading}</h3>
        <p>{copy.subtitle}</p>
      </div>
      <div className="memory-card-list">
        {memories.map((memory, index) => {
          const freshnessState = memory.freshness_status || "unknown";
          const freshnessScore = Number.isFinite(memory.freshness_score) ? memory.freshness_score : 0;
          const isFirstCard = index === 0;

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
                <span className={`memory-status memory-status-${memory.status}`}>{memory.status}</span>
              </div>
            </div>
            <div className="memory-card-value">{memory.value}</div>
            <p className="memory-card-copy">{memory.usage_hint}</p>
            <div className="memory-card-meta">
              <span>{formatVerifiedLabel(memory, locale)}</span>
              <span>
                {copy.freshnessScore}: {freshnessScore.toFixed(2)}
              </span>
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
                {copy.reverify}
              </button>
            </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
