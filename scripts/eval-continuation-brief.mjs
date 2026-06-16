import { createLogger, createServer } from "vite";

function parseSplitArg(argv) {
  const splitArg = argv.find((arg) => arg.startsWith("--split="));
  if (!splitArg) {
    return null;
  }
  const split = splitArg.slice("--split=".length);
  if (split !== "dev" && split !== "holdout") {
    throw new Error('Expected --split to be "dev" or "holdout".');
  }
  return split;
}

const split = parseSplitArg(process.argv.slice(2));
const logger = createLogger("error");
const logError = logger.error;
logger.error = (message, options) => {
  if (String(message).includes("WebSocket server error")) {
    return;
  }
  logError(message, options);
};

const server = await createServer({
  appType: "custom",
  configFile: false,
  customLogger: logger,
  logLevel: "error",
  server: { hmr: false, middlewareMode: true },
});

try {
  const { continuationBriefEvalCases } = await server.ssrLoadModule(
    "/src/utils/continuationBriefEvalCases.ts",
  );
  const { evaluateContinuationBriefSuite, formatContinuationBriefEvalReport } =
    await server.ssrLoadModule("/src/utils/continuationBriefEval.ts");
  const selectedCases = split
    ? continuationBriefEvalCases.filter((testCase) => testCase.split === split)
    : continuationBriefEvalCases;
  const result = evaluateContinuationBriefSuite(selectedCases);

  console.log(formatContinuationBriefEvalReport(result));
  process.exitCode = result.issues.length > 0 ? 1 : 0;
} finally {
  await server.close();
}
