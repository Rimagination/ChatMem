import { useState, useEffect } from "react";

interface ConversationSummary {
  id: string;
  project_dir: string;
  created_at: string;
  updated_at: string;
  summary: string | null;
  message_count: number;
  file_count: number;
}

interface Conversation {
  id: string;
  project_dir: string;
  messages: Message[];
  file_changes: FileChange[];
}

interface Message {
  id: string;
  timestamp: string;
  role: string;
  content: string;
}

interface FileChange {
  path: string;
  change_type: string;
  timestamp: string;
}

function App() {
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [selectedConversation, setSelectedConversation] = useState<Conversation | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadConversations();
  }, []);

  const loadConversations = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch("http://localhost:3001/api/conversations");
      if (!response.ok) {
        throw new Error("无法连接到后端服务器");
      }
      const data = await response.json();
      setConversations(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载失败");
    } finally {
      setLoading(false);
    }
  };

  const loadConversation = async (id: string) => {
    setLoading(true);
    try {
      const response = await fetch(`http://localhost:3001/api/conversations/${id}`);
      const data = await response.json();
      setSelectedConversation(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载失败");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="container">
      <header className="header">
        <h1>AgentSwap GUI (Lite)</h1>
        <button className="btn btn-primary" onClick={loadConversations}>
          刷新
        </button>
      </header>

      <main className="main-content">
        <aside className="sidebar">
          <div className="sidebar-header">
            <h2>对话列表</h2>
          </div>
          {loading && <div className="loading">加载中...</div>}
          {error && <div className="error">{error}</div>}
          <div className="conversation-list">
            {conversations.map((conv) => (
              <div
                key={conv.id}
                className="conversation-item"
                onClick={() => loadConversation(conv.id)}
              >
                <div className="conversation-item-title">
                  {conv.summary || "无标题对话"}
                </div>
                <div className="conversation-item-meta">
                  <span>💬 {conv.message_count} 消息</span>
                  <span>📁 {conv.file_count} 文件</span>
                </div>
              </div>
            ))}
          </div>
        </aside>

        <section className="content-area">
          <div className="content-body">
            {selectedConversation ? (
              <div>
                <h2>对话：{selectedConversation.id}</h2>
                <div className="message-list">
                  {selectedConversation.messages.map((msg, idx) => (
                    <div key={idx} className={`message message-${msg.role}`}>
                      <div className="message-role">{msg.role}</div>
                      <div className="message-content">{msg.content}</div>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className="empty-state">
                <div>选择一个对话查看详情</div>
              </div>
            )}
          </div>
        </section>
      </main>
    </div>
  );
}

export default App;
