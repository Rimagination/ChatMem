import type { ContinuationBriefEvalCase } from "./continuationBriefEval";

const thesisRepo = "F:/OneDrive/19-郑吉林博士论文";
const rscriptRepo = "D:/work/Rscript";

export const continuationBriefEvalCases: ContinuationBriefEvalCase[] = [
  {
    id: "codex-a4-figure-complete",
    name: "A4 graphical abstract adjustment is completed",
    split: "dev",
    input: {
      repoRoot: thesisRepo,
      conversation: {
        id: "019ebe79-ed23-75a2-ba26-6e3e7ce43781",
        source_agent: "codex",
        summary: "图10 概念模型与 graphical abstract 调整",
        resume_command: "codex resume 019ebe79-ed23-75a2-ba26-6e3e7ce43781",
        messages: [
          {
            id: "a4-user",
            role: "user",
            content:
              "有压盖 字体大小也要调整 我们这个最后要放在A4版面里 然后图片的大小 标注压盖等细节 你再调整下",
          },
          {
            id: "a4-sync",
            role: "assistant",
            content:
              "这版预览看起来已经没有明显压盖了，输出 PNG 是 `4553 × 1893 px`，放 A4 横版一整页宽度也够清楚。我再同步到投稿材料包，保证包里不是旧版本。",
          },
          {
            id: "a4-final",
            role: "assistant",
            content:
              "我又按 A4 可读性重排了一版。最新版：[图0-graphical abstract_JoH版.png](F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图0-graphical abstract_JoH版.png)，同步版：[图0-graphical abstract_JoH版.pdf](F:/OneDrive/19-郑吉林博士论文/数据/背景值/05_图件与GIS/当前主线Nature图件/图0-graphical abstract_JoH版.pdf)。",
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
    },
    expectations: {
      currentGoalIncludes: ["A4 版图件已重排完成"],
      resumeIncludes: ["图0-graphical abstract_JoH版.png", "图0-graphical abstract_JoH版.pdf"],
      canonicalBefore: [["图0-graphical abstract_JoH版.png", "图10_四层概念模型.png"]],
      evidenceIncludes: ["user: 有压盖 字体大小也要调整", "assistant: 我又按 A4 可读性重排了一版"],
      forbidden: ["没有明显压盖"],
    },
  },
  {
    id: "codex-figure-four-attachment",
    name: "Temporary screenshot paths do not pollute figure-four resume",
    split: "dev",
    input: {
      repoRoot: thesisRepo,
      conversation: {
        id: "019e7d04-eb6d-7b80-ae37-2d110cc58d27",
        source_agent: "codex",
        summary: "图四图例位置调整",
        resume_command: "codex resume 019e7d04-eb6d-7b80-ae37-2d110cc58d27",
        messages: [
          {
            id: "fig4-old",
            role: "assistant",
            content:
              "旧备份包已经删掉了。我再把项目说明里的对应条目改掉，只保留当前 JoH 最终包和 PNG+代码包作为权威材料。",
          },
          {
            id: "fig4-user",
            role: "user",
            content:
              "# Files mentioned by the user:\n\n## codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png: C:/Users/keros/AppData/Local/Temp/codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png\n\n## My request for Codex:\n图四 把图例挪到下方更好吧\n<image name=[Image #1] path=\"C:\\Users\\keros\\AppData\\Local\\Temp\\codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png\"> </image>",
          },
          {
            id: "fig4-final",
            role: "assistant",
            content:
              "改好了。图四的图例已经移到下方，其他元素没动。已同步到：`10_JoH投稿最终整理包_20260614/01_Main_Figures/Fig04_HydrochemicalDistributions.png` 和 `11_JoH最终图件_PNG与代码_20260615/01_PNG主图/Fig04_HydrochemicalDistributions.png`。",
          },
        ],
        file_changes: [
          {
            path: "C:/Users/keros/AppData/Local/Temp/codex-clipboard-e83a6614-5074-4557-b3bc-3178b2e8f088.png",
            change_type: "added",
          },
          {
            path: "10_JoH投稿最终整理包_20260614/01_Main_Figures/Fig04_HydrochemicalDistributions.png",
            change_type: "modified",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["最新请求已处理完成"],
      resumeIncludes: ["从最新完成动作继续"],
      canonicalIncludes: ["Fig04_HydrochemicalDistributions.png"],
      evidenceIncludes: ["user: 图四 把图例挪到下方更好吧", "assistant: 改好了。图四的图例已经移到下方"],
      forbidden: ["codex-clipboard"],
      scopeForbidden: ["图四 把图例挪到下方更好吧"],
      resumeForbidden: ["图四 把图例挪到下方更好吧"],
      maxEvidenceArchiveLines: 1,
    },
  },
  {
    id: "codex-figure-handoff",
    name: "Figure handoff resumes from the handoff document",
    split: "dev",
    input: {
      repoRoot: thesisRepo,
      conversation: {
        id: "019e9ffd-8446-74a3-bf3d-bbddf0bd062c",
        source_agent: "codex",
        summary: "配图完善交接",
        resume_command: "codex resume 019e9ffd-8446-74a3-bf3d-bbddf0bd062c",
        messages: [
          {
            id: "handoff-user",
            role: "user",
            content: "当前对话有点长了 帮我写一个交接文档 然后我要新开个窗口继续完善配图",
          },
          {
            id: "handoff-final",
            role: "assistant",
            content:
              "交接文档写好了：[配图完善交接文档_2026-06-08.md](F:/OneDrive/19-郑吉林博士论文/数据/背景值/01_当前论文_地质分区主线/配图完善交接文档_2026-06-08.md)。里面已经整理了当前图号体系、每张图的状态、关键脚本、图件输出文件、用户偏好和雷区。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/数据/背景值/01_当前论文_地质分区主线/配图完善交接文档_2026-06-08.md",
            change_type: "added",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["继续完善配图"],
      resumeIncludes: ["配图完善交接文档_2026-06-08.md"],
      canonicalIncludes: ["配图完善交接文档_2026-06-08.md"],
      evidenceIncludes: ["user: 当前对话有点长了", "assistant: 交接文档写好了"],
    },
  },
  {
    id: "codex-generic-completed-after-user",
    name: "Generic completed fallback handles untemplated finished work",
    split: "dev",
    input: {
      repoRoot: rscriptRepo,
      conversation: {
        id: "generic-completed-after-user",
        source_agent: "codex",
        summary: "Small UI polish",
        resume_command: "codex resume generic-completed-after-user",
        messages: [
          {
            id: "generic-user",
            role: "user",
            content: "把这个按钮颜色换淡一点，旁边说明也收短一点",
          },
          {
            id: "generic-final",
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
    },
    expectations: {
      currentGoalIncludes: ["最新请求已处理完成"],
      resumeIncludes: ["从最新完成动作继续"],
      canonicalIncludes: ["settings_ui.R", "test_settings_ui.R"],
      evidenceIncludes: ["user: 把这个按钮颜色换淡一点", "assistant: 改好了。按钮颜色已经换成更浅的蓝灰色"],
      scopeForbidden: ["把这个按钮颜色换淡一点"],
      resumeForbidden: ["把这个按钮颜色换淡一点"],
    },
  },
  {
    id: "codex-submission-package",
    name: "Final manuscript package is treated as completed work",
    split: "holdout",
    input: {
      repoRoot: thesisRepo,
      conversation: {
        id: "019eb0b9-9dbb-7e22-b149-7922a39e096f",
        source_agent: "codex",
        summary: "最终投稿包整理",
        resume_command: "codex resume 019eb0b9-9dbb-7e22-b149-7922a39e096f",
        messages: [
          {
            id: "submission-user",
            role: "user",
            content:
              "ok 基本上这样了 把最终的正文和补充材料单独建个文件夹 名字改成manuscript和supplementary吧 然后很多旧文档该清理就清理吧",
          },
          {
            id: "submission-final",
            role: "assistant",
            content:
              "整理好了，没动正文内容，只做了复制、改名和归档。最终投稿包在这里：[manuscript.docx](F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/manuscript/manuscript.docx) 和 [supplementary.docx](F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/supplementary/supplementary.docx)。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/doc/Manuscript_投稿前最终版.docx",
            change_type: "modified",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/manuscript/manuscript.docx",
            change_type: "added",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/submission_final_20260612/supplementary/supplementary.docx",
            change_type: "added",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["最终投稿包已整理完成"],
      resumeIncludes: ["manuscript/manuscript.docx", "supplementary/supplementary.docx"],
      canonicalBefore: [["submission_final_20260612/manuscript/manuscript.docx", "Manuscript_投稿前最终版.docx"]],
      evidenceIncludes: ["user: ok 基本上这样了", "assistant: 整理好了"],
      scopeForbidden: ["该清理就清理"],
      resumeForbidden: ["该清理就清理"],
    },
  },
  {
    id: "codex-midterm-full-rewrite",
    name: "Full midterm report rewrite keeps delivered files visible",
    split: "holdout",
    input: {
      repoRoot: thesisRepo,
      conversation: {
        id: "019eb188-a177-7002-99c5-2252fad2e9c7",
        source_agent: "codex",
        summary: "博士论文中期报告全文重构",
        resume_command: "codex resume 019eb188-a177-7002-99c5-2252fad2e9c7",
        messages: [
          {
            id: "midterm-old",
            role: "assistant",
            content:
              "你说得准。这次我按博士论文结构方案重新校正了开头，不再从地下水硝酸盐污染评价起笔。",
          },
          {
            id: "midterm-user",
            role: "user",
            content: "也不能仅仅重写开头 全篇都要重构",
          },
          {
            id: "midterm-check",
            role: "assistant",
            content: "结构检查通过了：现在阶段性成果已经按第 2-7 章推进，不再是小论文成果的横向罗列。",
          },
          {
            id: "midterm-final",
            role: "assistant",
            content:
              "对，这次我按“全文重构”处理了，不再沿用旧稿的小论文式结构。新文件在这里：[郑吉林博士论文中期报告_全文重构版.docx](F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.docx) 和 [郑吉林博士论文中期报告_全文重构版.md](F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.md)。",
          },
        ],
        file_changes: [
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_正文初稿.docx",
            change_type: "modified",
          },
          {
            path: "F:/OneDrive/19-郑吉林博士论文/output/doc/郑吉林博士论文中期报告_全文重构版.docx",
            change_type: "added",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["最新请求已处理完成"],
      resumeIncludes: ["从最新完成动作继续"],
      canonicalIncludes: ["郑吉林博士论文中期报告_全文重构版.docx"],
      canonicalBefore: [["郑吉林博士论文中期报告_全文重构版.docx", "郑吉林博士论文中期报告_正文初稿.docx"]],
      evidenceIncludes: ["user: 也不能仅仅重写开头", "assistant: 对，这次我按“全文重构”处理了"],
      scopeForbidden: ["也不能仅仅重写开头"],
      resumeForbidden: ["也不能仅仅重写开头"],
    },
  },
  {
    id: "codex-rscript-preview-layout",
    name: "Completed data-preview layout fix resumes as verification work",
    split: "holdout",
    input: {
      repoRoot: rscriptRepo,
      conversation: {
        id: "019d51f2-4947-7320-b5b2-9b09a6d18fe5",
        source_agent: "codex",
        summary: "Rscript upload data preview layout",
        resume_command: "codex resume 019d51f2-4947-7320-b5b2-9b09a6d18fe5",
        messages: [
          {
            id: "preview-old",
            role: "assistant",
            content:
              "这轮我已经直接改了你指出的 3 个问题，并重启了应用。顶部上传/预览区的布局改成了更稳定的网页栅格，左侧固定工作区、右侧弹性预览区。",
          },
          {
            id: "preview-user",
            role: "user",
            content: "你看还是不合适啊 这个数据预览部分应该直接就固定好大小位置",
          },
          {
            id: "preview-final",
            role: "assistant",
            content:
              "我已经按你这张图继续收了一轮，并重启了应用，当前进程是 `127.0.0.1:3838`，PID `15576`。这次针对的就是你说的点：`数据预览` 现在不应该再跟着内容乱变位置和大小。我做了两层修正：给上传页加了固定宽度的工作区外壳；把 `数据预览` 区固定成稳定右侧面板，预览表滚动区域高度也固定。改动在：[upload_ui.R](D:/work/Rscript/app/ui/upload_ui.R)。",
          },
        ],
        file_changes: [
          {
            path: "D:/work/Rscript/app/ui/main_ui.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/app/ui/upload_ui.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/app/server/upload_server.R",
            change_type: "modified",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["数据预览固定布局已完成"],
      resumeIncludes: ["核对数据预览固定大小与位置"],
      canonicalIncludes: ["D:/work/Rscript/app/ui/upload_ui.R"],
      canonicalBefore: [["app/ui/upload_ui.R", "app/ui/main_ui.R"]],
      evidenceIncludes: ["user: 你看还是不合适啊", "assistant: 我已经按你这张图继续收了一轮"],
      scopeForbidden: ["你看还是不合适啊"],
      resumeForbidden: ["你看还是不合适啊"],
    },
  },
  {
    id: "codex-rscript-full-test-export",
    name: "Completed full-test request resumes as tested export-flow fix",
    split: "holdout",
    input: {
      repoRoot: rscriptRepo,
      conversation: {
        id: "019ebbc4-5a93-7620-84cc-fc7a14e0b077",
        source_agent: "codex",
        summary: "Zero-install web MVP full test",
        resume_command: "codex resume 019ebbc4-5a93-7620-84cc-fc7a14e0b077",
        messages: [
          {
            id: "test-user",
            role: "user",
            content: "你帮我完整测试下呗",
          },
          {
            id: "test-contract",
            role: "assistant",
            content:
              "契约测试已经加好，现在先让它失败一次，确认它确实捕捉到了导出页这类“看起来有按钮、实际没接上”的问题。",
          },
          {
            id: "test-final",
            role: "assistant",
            content:
              "完整测完了，也顺手修了两个真实验收会卡住的问题。修复内容：`编辑出图` 页新增 `进入导出` 按钮；修复 `导出` 页前后端名字不一致的问题；新增 3 个回归测试。改动在：[export_server.R](D:/work/Rscript/.worktrees/zero-install-web-mvp/app/server/export_server.R)、[workshop_server.R](D:/work/Rscript/.worktrees/zero-install-web-mvp/app/server/workshop_server.R)、[test_export_server_ui_contracts.R](D:/work/Rscript/.worktrees/zero-install-web-mvp/scripts/test_export_server_ui_contracts.R)。验证结果：R 解析检查通过；R 冒烟测试全过：17 个 `test_*.R` 全部 exit 0；Python API 测试全过：`9 passed`。",
          },
        ],
        file_changes: [
          {
            path: "D:/work/Rscript/docs/superpowers/specs/2026-06-12-zero-install-web-mvp-design.md",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/.worktrees/zero-install-web-mvp/app/server/export_server.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/.worktrees/zero-install-web-mvp/app/server/workshop_server.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/.worktrees/zero-install-web-mvp/scripts/test_export_server_ui_contracts.R",
            change_type: "added",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["完整测试与导出修复已完成"],
      resumeIncludes: ["复核完整测试结果"],
      canonicalIncludes: ["export_server.R", "test_export_server_ui_contracts.R"],
      canonicalBefore: [["app/server/export_server.R", "docs/superpowers/specs"]],
      evidenceIncludes: ["user: 你帮我完整测试下呗", "assistant: 完整测完了"],
      scopeForbidden: ["你帮我完整测试下呗"],
      resumeForbidden: ["你帮我完整测试下呗"],
    },
  },
  {
    id: "codex-rscript-git-save",
    name: "Completed local git save resumes after the commit",
    split: "holdout",
    input: {
      repoRoot: rscriptRepo,
      conversation: {
        id: "019d47f2-d92e-7162-8258-f8645f950786",
        source_agent: "codex",
        summary: "Rscript data exploration dataset switching fix",
        resume_command: "codex resume 019d47f2-d92e-7162-8258-f8645f950786",
        messages: [
          {
            id: "git-fix",
            role: "assistant",
            content:
              "这次我把“换一份新数据后整页发灰”的阻塞点收掉了。核心改动在 [recommend_server.R](/D:/work/Rscript/app/server/recommend_server.R)、[recommend_ui.R](/D:/work/Rscript/app/ui/recommend_ui.R)、[test_data_exploration.R](/D:/work/Rscript/scripts/test_data_exploration.R)。",
          },
          {
            id: "git-user",
            role: "user",
            content: "先git保存一下",
          },
          {
            id: "git-final",
            role: "assistant",
            content:
              "已提交到本地 git，commit 是 `6638cda`，信息是 `Stabilize data exploration dataset switching`。当前还没推远程。 ::git-commit{cwd=\"D:/work/Rscript\"}",
          },
        ],
        file_changes: [
          {
            path: "D:/work/Rscript/app/server/recommend_server.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/app/ui/recommend_ui.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/scripts/test_data_exploration.R",
            change_type: "modified",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["本地 git 已提交"],
      resumeIncludes: ["6638cda", "还没推远程"],
      canonicalIncludes: ["recommend_server.R", "recommend_ui.R", "test_data_exploration.R"],
      evidenceIncludes: ["user: 先git保存一下", "assistant: 已提交到本地 git"],
      scopeForbidden: ["先git保存一下"],
      resumeForbidden: ["先git保存一下"],
    },
  },
  {
    id: "codex-rscript-duplicate-summary-copy",
    name: "Completed duplicate overview copy cleanup resumes as review work",
    split: "holdout",
    input: {
      repoRoot: rscriptRepo,
      conversation: {
        id: "019d4c48-593c-79e3-baf1-21b375e5e0c3",
        source_agent: "codex",
        summary: "Rscript data overview duplicate copy cleanup",
        resume_command: "codex resume 019d4c48-593c-79e3-baf1-21b375e5e0c3",
        messages: [
          {
            id: "dup-previous",
            role: "assistant",
            content:
              "这轮已经按你确认的结构改了，重点是：`数据概览` 不再放 `AI 辅助摘要`，只保留数据理解和两个明确入口。",
          },
          {
            id: "dup-user",
            role: "user",
            content: "你看我箭头标的 这都重复了",
          },
          {
            id: "dup-final",
            role: "assistant",
            content:
              "对，这两处确实在重复说同一件事。我已经处理成：顶部蓝条只保留一行总览信息；右侧 `AI 辅助摘要` 卡片不再重复显示“点按钮后会生成……”这类占位说明；`待生成 AI 辅助摘要` 这个标题也改成中性的 `AI 辅助摘要`。改动在 [upload_server.R](/D:/work/Rscript/app/server/upload_server.R)。回归已过：[test_data_overview_ui.R](/D:/work/Rscript/scripts/test_data_overview_ui.R)。",
          },
        ],
        file_changes: [
          {
            path: "D:/work/Rscript/app/server/upload_server.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/scripts/test_data_overview_ui.R",
            change_type: "modified",
          },
          {
            path: "D:/work/Rscript/app/ui/analysis_navigation_ui.R",
            change_type: "modified",
          },
        ],
      },
    },
    expectations: {
      currentGoalIncludes: ["重复提示已清理"],
      resumeIncludes: ["核对数据概览重复提示"],
      canonicalIncludes: ["upload_server.R", "test_data_overview_ui.R"],
      canonicalBefore: [["app/server/upload_server.R", "app/ui/analysis_navigation_ui.R"]],
      evidenceIncludes: ["user: 你看我箭头标的", "assistant: 对，这两处确实在重复说同一件事"],
      canonicalForbidden: ["/D:/work/Rscript/app/server/upload_server.R"],
      scopeForbidden: ["你看我箭头标的"],
      resumeForbidden: ["你看我箭头标的"],
    },
  },
];
