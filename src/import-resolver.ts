import * as vscode from "vscode";

// ============================================================================
// PUBLIC API
// ============================================================================

/**
 * Resolve an imported identifier to its actual implementation file
 * This resolves the identifier itself (e.g., "AlertDialog"), not the import source
 */
export async function resolveImportedIdentifier(
  importInfo: any,
  fromDocument: vscode.TextDocument,
): Promise<vscode.Uri | null> {
  try {
    // Find the position of the identifier we're importing
    const position = findImportIdentifierPosition(importInfo, fromDocument);

    if (!position) {
      return null;
    }

    // Wait a moment for TypeScript language service to be ready
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Try multiple resolution strategies to find the actual implementation

    // Strategy 1: Use implementation provider (skips type definitions and re-exports)
    let implementations = (await vscode.commands.executeCommand(
      "vscode.executeImplementationProvider",
      fromDocument.uri,
      position,
    )) as any[];

    // Strategy 2: Use definition provider as fallback
    const definitions = (await vscode.commands.executeCommand(
      "vscode.executeDefinitionProvider",
      fromDocument.uri,
      position,
    )) as any[];

    // Prefer definition over implementation (definition gives us the actual package, not the wrapper)
    const results =
      definitions && definitions.length > 0 ? definitions : implementations;

    if (results && results.length > 0) {
      const result = results[0];
      const uri = result.targetUri || result.uri;

      if (uri) {
        // Convert to VSCode Uri if needed
        return vscode.Uri.parse(
          uri.scheme ? `${uri.scheme}:${uri.path}` : uri.path,
        );
      }
    }

    return null;
  } catch {
    return null;
  }
}

/**
 * Try to find the actual implementation file when we resolve to a declaration file (.d.ts, .d.mts, .d.cts)
 * For npm packages, we need to find the source file with "use client" directive
 */
export async function findImplementationFile(
  declarationUri: vscode.Uri,
): Promise<vscode.Uri | null> {
  const path = declarationUri.path;

  // Check if it's a declaration file
  if (
    !path.endsWith(".d.ts") &&
    !path.endsWith(".d.mts") &&
    !path.endsWith(".d.cts")
  ) {
    return declarationUri; // Not a declaration file, return as-is
  }

  // Try to find implementation file by replacing .d.ts with various extensions
  const basePath = path.replace(/\.d\.(ts|mts|cts)$/, "");
  const possibleExtensions = [
    ".tsx",
    ".ts",
    ".jsx",
    ".js",
    ".mts",
    ".mjs",
    ".cts",
    ".cjs",
  ];

  for (const ext of possibleExtensions) {
    const implPath = basePath + ext;
    const implUri = declarationUri.with({ path: implPath });

    try {
      // Try to read the file to see if it exists
      await vscode.workspace.fs.stat(implUri);
      return implUri;
    } catch {
      // File doesn't exist, try next extension
    }
  }

  return null;
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Find the position of an imported identifier in the document
 * We need to resolve the identifier itself, not the import source
 */
function findImportIdentifierPosition(
  importInfo: any,
  document: vscode.TextDocument,
): vscode.Position | null {
  // Import statements can span multiple lines, so we need to search backwards from the source
  // to find the import keyword, then search forward for the identifier

  const firstIdentifier = importInfo.identifier[0];
  const sourceLine = importInfo.sourceSpan.start.line;

  // Search backwards up to 10 lines to find the "import" keyword
  for (
    let lineNum = sourceLine;
    lineNum >= Math.max(0, sourceLine - 10);
    lineNum--
  ) {
    const line = document.lineAt(lineNum);
    const lineText = line.text;

    const importIndex = lineText.indexOf("import");
    if (importIndex !== -1) {
      // Found the import keyword, now search forward from this line for the identifier
      for (let searchLine = lineNum; searchLine <= sourceLine; searchLine++) {
        const searchLineText = document.lineAt(searchLine).text;
        const identifierIndex = searchLineText.indexOf(firstIdentifier);

        if (identifierIndex !== -1) {
          // Make sure it's after "import" if on the same line
          if (searchLine === lineNum && identifierIndex < importIndex) {
            continue;
          }
          return new vscode.Position(searchLine, identifierIndex);
        }
      }
    }
  }

  return null;
}
