import * as vscode from "vscode";
import { analyzeReactBoundary } from "./analyzeReactBoundary";

const componentDecoration = vscode.window.createTextEditorDecorationType({
  after: {
    contentText: " ⬅️ Client Component",
    margin: "0 0 0 1rem",
    color: "rgba(100, 100, 100, 0.7)",
    fontStyle: "italic",
  },
});

const usageDecoration = vscode.window.createTextEditorDecorationType({
  after: {
    contentText: " ⬅️ Client Component",
    margin: "0 0 0 1rem",
    color: "rgba(255, 150, 50, 0.8)",
    fontStyle: "italic",
  },
});

/**
 * Resolve an import specifier to its file path using VSCode's language service
 * The sourceSpan should point inside the import string (Rust provides this after the opening quote)
 */
async function resolveImport(
  sourceSpan: { start: { line: number; character: number } },
  fromDocument: vscode.TextDocument,
): Promise<vscode.Uri | null> {
  try {
    // Use the position directly from Rust AST analysis
    const position = new vscode.Position(
      sourceSpan.start.line,
      sourceSpan.start.character,
    );

    // Wait a moment for TypeScript language service to be ready
    await new Promise(resolve => setTimeout(resolve, 50));

    // Use VSCode's definition provider to resolve the import
    const definitions = (await vscode.commands.executeCommand(
      "vscode.executeDefinitionProvider",
      fromDocument.uri,
      position,
    )) as any[];

    if (definitions && definitions.length > 0) {
      // The definition provider returns objects with targetUri property
      const definition = definitions[0];
      const uri = definition.targetUri || definition.uri;

      if (uri) {
        // Convert to VSCode Uri if needed
        return vscode.Uri.parse(uri.scheme ? `${uri.scheme}:${uri.path}` : uri.path);
      }
    }

    return null;
  } catch {
    return null;
  }
}

/**
 * Analyze a document and decorate client components
 */
export async function analyzeDocument(
  editor: vscode.TextEditor | undefined,
  api: analyzeReactBoundary.Exports,
  channel: vscode.LogOutputChannel,
): Promise<void> {
  if (!editor) return;
  if (editor.document.isUntitled) return;

  const document = editor.document;

  const extension = document.uri.path.split(".").pop();
  if (!extension) {
    return;
  }

  // Read from the document buffer (not disk) to get unsaved changes
  const documentText = document.getText();
  const fileContent = new TextEncoder().encode(documentText);
  const analyzed = api.analyze(fileContent, extension);

  const componentRanges: vscode.Range[] = [];
  const usageRanges: vscode.Range[] = [];

  // Decorate local client components
  for (const component of analyzed.components) {
    if (component.isClientComponent) {
      const range = new vscode.Range(
        component.range.start.line,
        component.range.start.character,
        component.range.end.line,
        component.range.end.character,
      );
      componentRanges.push(range);
    }
  }

  // Check which imports are client components
  const clientComponentImports = new Set<string>();

  for (const importInfo of analyzed.imports) {
    try {
      // Use VSCode's TypeScript language service to resolve the import
      // Pass the source span directly from Rust AST analysis
      const resolvedUri = await resolveImport(importInfo.sourceSpan, document);

      if (resolvedUri) {

        // Try to get the open document first (for unsaved changes), otherwise read from disk
        const openDoc = vscode.workspace.textDocuments.find(
          (doc) => doc.uri.toString() === resolvedUri.toString()
        );
        const importedFileContent = openDoc
          ? new TextEncoder().encode(openDoc.getText())
          : await vscode.workspace.fs.readFile(resolvedUri);
        const importedExtension = resolvedUri.path.split(".").pop();

        if (importedExtension) {
          const importedAnalyzed = api.analyze(
            importedFileContent,
            importedExtension,
          );

          // Check if the imported file has client components
          for (const component of importedAnalyzed.components) {
            if (component.isClientComponent) {
              // Add all identifiers from this import as client components
              for (const identifier of importInfo.identifier) {
                clientComponentImports.add(identifier);
              }
              break;
            }
          }
        }
      }
    } catch {
      // Skip imports we can't resolve
    }
  }

  // Decorate JSX usages of client components
  for (const usage of analyzed.jsxUsages) {
    if (clientComponentImports.has(usage.componentName)) {
      const range = new vscode.Range(
        usage.range.start.line,
        usage.range.start.character,
        usage.range.end.line,
        usage.range.end.character,
      );
      usageRanges.push(range);
    }
  }

  // Apply decorations
  vscode.window.activeTextEditor?.setDecorations(
    componentDecoration,
    componentRanges,
  );
  vscode.window.activeTextEditor?.setDecorations(usageDecoration, usageRanges);

  // Log summary for users
  if (clientComponentImports.size > 0) {
    channel.info(
      `Found ${clientComponentImports.size} client component import(s): ${Array.from(clientComponentImports).join(", ")}`,
    );
  }
  if (usageRanges.length > 0) {
    channel.info(`Highlighted ${usageRanges.length} client component usage(s)`);
  }
}
