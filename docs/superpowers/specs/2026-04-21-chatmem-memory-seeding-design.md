# ChatMem Memory Seeding Design

## Goal

让用户可以把已有对话显式沉淀为项目记忆，解决“旧对话很多，但 agent 通过 ChatMem skill 查询不到项目记忆”的问题。

本设计确认两件事：

- 项目记忆属于项目，不属于某一段对话。
- 记忆候选是待用户批准的建议，不是已经生效的长期记忆。

最终体验应该是：用户选中对话时，对话内容全宽显示；项目记忆和记忆候选不常驻占用对话阅读空间。

## Product Positioning

项目记忆是 future agent startup context。它会影响后续 agent 在同一项目里恢复工作时看到的背景、规则和注意事项。

记忆候选是 agent-assisted 提炼结果。它必须经过用户确认，才会进入项目记忆。ChatMem 不应该静默把旧对话写成长期记忆，因为旧对话里可能有临时判断、废弃命令、误解或一次性调试噪音。

因此核心闭环是：

```text
对话或项目旧记录
-> agent 提炼候选
-> 用户审核
-> 项目记忆
-> ChatMem skill / MCP 为后续 agent 提供启动上下文
```

## Chosen Direction

采用用户确认的组合方案：

- 信息架构采用项目首页：项目记忆放在“项目首页 / 项目上下文”里。
- 交互采用审核弹层：记忆候选作为需要决策的内容出现。
- 对话页保持全宽：选中对话后，不显示常驻记忆侧栏。

## User-Facing Terminology

优先使用任务语言，减少内部对象名：

- “项目记忆”用于已确认的长期上下文。
- “待确认记忆”用于 memory candidates。
- “提炼成记忆”用于从当前对话生成候选。
- “扫描旧对话”用于项目级批量生成候选。
- “保留为项目记忆”用于批准候选。
- “不保留”用于拒绝候选。

“candidate”“repo memory”“MCP”等术语只应出现在高级说明或开发文档里。

## Information Architecture

### Project Home

点击左侧项目组或项目标题进入项目首页。项目首页承载：

- 项目摘要：项目路径、对话数量、项目记忆数量、待确认数量。
- 已确认项目记忆：按类型展示规则、命令、坑点、偏好、架构决策。
- 待确认记忆：显示数量和打开审核弹层的入口。
- 生成入口：从旧对话提炼记忆。
- 最近对话：保留快速进入具体对话的路径。

项目首页是项目记忆的主归属地。

### Conversation Page

选中具体对话后，内容区只展示对话本身和恢复操作：

- 对话标题、项目路径、来源 agent。
- 复制位置、复制恢复命令、迁移、删除等现有操作。
- 全宽 ConversationDetail。
- 顶部轻量入口：“提炼成记忆”。
- 顶部待确认 badge：例如“待确认 2”。

不在对话页常驻显示项目记忆列表或候选列表。

### Review Modal

待确认记忆通过弹层处理。弹层提供：

- 候选摘要。
- 候选正文。
- 为什么值得记住。
- 证据来源或关联对话。
- 可能的重复/合并提示。
- 操作：保留为项目记忆、编辑后保留、稍后、 不保留。

弹层关闭后回到原来的对话或项目首页。

## Phase 1: Current Conversation To Memory

第一期目标是跑通最小闭环：

```text
当前对话
-> agent-assisted 提炼
-> create_memory_candidate
-> 审核弹层
-> review_memory_candidate approve
-> 项目记忆
```

### User Flow

1. 用户打开一段对话。
2. 用户点击“提炼成记忆”。
3. ChatMem 展示 agent-assisted 指令，或提供复制/启动给当前 agent 的任务提示。
4. agent 读取当前对话和项目路径，判断哪些内容值得长期保留。
5. agent 通过 ChatMem MCP 创建记忆候选。
6. ChatMem 刷新待确认数量并打开审核弹层。
7. 用户确认、编辑、稍后或拒绝。
8. 被确认的候选成为项目记忆，并可被后续 `get_repo_memory` 返回。

### Agent-Assisted Prompt Contract

ChatMem 生成给 agent 的任务提示应包含：

- repo root。
- source agent。
- conversation id。
- conversation storage path。
- 当前对话标题或摘要。
- 生成候选时的规则。

候选规则：

- 只提炼 durable、repo-scoped、future-useful 的内容。
- 优先识别项目规则、架构决策、常用命令、坑点、用户偏好、验证要求。
- 不记录秘密、账号、token、私密个人信息。
- 不记录临时任务清单、一次性调试噪音、尚未验证的猜测。
- 不直接批准，只创建 pending memory candidates。

### Phase 1 Success Criteria

- 用户能从当前对话触发“提炼成记忆”。
- 对话页仍然全宽显示，没有常驻记忆侧栏。
- agent 能按提示创建候选。
- 用户能在弹层中审核候选。
- 批准后的记忆出现在项目首页。
- 后续 agent 通过 ChatMem skill/MCP 能读到这条项目记忆。

