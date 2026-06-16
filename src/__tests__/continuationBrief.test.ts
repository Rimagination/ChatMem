import { describe, expect, it } from "vitest";
import { buildContinuationBriefPrompt } from "../utils/continuationBrief";

describe("buildContinuationBriefPrompt", () => {
  it("builds a source-backed continuation brief without copying raw transcript paths", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "D:/VSP/demo",
      conversation: {
        id: "conv-001",
        source_agent: "claude",
        summary: "Debug session",
        resume_command: "codex resume conv-001",
        storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
        messages: [
          {
            id: "msg-001",
            role: "user",
            content: "Fix the memory view",
            timestamp: "2026-04-08T08:00:00Z",
          },
          {
            id: "msg-002",
            role: "assistant",
            content: "Implemented compact continuation with focused evidence windows.",
            timestamp: "2026-04-08T08:30:00Z",
          },
        ],
        file_changes: [
          {
            path: "src/App.tsx",
            change_type: "modified",
          },
        ],
      },
      checkpointId: "checkpoint-001",
      handoffId: "handoff-001",
    });

    expect(prompt).toContain("# Continuation Brief");
    expect(prompt).toContain("Current goal: 最新请求已处理完成");
    expect(prompt).toContain("Current workline");
    expect(prompt).toContain("Implemented compact continuation with focused evidence windows.");
    expect(prompt).toContain("Evidence source: claude:conv-001");
    expect(prompt).toContain("Canonical files");
    expect(prompt).toContain("- src/App.tsx");
    expect(prompt).toContain("Token posture:");
    expect(prompt).toContain("brief instead of the raw transcript");
    expect(prompt).toContain("checkpoint: checkpoint-001");
    expect(prompt).toContain("handoff: handoff-001");
    expect(prompt).toContain('get_project_context with intent="continue_work" and limit=3');
    expect(prompt).toContain("read_history_conversation");
    expect(prompt).not.toContain("rollout-conv-001.jsonl");
  });

  it("prefers the latest active workline over superseded early goals", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-long-session",
        source_agent: "codex",
        summary: "你看下nitrogen_background_paper_section.md 这个文件，你觉得这个氮背景值确定的内容，是否够支持一篇sci论文",
        resume_command: "codex resume codex-long-session",
        storage_path: "C:/Users/demo/.codex/sessions/2026/05/31/rollout-codex-long-session.jsonl",
        messages: [
          {
            id: "msg-001",
            role: "user",
            content: "你看下nitrogen_background_paper_section.md 这个文件，你觉得这个氮背景值确定的内容，是否够支持一篇sci论文",
          },
          {
            id: "msg-002",
            role: "assistant",
            content: "结论：现在这份内容还不够单独支撑一篇完整 SCI 论文，但已经够作为核心结果章节。",
          },
          {
            id: "msg-003",
            role: "user",
            content: "有个问题 就是我们被EI期刊拒稿了 所以我开始改投JOH 正在修改论文的内容 结构之类的",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "对，已经改好了 AGENTS.md。我把小论文项目说明从 EI 投稿最终版改成了 JoH 改投稿当前工作线，EI 已拒稿，相关文件只作为历史追溯。",
          },
          {
            id: "msg-005",
            role: "user",
            content: "图四 把图例挪到下方更好吧",
          },
          {
            id: "msg-006",
            role: "assistant",
            content:
              "改好了。图四的图例已经移到下方，其他元素没动：数据、配色、坐标、KW 标注、阈值线都保持原样。已同步到 10_JoH投稿最终整理包_20260614/01_Main_Figures/Fig04_HydrochemicalDistributions.png 和 11_JoH最终图件_PNG与代码_20260615/01_PNG主图/Fig04_HydrochemicalDistributions.png。",
          },
        ],
        file_changes: [
          {
            path: "数据/背景值/10_JoH投稿最终整理包_20260614/01_Main_Figures/Fig04_HydrochemicalDistributions.png",
            change_type: "modified",
          },
          {
            path: "数据/背景值/11_JoH最终图件_PNG与代码_20260615/01_PNG主图/Fig04_HydrochemicalDistributions.png",
            change_type: "modified",
          },
          {
            path: "数据/背景值/11_JoH最终图件_PNG与代码_20260615/02_绘图代码/生成图3_地质分区水化学指标分布_Python.py",
            change_type: "modified",
          },
        ],
      },
    });

    expect(prompt).toContain("## Current workline");
    expect(prompt).toContain("JoH");
    expect(prompt).toContain("## Latest completed action");
    expect(prompt).toContain("图四的图例已经移到下方");
    expect(prompt).toContain("## Where to resume");
    expect(prompt).toContain("图四 把图例挪到下方更好吧");
    expect(prompt).toContain("## Canonical files");
    expect(prompt).toContain("10_JoH投稿最终整理包_20260614");
    expect(prompt).toContain("11_JoH最终图件_PNG与代码_20260615");
    expect(prompt).toContain("## Obsolete or archived context");
    expect(prompt).toContain("EI");
    expect(prompt).toContain("历史追溯");
    expect(prompt).not.toContain("- Current goal: 你看下nitrogen_background_paper_section.md");
  });

  it("keeps the latest request and completion in evidence when archive signals score higher", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-evidence-balance",
        source_agent: "codex",
        summary: "早期 SCI 判断",
        resume_command: "codex resume codex-evidence-balance",
        messages: [
          {
            id: "msg-001",
            role: "user",
            content: "你看下这个内容是否够支撑 SCI 论文",
          },
          {
            id: "msg-002",
            role: "assistant",
            content:
              "清理好了，走的是归档路线。EI 旧投稿包和 07_全量复现备份包都已经归档，当前 JoH 最终包保留。",
          },
          {
            id: "msg-003",
            role: "assistant",
            content:
              "删掉了。已删除：F:/project/数据/背景值/07_全量复现备份包。现在只剩当前工作线和归档。",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "旧备份包已经删掉了，我再把项目说明里的对应条目改掉，只保留当前 JoH 最终包和 PNG 与代码包作为权威材料。",
          },
          {
            id: "msg-005",
            role: "assistant",
            content:
              "最后核对：当前目录不再保留 EI 旧投稿包，归档目录里有历史材料，当前 JoH 最终整理包继续作为主线。",
          },
          {
            id: "msg-006",
            role: "user",
            content: "图四 把图例挪到下方更好吧",
          },
          {
            id: "msg-007",
            role: "assistant",
            content:
              "改好了。图四的图例已经移到下方，其他元素没动：数据、配色、坐标、KW 标注、阈值线都保持原样。已同步到 10_JoH投稿最终整理包_20260614/01_Main_Figures/Fig04_HydrochemicalDistributions.png。",
          },
        ],
        file_changes: [],
      },
    });

    const evidence = prompt.slice(prompt.indexOf("## Evidence"), prompt.indexOf("## Token posture:"));
    const archiveEvidenceCount = evidence
      .split("\n")
      .filter((line) => line.startsWith("- "))
      .filter((line) => /归档|删除|删掉|EI|备份包/.test(line)).length;

    expect(evidence).toContain("user: 图四 把图例挪到下方更好吧");
    expect(evidence).toContain("assistant: 改好了。图四的图例已经移到下方");
    expect(archiveEvidenceCount).toBe(1);
    expect(evidence).not.toContain("user: 你看下这个内容是否够支撑 SCI 论文");
  });

  it("cleans Codex attachment wrappers and prioritizes files from the latest completed action", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-figure-four",
        source_agent: "codex",
        summary: "早期 SCI 判断",
        resume_command: "codex resume codex-figure-four",
        messages: [
          {
            id: "msg-001",
            role: "assistant",
            content:
              "删掉了。已删除：F:/OneDrive/19-郑吉林博士论文/数据/背景值/07_全量复现备份包。现在只剩当前工作线和归档。",
          },
          {
            id: "msg-002",
            role: "user",
            content:
              "# Files mentioned by the user:\n\n## codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png: C:/Users/keros/AppData/Local/Temp/codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png\n\n## My request for Codex:\n图四 把图例挪到下方更好吧\n<image name=[Image #1] path=\"C:\\Users\\keros\\AppData\\Local\\Temp\\codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png\"> </image>",
          },
          {
            id: "msg-003",
            role: "assistant",
            content:
              "`rg` 扫到了不少归档里的旧文本和嵌入图像内容，信息有点吵。我会缩小到 R 脚本里的最终图名/输出名来找。",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "改好了。图四的图例已经移到下方，其他元素没动：数据、配色、坐标、KW 标注、阈值线都保持原样。已同步到：`10_JoH投稿最终整理包_20260614/01_Main_Figures/Fig04_HydrochemicalDistributions.png` 和 `11_JoH最终图件_PNG与代码_20260615/01_PNG主图/Fig04_HydrochemicalDistributions.png`。源脚本：`11_JoH最终图件_PNG与代码_20260615/02_绘图代码/生成图4_JoH筛选参照稳健性.R`。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/数据/背景值/03_分析脚本/生成图8_JoH空间背景检查.R",
            change_type: "modified",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/数据/背景值/03_分析脚本/生成图7_JoH阈值下样本分离.R",
            change_type: "modified",
          },
          {
            path: "C:/Users/keros/AppData/Local/Temp/codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png",
            change_type: "modified",
          },
        ],
      },
    });

    const scope = prompt.slice(prompt.indexOf("## Scope"), prompt.indexOf("## Current workline"));
    const resume = prompt.slice(prompt.indexOf("## Where to resume"), prompt.indexOf("## Canonical files"));
    const files = prompt.slice(prompt.indexOf("## Canonical files"), prompt.indexOf("## Obsolete or archived context"));
    const evidence = prompt.slice(prompt.indexOf("## Evidence"), prompt.indexOf("## Token posture:"));

    expect(scope).toContain("Current goal: 最新请求已处理完成");
    expect(scope).not.toContain("图四 把图例挪到下方更好吧");
    expect(scope).not.toContain("Files mentioned");
    expect(scope).not.toContain("codex-clipboard");
    expect(scope).not.toContain("<image");
    expect(resume).toContain("Start from the latest user request: 从最新完成动作继续");
    expect(resume).not.toContain("图四 把图例挪到下方更好吧");
    expect(resume).not.toContain("<image");
    expect(files.indexOf("Fig04_HydrochemicalDistributions.png")).toBeLessThan(
      files.indexOf("生成图8_JoH空间背景检查.R"),
    );
    expect(files).toContain("生成图4_JoH筛选参照稳健性.R");
    expect(files).not.toContain("AppData/Local/Temp");
    expect(files).not.toContain("codex-clipboard");
    expect(evidence).toContain("user: 图四 把图例挪到下方更好吧");
    expect(evidence).toContain("assistant: 改好了。图四的图例已经移到下方");
    expect(evidence).not.toContain("rg` 扫到了不少");
    expect(evidence).not.toContain("<image");
  });

  it("uses the final delivery as the latest action without marking it obsolete", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-midterm-report",
        source_agent: "codex",
        summary: "博士论文中期报告重构",
        resume_command: "codex resume codex-midterm-report",
        messages: [
          {
            id: "msg-001",
            role: "assistant",
            content:
              "你说得准。这次我按博士论文结构方案.md重新校正了开头，不再从“地下水硝酸盐污染评价”起笔，而是从“寒区关键带氮素迁移、转化、累积与衰减机制”起笔。",
          },
          {
            id: "msg-002",
            role: "user",
            content: "也不能仅仅重写开头 全篇都要重构",
          },
          {
            id: "msg-003",
            role: "assistant",
            content:
              "结构检查通过了：现在“阶段性成果”已经按第 2-7 章推进，不再是小论文成果的横向罗列。我再导出 PDF 看首页和表格页，确认没有排版问题。",
          },
          {
            id: "msg-004",
            role: "assistant",
            content: "首页已经正常，标题换行也顺了。再看一个表格页，确认大表没有明显挤压。",
          },
          {
            id: "msg-005",
            role: "assistant",
            content:
              "对，这次我按“全文重构”处理了，不再沿用旧稿的小论文式结构。新文件在这里：郑吉林博士论文中期报告_全文重构版.docx：F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.docx，郑吉林博士论文中期报告_全文重构版.md：F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.md。这版的核心变化是全文按博士论文主线重构。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.docx",
            change_type: "modified",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.md",
            change_type: "modified",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_正文初稿_按结构方案重写开头版.docx",
            change_type: "modified",
          },
        ],
      },
    });

    const action = prompt.slice(
      prompt.indexOf("## Latest completed action"),
      prompt.indexOf("## Where to resume"),
    );
    const obsolete = prompt.slice(
      prompt.indexOf("## Obsolete or archived context"),
      prompt.indexOf("## Evidence"),
    );

    expect(action).toContain("全文重构版.docx");
    expect(action).toContain("全文重构版.md");
    expect(action).not.toContain("首页已经正常");
    expect(obsolete).not.toContain("全文重构版.docx");
    expect(obsolete).not.toContain("全文重构版.md");
  });

  it("does not treat future revision plans with 已有 as completed actions", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-midterm-followup",
        source_agent: "codex",
        summary: "博士论文中期报告重构",
        messages: [
          {
            id: "msg-001",
            role: "user",
            content: "也不能仅仅重写开头 全篇都要重构",
          },
          {
            id: "msg-002",
            role: "assistant",
            content:
              "对，这次我按“全文重构”处理了，不再沿用旧稿的小论文式结构。新文件在这里：F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.docx 和 F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.md。",
          },
          {
            id: "msg-003",
            role: "user",
            content:
              "开头第一句就不对啊 哪有这么说话的啊。看看我郭晓宇版本的示例",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "我会把这一节改成更像郭晓宇示例的正向铺陈：先讲地下水氮素与寒区关键带意义，再讲研究区代表性，再讲已有研究不足，最后才落到本研究。",
          },
        ],
        file_changes: [],
      },
    });

    const scope = prompt.slice(prompt.indexOf("## Scope"), prompt.indexOf("## Current workline"));
    const action = prompt.slice(
      prompt.indexOf("## Latest completed action"),
      prompt.indexOf("## Where to resume"),
    );
    const evidence = prompt.slice(prompt.indexOf("## Evidence"), prompt.indexOf("## Token posture:"));

    expect(scope).toContain("Current goal: 开头第一句就不对啊");
    expect(action).toContain("全文重构版.docx");
    expect(action).toContain("全文重构版.md");
    expect(action).not.toContain("已有研究不足");
    expect(evidence).toContain("user: 开头第一句就不对啊");
  });

  it("does not mark delivered files obsolete just because the delivery says 不再", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-midterm-delivery-obsolete",
        source_agent: "codex",
        summary: "博士论文中期报告重构",
        messages: [
          {
            id: "msg-001",
            role: "user",
            content: "全篇都要重构",
          },
          {
            id: "msg-002",
            role: "assistant",
            content:
              "对，这次我按“全文重构”处理了，不再沿用旧稿的小论文式结构。新文件在这里：F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.docx 和 F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.md。",
          },
          {
            id: "msg-003",
            role: "user",
            content: "开头第一句还是不对",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "对，这次这个开头语气已经改掉了，不再用解释式句子。已同步更新：F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.docx 和 F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.md。",
          },
        ],
        file_changes: [],
      },
    });

    const obsolete = prompt.slice(
      prompt.indexOf("## Obsolete or archived context"),
      prompt.indexOf("## Evidence"),
    );
    const evidence = prompt.slice(prompt.indexOf("## Evidence"), prompt.indexOf("## Token posture:"));

    expect(obsolete).not.toContain("全文重构版.docx");
    expect(obsolete).not.toContain("全文重构版.md");
    expect(evidence).toContain("assistant: 对，这次我按“全文重构”处理了");
    expect(evidence).toContain("assistant: 对，这次这个开头语气已经改掉了");
  });

  it("turns a completed handoff request into a resume goal instead of more handoff work", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-figure-handoff",
        source_agent: "codex",
        summary: "配图完善和图号迁移",
        resume_command: "codex resume codex-figure-handoff",
        messages: [
          {
            id: "msg-001",
            role: "assistant",
            content:
              "我给你定位到具体脚本和图例那一段，这样你可以直接改坐标，不用在整份脚本里找。",
          },
          {
            id: "msg-002",
            role: "assistant",
            content:
              "现在旧编号生成物已经确认清楚，我会按文件名前缀精确删除，不碰原始数据和当前新编号图件。",
          },
          {
            id: "msg-003",
            role: "assistant",
            content:
              "我会只加边框和白底，不再改动图例文字和位置，避免又把版式带偏。",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "这次我把图例框整体移到 Granite 子图上方留白区了，不再压盖绿色包络线。",
          },
          {
            id: "msg-005",
            role: "assistant",
            content:
              "已按你的意思迁移好了：原图 5A 已规范为图 5，原图 5B 改为图 6，后面的原图 6/图 7 顺延为图 7/图 8。",
          },
          {
            id: "msg-006",
            role: "user",
            content: "当前对话有点长了 帮我写一个交接文档 然后我要新开个窗口继续完善配图",
          },
          {
            id: "msg-007",
            role: "assistant",
            content:
              "交接文档写好了：[配图完善交接文档_2026-06-08.md](F:/OneDrive/19-郑吉林博士论文/数据/背景值/01_当前论文_地质分区主线/配图完善交接文档_2026-06-08.md) 里面已经整理了：当前图号体系、每张图的状态、关键脚本、图件输出文件、用户偏好和雷区、下一步建议，以及一段可以直接复制到新窗口的开场提示。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/数据/背景值/01_当前论文_地质分区主线/配图完善交接文档_2026-06-08.md",
            change_type: "modified",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/数据/背景值/01_当前论文_地质分区主线/硝酸盐背景值论文_地质分区主线_中文版.md",
            change_type: "modified",
          },
        ],
      },
    });

    const scope = prompt.slice(prompt.indexOf("## Scope"), prompt.indexOf("## Current workline"));
    const action = prompt.slice(
      prompt.indexOf("## Latest completed action"),
      prompt.indexOf("## Where to resume"),
    );
    const resume = prompt.slice(prompt.indexOf("## Where to resume"), prompt.indexOf("## Canonical files"));
    const obsolete = prompt.slice(
      prompt.indexOf("## Obsolete or archived context"),
      prompt.indexOf("## Evidence"),
    );

    expect(scope).toContain("Current goal: 继续完善配图");
    expect(scope).not.toContain("帮我写一个交接文档");
    expect(action).toContain("配图完善交接文档_2026-06-08.md");
    expect(resume).toContain("继续完善配图");
    expect(resume).toContain("配图完善交接文档_2026-06-08.md");
    expect(obsolete).not.toContain("定位到具体脚本");
    expect(obsolete).not.toContain("旧编号生成物已经确认清楚");
    expect(obsolete).not.toContain("不再改动图例");
    expect(obsolete).not.toContain("不再压盖");
  });

  it("turns a completed final submission package request into a package resume goal", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-submission-package",
        source_agent: "codex",
        summary: "最终投稿包整理",
        resume_command: "codex resume codex-submission-package",
        messages: [
          {
            id: "msg-001",
            role: "assistant",
            content:
              "已按这版终检意见处理完，生成新文件：[Manuscript_投稿前最终清理版.docx](F:/OneDrive/19-郑吉林博士论文/output/doc/Manuscript_投稿前最终清理版.docx)。",
          },
          {
            id: "msg-002",
            role: "assistant",
            content:
              "我只读检查了这份补充材料，没有改原文件：[Supplementary_Information_投稿前最终版.docx](F:/OneDrive/19-郑吉林博士论文/output/doc/Supplementary_Information_投稿前最终版.docx)。",
          },
          {
            id: "msg-003",
            role: "user",
            content:
              "ok 基本上这样了 把最终的正文和补充材料单独建个文件夹 名字改成manuscript和supplementary吧 然后很多旧文档该清理就清理吧",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "我来做成“投稿最终包”的形式：复制一份最终正文和补充材料进去，文件名简化成 `manuscript.docx` 和 `supplementary.docx`；旧文件我先归档到一个旧版文件夹，不直接删除，避免 OneDrive/手改版本误伤。",
          },
          {
            id: "msg-005",
            role: "assistant",
            content:
              "我会把最终包放到 `output/submission_final_20260612`，下面分成 `manuscript` 和 `supplementary` 两个子文件夹，方便你投稿系统上传时直接找。旧版我归档到 `output/doc/_archive_旧版投稿文档_20260612`。",
          },
          {
            id: "msg-006",
            role: "assistant",
            content:
              "整理好了，没动正文内容，只做了复制、改名和归档。最终投稿包在这里：[manuscript.docx](F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/manuscript/manuscript.docx) 和 [supplementary.docx](F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/supplementary/supplementary.docx)。旧版文件已归档到 `output/doc/_archive_旧版投稿文档_20260612`。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/manuscript/manuscript.docx",
            change_type: "created",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/supplementary/supplementary.docx",
            change_type: "created",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/doc/Manuscript_投稿前最终版.docx",
            change_type: "modified",
          },
        ],
      },
    });

    const scope = prompt.slice(prompt.indexOf("## Scope"), prompt.indexOf("## Current workline"));
    const resume = prompt.slice(prompt.indexOf("## Where to resume"), prompt.indexOf("## Canonical files"));
    const files = prompt.slice(prompt.indexOf("## Canonical files"), prompt.indexOf("## Obsolete or archived context"));
    const obsolete = prompt.slice(
      prompt.indexOf("## Obsolete or archived context"),
      prompt.indexOf("## Evidence"),
    );
    const evidence = prompt.slice(prompt.indexOf("## Evidence"), prompt.indexOf("## Token posture:"));
    const archiveEvidenceCount = evidence
      .split("\n")
      .filter((line) => line.startsWith("- "))
      .filter((line) => /旧版|归档|删除|删掉|archive/.test(line)).length;

    expect(scope).toContain("Current goal: 最终投稿包已整理完成");
    expect(scope).not.toContain("该清理就清理");
    expect(resume).toContain("manuscript/manuscript.docx");
    expect(resume).toContain("supplementary/supplementary.docx");
    expect(files.split("\n")[1]).toContain("submission_final_20260612/manuscript/manuscript.docx");
    expect(files.split("\n")[2]).toContain("submission_final_20260612/supplementary/supplementary.docx");
    expect(obsolete).not.toContain("我来做成");
    expect(obsolete).not.toContain("我会把最终包放到");
    expect(evidence).toContain("user: ok 基本上这样了");
    expect(evidence).toContain("assistant: 整理好了");
    expect(archiveEvidenceCount).toBeLessThanOrEqual(1);
  });

  it("turns a completed A4 figure adjustment into a figure review goal", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "F:/OneDrive/19-郑吉林博士论文",
      conversation: {
        id: "codex-a4-figure",
        source_agent: "codex",
        summary: "图10 概念模型与 graphical abstract 调整",
        resume_command: "codex resume codex-a4-figure",
        messages: [
          {
            id: "msg-001",
            role: "assistant",
            content:
              "我又重排了一版，这次不是简单放大，而是按 A4 横版阅读重新处理了：字体继续放大，图内文字进一步压短，左右两个含水介质大框加宽。",
          },
          {
            id: "msg-002",
            role: "user",
            content:
              "有压盖 字体大小也要调整 我们这个最后要放在A4版面里 然后图片的大小 标注压盖等细节 你再调整下",
          },
          {
            id: "msg-003",
            role: "assistant",
            content:
              "这版预览看起来已经没有明显压盖了，输出 PNG 是 `4553 × 1893 px`，放 A4 横版一整页宽度也够清楚。已导出：[图0-graphical abstract_A4可读性重排版.png](F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图0-graphical abstract_A4可读性重排版.png) 和 [图0-graphical abstract_A4可读性重排版.pdf](F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图0-graphical abstract_A4可读性重排版.pdf)。我再同步到投稿材料包，保证包里不是旧版本。",
          },
          {
            id: "msg-004",
            role: "assistant",
            content:
              "我又按 A4 可读性重排了一版，重点改了这些：整体字号放大，第四系框里图标和标签重新拉开，左侧地表图、雪融降雨框、河谷背景框重新排开。预览图：[图0-graphical abstract_A4可读性重排版.png](F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图0-graphical abstract_A4可读性重排版.png)。PNG 改为高分辨率导出：`4553 × 1893 px`。最新版：[图0-graphical abstract_A4可读性重排版.png](F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图0-graphical abstract_A4可读性重排版.png)。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图10_四层概念模型.png",
            change_type: "modified",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图10_四层概念模型.pdf",
            change_type: "modified",
          },
        ],
      },
    });

    const scope = prompt.slice(prompt.indexOf("## Scope"), prompt.indexOf("## Current workline"));
    const resume = prompt.slice(prompt.indexOf("## Where to resume"), prompt.indexOf("## Canonical files"));
    const files = prompt.slice(prompt.indexOf("## Canonical files"), prompt.indexOf("## Obsolete or archived context"));
    const obsolete = prompt.slice(
      prompt.indexOf("## Obsolete or archived context"),
      prompt.indexOf("## Evidence"),
    );
    const evidence = prompt.slice(prompt.indexOf("## Evidence"), prompt.indexOf("## Token posture:"));

    expect(scope).toContain("Current goal: A4 版图件已重排完成");
    expect(scope).not.toContain("有压盖 字体大小也要调整");
    expect(resume).toContain("图0-graphical abstract_A4可读性重排版.png");
    expect(resume).toContain("图0-graphical abstract_A4可读性重排版.pdf");
    expect(files.indexOf("图0-graphical abstract_A4可读性重排版.png")).toBeLessThan(
      files.indexOf("图10_四层概念模型.png"),
    );
    expect(obsolete).not.toContain("没有明显压盖");
    expect(evidence).toContain("user: 有压盖 字体大小也要调整");
    expect(evidence).toContain("assistant: 我又按 A4 可读性重排了一版");
  });

  it("uses a generic completed-state fallback after an untemplated user request is handled", () => {
    const prompt = buildContinuationBriefPrompt({
      repoRoot: "D:/work/Rscript",
      conversation: {
        id: "codex-generic-complete",
        source_agent: "codex",
        summary: "Rscript small UI polish",
        resume_command: "codex resume codex-generic-complete",
        messages: [
          {
            id: "msg-001",
            role: "user",
            content: "把这个按钮颜色换淡一点，旁边说明也收短一点",
          },
          {
            id: "msg-002",
            role: "assistant",
            content:
              "改好了。按钮颜色已经换成更浅的蓝灰色，旁边说明也压短成一行。改动在 [settings_ui.R](D:/work/Rscript/app/ui/settings_ui.R)，并补了 [test_settings_ui.R](D:/work/Rscript/scripts/test_settings_ui.R)。回归已过。",
          },
        ],
        file_changes: [
          {
            path: "D:/work/Rscript/app/ui/settings_ui.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/scripts/test_settings_ui.R",
            change_type: "modified",
          },
        ],
      },
    });

    const scope = prompt.slice(prompt.indexOf("## Scope"), prompt.indexOf("## Current workline"));
    const resume = prompt.slice(prompt.indexOf("## Where to resume"), prompt.indexOf("## Canonical files"));
    const completed = prompt.slice(
      prompt.indexOf("## Latest completed action"),
      prompt.indexOf("## Where to resume"),
    );

    expect(scope).toContain("Current goal: 最新请求已处理完成");
    expect(scope).not.toContain("把这个按钮颜色换淡一点");
    expect(resume).toContain("从最新完成动作继续");
    expect(resume).not.toContain("把这个按钮颜色换淡一点");
    expect(completed).toContain("改好了。按钮颜色已经换成更浅的蓝灰色");
  });
});
