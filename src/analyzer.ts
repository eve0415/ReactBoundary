import { analyzeReactBoundary } from './analyzeReactBoundary';
import { componentDecoration, usageDecoration } from './decorations';
import {
  resolveImportedIdentifier,
  findImplementationFile,
} from './import-resolver';
import * as vscode from 'vscode';

/**
 * Analyze a document and decorate Client Components
 */
export async function analyzeDocument(
  editor: vscode.TextEditor | undefined,
  api: analyzeReactBoundary.Exports,
  channel: vscode.LogOutputChannel,
): Promise<void> {
  if (!editor) return;
  if (editor.document.isUntitled) return;

  const document = editor.document;

  const extension = document.uri.path.split('.').pop();
  if (!extension) {
    return;
  }

  // Read from the document buffer (not disk) to get unsaved changes
  const documentText = document.getText();
  const fileContent = new TextEncoder().encode(documentText);
  const analyzed = api.analyze(fileContent, extension);

  const componentRanges: vscode.Range[] = [];
  const usageRanges: vscode.Range[] = [];

  // Decorate local Client Components
  for (const component of analyzed.components) {
    if (component.isClientComponent) {
      // Place decoration at the end of the line for cleaner appearance
      const line = document.lineAt(component.range.start.line);
      const endOfLine = line.range.end;
      const range = new vscode.Range(endOfLine, endOfLine);
      componentRanges.push(range);
    }
  }

  // Check which imports are Client Components
  const clientComponentImports = new Set<string>();

  for (const importInfo of analyzed.imports) {
    try {
      const resolvedUri = await resolveImportedIdentifier(importInfo, document);

      if (resolvedUri) {
        // Try to find the implementation file if we resolved to a declaration file
        const implUri = await findImplementationFile(resolvedUri);
        const fileUri = implUri || resolvedUri;

        // Try to get the open document first (for unsaved changes), otherwise read from disk
        const openDoc = vscode.workspace.textDocuments.find(
          doc => doc.uri.toString() === fileUri.toString(),
        );
        const importedFileContent = openDoc
          ? new TextEncoder().encode(openDoc.getText())
          : await vscode.workspace.fs.readFile(fileUri);
        const importedExtension = fileUri.path.split('.').pop();

        if (importedExtension) {
          const importedAnalyzed = api.analyze(
            importedFileContent,
            importedExtension,
          );

          // Check if the imported file has Client Components
          for (const component of importedAnalyzed.components) {
            if (component.isClientComponent) {
              // Add all identifiers from this import as Client Components
              for (const identifier of importInfo.identifier) {
                clientComponentImports.add(identifier);
              }
              break;
            }
          }
        }
      }
    } catch {
      // Silently ignore import resolution errors (external packages, type-only imports, etc.)
    }
  }

  // Check if current file has "use client" directive
  // If so, don't decorate Client Component usages (already a Client Component)
  const isCurrentFileClient = analyzed.components.some(
    c => c.isClientComponent,
  );

  // Decorate JSX usages of Client Components only in files without "use client"
  if (!isCurrentFileClient) {
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
      `→ Found ${clientComponentImports.size} Client Component import${
        clientComponentImports.size === 1 ? '' : 's'
      }: ${Array.from(clientComponentImports).join(', ')}`,
    );
  }
  if (usageRanges.length > 0) {
    channel.info(
      `→ Highlighted ${usageRanges.length} Client Component usage${
        usageRanges.length === 1 ? '' : 's'
      }`,
    );
  }
}
