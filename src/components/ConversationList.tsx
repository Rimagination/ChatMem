import { formatDistanceToNow } from "../utils/dateUtils";

interface ConversationSummary {
  id: string;
  source_agent: string;
  project_dir: string;
  created_at: string;
  updated_at: string;
  summary: string | null;
  message_count: number;
  file_count: number;
}

interface ConversationListProps {
  conversations: ConversationSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  loading: boolean;
}

function ConversationList({
  conversations,
  selectedId,
  onSelect,
  loading,
}: ConversationListProps) {
  if (loading) {
    return (
      <div className="conversation-list">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </div>
    );
  }

  if (conversations.length === 0) {
    return (
      <div className="conversation-list">
        <div className="empty-state">
          <div className="empty-state-icon">◎</div>
          <div className="empty-state-text">未找到对话</div>
        </div>
      </div>
    );
  }

  return (
    <div className="conversation-list">
      {conversations.map((conversation) => {
        const title = conversation.summary || "未命名对话";
        const isSelected = selectedId === conversation.id;

        return (
          <div
            key={conversation.id}
            className={`conversation-item ${isSelected ? "selected" : ""}`}
            onClick={() => onSelect(conversation.id)}
          >
            <div className="conversation-item-row">
              <div className="conversation-item-main">
                <div className="conversation-item-title" title={title}>
                  {title}
                </div>
                <div className="conversation-item-path" title={conversation.project_dir}>
                  {conversation.project_dir}
                </div>
              </div>
              <div className="conversation-item-time">
                {formatDistanceToNow(conversation.updated_at)}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export default ConversationList;
