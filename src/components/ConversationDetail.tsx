import { useEffect, useState } from "react";
import { formatDateTime } from "../utils/dateUtils";

interface Message {
  id: string;
  timestamp: string;
  role: string;
  content: string;
  tool_calls: ToolCall[];
  metadata: Record<string, unknown>;
}

interface ToolCall {
  name: string;
  input: unknown;
  output: string | null;
  status: string;
}

interface FileChange {
  path: string;
  change_type: string;
  timestamp: string;
  message_id: string;
}

interface Conversation {
  id: string;
  source_agent: string;
  project_dir: string;
  created_at: string;
  updated_at: string;
  summary: string | null;
  messages: Message[];
  file_changes: FileChange[];
}

interface ConversationDetailProps {
  conversation: Conversation;
}

const COLLAPSIBLE_MESSAGE_LENGTH = 280;

function formatRole(role: string) {
  switch (role) {
    case "user":
      return "\u7528\u6237";
    case "assistant":
      return "\u52a9\u624b";
    case "system":
      return "\u7cfb\u7edf";
    default:
      return role;
  }
}

function changeLabel(changeType: string) {
  switch (changeType) {
    case "created":
      return "\u65b0\u589e";
    case "modified":
      return "\u4fee\u6539";
    case "deleted":
      return "\u5220\u9664";
    default:
      return changeType;
  }
}

function shouldCollapseMessage(message: Message) {
  return message.role === "assistant" && message.content.trim().length > COLLAPSIBLE_MESSAGE_LENGTH;
}

function ConversationDetail({ conversation }: ConversationDetailProps) {
  const [expandedMessages, setExpandedMessages] = useState<Record<string, boolean>>({});
  const [expandedTools, setExpandedTools] = useState<Record<string, boolean>>({});

  useEffect(() => {
    setExpandedMessages({});
    setExpandedTools({});
  }, [conversation.id]);

  const toolCallCount = conversation.messages.reduce(
    (count, message) => count + message.tool_calls.length,
    0,
  );

  return (
    <div className="conversation-detail">
      <div className="stats">
        <div className="stat-item">
          <div className="stat-value">{conversation.messages.length}</div>
          <div className="stat-label">{"\u6d88\u606f\u6570"}</div>
        </div>
        <div className="stat-item">
          <div className="stat-value">{conversation.file_changes.length}</div>
          <div className="stat-label">{"\u6587\u4ef6\u53d8\u66f4"}</div>
        </div>
        <div className="stat-item">
          <div className="stat-value">{toolCallCount}</div>
          <div className="stat-label">{"\u5de5\u5177\u8c03\u7528"}</div>
        </div>
      </div>

      <div className="message-list">
        {conversation.messages.map((message) => {
          const hasContent = Boolean(message.content?.trim());
          const collapsible = shouldCollapseMessage(message);
          const isExpanded = expandedMessages[message.id] ?? false;

          return (
            <article key={message.id} className={`message message-${message.role}`}>
              <div className="message-shell">
                <div className="message-header">
                  <span className="message-role">{formatRole(message.role)}</span>
                  <span className="message-timestamp">{formatDateTime(message.timestamp)}</span>
                </div>

                {hasContent && (
                  <div className="message-bubble">
                    <div
                      className={`message-content ${
                        collapsible ? (isExpanded ? "is-expanded" : "is-collapsed") : ""
                      }`.trim()}
                    >
                      {message.content}
                    </div>

                    {collapsible && (
                      <button
                        type="button"
                        className="message-toggle"
                        onClick={() =>
                          setExpandedMessages((current) => ({
                            ...current,
                            [message.id]: !isExpanded,
                          }))
                        }
                      >
                        {isExpanded ? "\u6536\u8d77" : "\u5c55\u5f00\u5168\u6587"}
                      </button>
                    )}
                  </div>
                )}

                {message.tool_calls.length > 0 && (
                  <div className="tool-calls">
                    {message.tool_calls.map((toolCall, index) => {
                      const toolKey = `${message.id}-${index}`;
                      const toolExpanded = expandedTools[toolKey] ?? false;

                      return (
                        <div key={`${message.id}-${toolCall.name}-${index}`} className="tool-call">
                          <div className="tool-call-topline">
                            <span className="tool-call-name">{toolCall.name}</span>
                            <div className="tool-call-actions">
                              <span className={`tool-call-status tool-call-status-${toolCall.status}`}>
                                {toolCall.status === "success" ? "\u6210\u529f" : "\u5f02\u5e38"}
                              </span>
                              <button
                                type="button"
                                className="tool-call-toggle"
                                onClick={() =>
                                  setExpandedTools((current) => ({
                                    ...current,
                                    [toolKey]: !toolExpanded,
                                  }))
                                }
                              >
                                {toolExpanded
                                  ? "\u6536\u8d77\u5de5\u5177\u8be6\u60c5"
                                  : "\u5c55\u5f00\u5de5\u5177\u8be6\u60c5"}
                              </button>
                            </div>
                          </div>

                          {toolExpanded && (
                            <div className="tool-call-details">
                              <pre className="tool-call-input">
                                {JSON.stringify(toolCall.input, null, 2)}
                              </pre>
                              {toolCall.output && (
                                <div className="tool-call-output">{toolCall.output}</div>
                              )}
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </article>
          );
        })}
      </div>

      {conversation.file_changes.length > 0 && (
        <div className="file-changes">
          <h4>{"\u6587\u4ef6\u53d8\u66f4"}</h4>
          <div className="file-change-list">
            {conversation.file_changes.map((fileChange, index) => (
              <div key={`${fileChange.path}-${fileChange.timestamp}-${index}`} className="file-change-item">
                <span className={`file-change-badge file-change-${fileChange.change_type}`}>
                  {changeLabel(fileChange.change_type)}
                </span>
                <span className="file-change-path">{fileChange.path}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default ConversationDetail;