## Phase 2: Project Conversation Scan

第二期目标是解决历史项目中已有大量对话但没有项目记忆的问题。

### User Flow

1. 用户进入项目首页。
2. 用户点击“扫描旧对话”。
3. 用户选择扫描范围：
   - 最近 N 段对话。
   - 当前筛选结果。
   - 当前项目全部旧对话。
4. ChatMem 生成 agent-assisted 批量提炼任务。
5. agent 读取范围内的对话，批量创建候选。
6. ChatMem 展示批量结果摘要。
7. 用户在审核弹层中逐条确认、编辑、稍后或拒绝。

### Batch Requirements

- 批量扫描必须进入候选队列，不直接写入项目记忆。
- 批量结果需要去重或提示潜在合并。
- UI 需要显示数量：已扫描、已生成候选、疑似重复、失败。
- 批量任务需要可中断，失败后保留已生成候选。
- 项目首页应提示“这个项目还没有项目记忆，可以从旧对话提炼”。

### Phase 2 Success Criteria

- 用户能从项目首页批量生成候选。
- 批量生成不会污染已批准记忆。
- 用户能清楚看到哪些候选来自哪些对话。
- 重复候选能提示合并或覆盖风险。
- 项目从“无记忆”状态变成“有待确认候选”状态。

## Data And Integration

现有能力：

- MCP 已有 `create_memory_candidate`。
- Tauri 后端已有 `list_memory_candidates`、`review_memory_candidate`、`list_repo_memories`。
- 存储层已有候选创建、审批、approved memory、freshness 和 merge suggestion。

新增或调整能力：

- 桌面端 API 包装：创建候选的 Tauri command，或复用 MCP 路径生成候选。
- 对话页生成 agent-assisted prompt。
- 项目首页和审核弹层 UI。
- 批量扫描任务的状态展示。

第一期可以不让桌面端直接调用模型。AI 生成由当前 agent 完成。

## Error Handling

### No Memory Generated

如果 agent 没有创建候选，ChatMem 应告诉用户：

- “这段对话里没有明显值得长期记住的内容。”
- 提供“手动添加项目记忆”作为后备路径。

### MCP Unavailable

如果 ChatMem MCP 不可用：

- 显示可复制的 agent prompt。
- 提醒用户重启 agent 或检查 MCP 配置。
- 不要求用户粘贴完整历史记录。

### Duplicate Or Overlap

如果候选与现有记忆重叠：

- 在审核弹层提示“可能与已有项目记忆重复”。
- 用户可以选择编辑合并或不保留。

### Stale Memories

过期或需要复核的项目记忆不应挤占对话页。

它们应出现在项目首页和审核弹层。

## Testing Strategy

### Frontend Tests

- 对话页不再渲染常驻 `Project Memory` 侧栏。
- 对话页显示“提炼成记忆”入口。
- 点击入口显示 agent-assisted 提示或生成弹层。
- 待确认 badge 能反映候选数量。
- 项目首页显示已确认项目记忆和待确认数量。
- 审核弹层能批准、编辑、稍后、拒绝候选。

### Backend Tests

- 创建候选的 command 或调用路径能保存 evidence refs。
- 审批候选后会进入 approved memory。
- 拒绝/稍后不会进入 approved memory。
- 批量候选能触发 merge suggestion。

### Integration Tests

- 当前对话提炼后的候选能被 UI 刷新看到。
- 批准后的项目记忆能被 `get_repo_memory` 选中返回。
- 没有项目记忆时，项目首页显示提炼引导。

## Non-Goals

本设计不要求第一期内：

- 桌面端内置 OpenAI、Claude、本地模型或 API key 管理。
- 静默自动扫描所有旧对话。
- 自动批准任何项目记忆。
- 在对话页恢复常驻右侧记忆栏。
- 解决所有 agent 的原生对话写入格式差异。

## Risks

- Agent-assisted 流程需要用户当前有可用 agent。
- 批量扫描可能生成过多候选，需要强审核和去重。
- 旧对话里的信息可能过期，需要 freshness 和证据提示。
- 如果入口文案不清楚，用户可能误以为“提炼成记忆”会直接永久写入。

## Implementation Defaults

为避免实现阶段产生歧义，本设计采用以下默认决策：

- 第一阶段不在桌面端直接调用 AI，也不要求新增桌面端模型配置。
- 第一阶段由 agent 通过 ChatMem MCP 调用 `create_memory_candidate` 创建候选。
- 桌面端负责生成 agent-assisted prompt，并提供复制到剪贴板的操作。
- 第一阶段不做“一键发送到当前 agent”的桌面集成。
- 项目首页作为点击左侧项目组后的新状态出现；选中具体对话后仍进入全宽对话页。
- 第二阶段批量扫描默认范围是当前项目最近 20 段对话，并提供“全部旧对话”作为显式选择。
