import { useEffect, useMemo, useState } from "react";
import type { Locale } from "../i18n/types";
import type { LibraryRecord, LibraryRecordKind } from "../library/model";
import { formatDateTime } from "../utils/dateUtils";

type LibraryFilter = "all" | LibraryRecordKind;

type LibraryPanelProps = {
  locale: Locale;
  repoLabel: string;
  repoPath: string;
  records: LibraryRecord[];
  onOpenRecord: (record: LibraryRecord) => void;
};

const FILTER_ORDER: LibraryFilter[] = [
  "all",
  "conversation",
  "memory",
  "checkpoint",
  "handoff",
  "run",
  "artifact",
  "episode",
];

function getFilterLabel(filter: LibraryFilter, locale: Locale) {
  if (locale === "en") {
    switch (filter) {
      case "all":
        return "All";
      case "conversation":
        return "Conversations";
      case "memory":
        return "Memories";
      case "checkpoint":
        return "Checkpoints";
      case "handoff":
        return "Handoffs";
      case "run":
        return "Runs";
      case "artifact":
        return "Artifacts";
      case "episode":
        return "Episodes";
    }
  }

  switch (filter) {
    case "all":
      return "\u5168\u90e8";
    case "conversation":
      return "\u5bf9\u8bdd";
    case "memory":
      return "\u8bb0\u5fc6";
    case "checkpoint":
      return "\u68c0\u67e5\u70b9";
    case "handoff":
      return "\u4ea4\u63a5";
    case "run":
      return "\u8fd0\u884c";
    case "artifact":
      return "\u4ea7\u7269";
    case "episode":
      return "\u9636\u6bb5";
  }
}

function getKindBadgeLabel(kind: LibraryRecordKind, locale: Locale) {
  if (locale === "en") {
    switch (kind) {
      case "conversation":
        return "Conversation";
      case "memory":
        return "Memory";
      case "checkpoint":
        return "Checkpoint";
      case "handoff":
        return "Handoff";
      case "run":
        return "Run";
      case "artifact":
        return "Artifact";
      case "episode":
        return "Episode";
    }
  }

  switch (kind) {
    case "conversation":
      return "\u5bf9\u8bdd";
    case "memory":
      return "\u8bb0\u5fc6";
    case "checkpoint":
      return "\u68c0\u67e5\u70b9";
    case "handoff":
      return "\u4ea4\u63a5";
    case "run":
      return "\u8fd0\u884c";
    case "artifact":
      return "\u4ea7\u7269";
    case "episode":
      return "\u9636\u6bb5";
  }
}

export default function LibraryPanel({
  locale,
  repoLabel,
  repoPath,
  records,
  onOpenRecord,
}: LibraryPanelProps) {
  const [activeFilter, setActiveFilter] = useState<LibraryFilter>("all");

  useEffect(() => {
    setActiveFilter("all");
  }, [repoPath]);

  const counts = useMemo(() => {
    const nextCounts = new Map<LibraryFilter, number>();
    FILTER_ORDER.forEach((filter) => {
      nextCounts.set(
        filter,
        filter === "all" ? records.length : records.filter((record) => record.kind === filter).length,
      );
    });
    return nextCounts;
  }, [records]);

  const visibleRecords = useMemo(
    () => (activeFilter === "all" ? records : records.filter((record) => record.kind === activeFilter)),
    [activeFilter, records],
  );

  const copy =
    locale === "en"
      ? {
          eyebrow: "Current repo library",
          title: "Library",
          subtitle:
            "Keep the current repository's conversations, memory, checkpoints, handoffs, runs, and artifacts in one item layer.",
          open: "Open",
          location: "Repository",
          empty: "No items match this filter yet.",
        }
      : {
          eyebrow: "\u5f53\u524d\u4ed3\u5e93\u8d44\u6599\u5e93",
          title: "\u8d44\u6599\u5e93",
          subtitle:
            "\u628a\u5f53\u524d\u4ed3\u5e93\u7684\u5bf9\u8bdd\u3001\u8bb0\u5fc6\u3001\u68c0\u67e5\u70b9\u3001\u4ea4\u63a5\u3001\u8fd0\u884c\u548c\u4ea7\u7269\u653e\u5728\u540c\u4e00\u5c42\u6761\u76ee\u91cc\u67e5\u770b\u3002",
          open: "\u6253\u5f00",
          location: "\u4ed3\u5e93",
          empty: "\u8fd9\u4e2a\u7b5b\u9009\u4e0b\u8fd8\u6ca1\u6709\u6761\u76ee\u3002",
        };

  return (
    <section className="task-panel library-panel">
      <div className="task-panel-header">
        <div>
          <span className="task-panel-label">{copy.eyebrow}</span>
          <h2>{copy.title}</h2>
          <p>{copy.subtitle}</p>
        </div>
      </div>

      <div className="library-panel-meta">
        <span className="meta-label">{copy.location}</span>
        <span className="meta-value">{repoLabel}</span>
        <span className="meta-value is-muted">{repoPath}</span>
      </div>

      <div className="library-filter-row">
        {FILTER_ORDER.map((filter) => {
          const label = getFilterLabel(filter, locale);
          const count = counts.get(filter) ?? 0;
          const buttonLabel = `${label} (${count})`;
          return (
            <button
              key={filter}
              type="button"
              className={`library-filter-chip ${activeFilter === filter ? "active" : ""}`}
              onClick={() => setActiveFilter(filter)}
              aria-pressed={activeFilter === filter}
            >
              {buttonLabel}
            </button>
          );
        })}
      </div>

      {visibleRecords.length === 0 ? (
        <div className="inline-empty-state">
          <div className="inline-empty-body">{copy.empty}</div>
        </div>
      ) : (
        <div className="library-record-list">
          {visibleRecords.map((record) => (
            <article key={`${record.kind}-${record.id}`} className="library-record-card">
              <div className="library-record-main">
                <div className="library-record-topline">
                  <span className={`library-kind-badge library-kind-${record.kind}`}>
                    {getKindBadgeLabel(record.kind, locale)}
                  </span>
                  {record.status ? <span className="timeline-pill">{record.status}</span> : null}
                </div>
                <strong>{record.title}</strong>
                <p>{record.subtitle}</p>
                <span className="library-record-timestamp">{formatDateTime(record.timestamp)}</span>
              </div>
              <button type="button" className="btn btn-secondary" onClick={() => onOpenRecord(record)}>
                {copy.open}
              </button>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
