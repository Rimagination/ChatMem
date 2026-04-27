# ChatMem Product Strategy

## Current Implementation Notes

- 本地历史扫描现在会写入 `repo_scan_runs` 审计记录；项目健康状态会暴露 `latest_scan`，用于区分“还没扫描”和“扫描过但路径/别名没有匹配”。
- 扫描报告现在会聚合未匹配的 `project_dir`；本地历史卡片可以把这些路径一键并入当前项目，手动 alias 会参与后续扫描匹配，并把对话索引归入当前 canonical repo。
- 首启自动索引现在会先运行 `import_all_local_history`，把本机 Claude、Codex、Gemini 的已有对话整体导入本地索引，再对当前项目做路径匹配和 alias 诊断；自动 bootstrap 中全量导入每个 app session 只尝试一次，手动“重新扫描本地历史”会强制刷新。
- 本地历史卡片即使已经导入了部分对话，也会继续显示剩余待并入路径，避免 `easymd`、文件 cwd、旧项目名这类历史对话被静默遗漏。
- Wiki 编译器现在会生成 `risk-ledger`，把 gotcha、过期/待复核规则和失败历史整理成项目风险台账。

- 本地历史现在是工作区顶部的主入口：它负责回答“以前聊过什么”，通过 `get_project_context` 的 `recall` 意图返回带证据的历史片段。
- 启动规则只保留新任务必须携带的稳定规则，不再承担全文历史检索职责。
- Project Context 只展示用户能理解的一线对象：conversation、memory、checkpoint、handoff；run、artifact、episode 暂时降为内部材料。
- Wiki 抽屉现在有完整阅读器；`project-overview` 被编译成“项目地图”，明确区分启动规则、本地历史证据、继续工作和维护边界。
- Wiki 编译器现在会生成 `module-map`，从已确认规则和本地历史片段中抽取源码、配置、文档路径，形成可读的模块地图。

ChatMem 的定位不是通用记忆 SDK，也不是某个模型厂商自带记忆的替代品。它更像一个本地优先的 AI 编程项目记忆控制台：把 Claude、Codex、Gemini 等工具产生的本地对话、项目事实、启动规则、Wiki 和交接包统一管理，让 agent 可以带着证据继续工作。

## 产品定位

ChatMem 应该守住三个差异化方向：

- 本地优先：先把用户机器上已经存在的历史对话变成可检索资源，而不是要求用户从安装后才开始积累记忆。
- 跨 agent：同一个项目的上下文不应该被锁在某一个客户端、某一个模型或某一次会话里。
- 可治理：自动抽取只能提出建议，真正进入启动上下文的规则必须能解释来源、能删除、能回滚、能审计。

## 记忆分层

ChatMem 不应该把所有东西都叫“记忆”。更合理的分层是：

- 本地历史：原始对话和分块索引，负责“我们以前具体聊过什么”。
- 检索证据：从历史中查到的片段，负责回答和复核。
- Wiki：项目的稳定结构化知识，负责“这个项目现在是什么样”。
- 启动规则：每次新任务都应该带上的少量稳定指令，负责“以后怎么做”。
- 交接包：某次工作中断后的恢复现场，负责“接下来做什么”。

## Wiki 的必要性

Wiki 的价值不是保存所有对话，也不是替代检索。Wiki 应该解决原始对话太碎、启动规则太短的问题：

- 把反复出现的项目事实沉淀成结构化页面。
- 用证据链接回原始对话、文件或变更，避免凭空总结。
- 帮新 agent 快速理解项目模块、设计取舍、发布流程和风险区。
- 定期从历史和代码变化中刷新，而不是让用户手工维护一堆散乱笔记。

如果某条信息只是一次任务的临时状态，用交接包；如果是长期项目知识，用 Wiki；如果是以后每次都必须执行的约束，用启动规则。

## 一口气改进路线

