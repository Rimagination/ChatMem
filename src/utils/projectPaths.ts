const FILE_LIKE_EXTENSIONS = new Set([
  "c",
  "cc",
  "cpp",
  "cs",
  "css",
  "csv",
  "go",
  "h",
  "hpp",
  "html",
  "java",
  "js",
  "json",
  "jsonl",
  "jsx",
  "lock",
  "md",
  "mdx",
  "py",
  "rs",
  "scss",
  "toml",
  "ts",
  "tsx",
  "txt",
  "yaml",
  "yml",
]);

function stripFileLeaf(path: string) {
  const parts = path.split("/").filter(Boolean);
  const leaf = parts[parts.length - 1] ?? "";
  const extension = leaf.includes(".") ? leaf.slice(leaf.lastIndexOf(".") + 1).toLowerCase() : "";

  if (!extension || !FILE_LIKE_EXTENSIONS.has(extension)) {
    return path;
  }

  const parent = path.slice(0, path.length - leaf.length).replace(/\/+$/g, "");
  return parent || path;
}

export function normalizeProjectPath(projectDir: string) {
  let normalized = projectDir
    .trim()
    .replace(/^\\\\\?\\UNC\\/i, "//")
    .replace(/^\\\\\?\\/i, "")
    .replace(/^\/\/\?\//i, "")
    .replace(/\\/g, "/")
    .replace(/\/+/g, "/")
    .replace(/^([a-zA-Z])\/+/, "$1:/")
    .replace(/\/+$/g, "");

  normalized = stripFileLeaf(normalized);
  return normalized;
}

export function projectPathKey(projectDir: string) {
  const normalized = normalizeProjectPath(projectDir);
  if (/^[a-zA-Z]:\//.test(normalized)) {
    return normalized.toLowerCase();
  }

  return normalized;
}