短期改进应聚焦在可用性，而不是继续堆术语：

- 统一术语：候选规则、启动规则、Wiki、本地历史分别显示，避免一个数字在多个地方重复出现。
- 优先本地历史：安装后先扫描和索引已有对话，让用户马上能问“我们之前说过什么”。
- 收紧自动抽取：默认只从明确的“记住/规则/注意”等表达里生成候选，避免把普通英文提示误判为规则。
- 强化 Wiki 编译：Wiki 页面要有证据、更新时间、来源对话和待复核状态。
- 做记忆治理：支持批量批准、批量忽略、过期、合并、改写、查看证据。
- 做同步迁移：把应用设置、索引元数据、Wiki 和启动规则放到稳定的用户数据目录，再通过 WebDAV/网盘同步。

## 功能取舍

根据竞品分析，ChatMem 当前应把功能分成三类处理：

值得深入：

- 本地历史检索：这是 ChatMem 与模型厂商记忆、mem0 类 SDK 最大的差异。用户安装后已有的对话必须立刻可召回。
- 启动规则治理：只保存每次新任务都应该携带的少量稳定规则，并保留候选、证据、冲突、合并和过期机制。
- Wiki 编译：从已批准规则和本地历史证据生成项目地图，帮助 agent 快速理解项目结构、命令、决策和风险。
- 继续工作包：checkpoint 和 handoff 应作为临时状态进入“继续工作”，而不是和长期规则混在一起。

已经做了但还不够深入：

- Runs / Artifacts：现在更像从对话证据派生出来的内部对象，还没有稳定生命周期、可打开产物、验证状态和用户动作。
- Episodes：适合喂给 Wiki 和最近工作摘要，但不适合作为用户需要单独管理的一级术语。
- Entity graph：目前是轻量启发式实体抽取，不能按 Zep/Cognee 那样当成知识图谱卖点。

先降级或隐藏：

- 不在主资料库里展示 Runs、Artifacts、Episodes；等它们有真实生命周期和用户动作后再升级。
- 不把 Entity Graph 放进主 UI；先作为内部召回和 Wiki 编译辅助。
- 不把所有东西都叫 memory；UI 层只保留本地历史、启动规则、Wiki、继续工作这几类用户能理解的对象。

已开始落地：

- 项目上下文抽屉增加“继续”页，把 checkpoint 和 handoff 放到临时继续工作层。
- 资料库记录收敛为 conversation、memory、checkpoint、handoff，不再混入 run、artifact、episode。
- Wiki 继续保留为 approved memory 和 local-history episodes 的可读 projection，后续再增强为真正的项目地图。

## 设计参考与致谢

ChatMem 参考了许多记忆、agent、知识库和代码 Wiki 项目的思路，但不是任何单一项目的复刻。下面这些项目和产品方向都给了 ChatMem 很多启发：

- [mem0](https://github.com/mem0ai/mem0)：通用 agent 记忆层、抽取和召回体验。
- [Letta / MemGPT](https://github.com/letta-ai/letta)：长期状态、agent 运行时和可持久化上下文。
- [Zep](https://www.getzep.com/)：面向对话记忆的时间线、事实和图谱化思路。
- [Cognee](https://github.com/topoteretes/cognee)：从数据构建 AI 可用知识图谱和检索层。
- [LangGraph / LangMem](https://docs.langchain.com/oss/python/concepts/memory)：语义记忆、情景记忆、程序记忆等分类。
- [LLM Wiki / DeepWiki / CodeWiki](https://github.com/ussumant/llm-wiki-compiler)：把代码库和资料编译成 agent 可读 Wiki 的方向。
- OpenAI / Claude native memory：模型厂商记忆提醒我们，个人偏好和项目上下文都应该能被持续带入，但本地项目证据仍需要用户自己掌控。

致谢列表表示设计参考和相关工作，不表示 ChatMem 依赖、复制或由这些项目背书。
