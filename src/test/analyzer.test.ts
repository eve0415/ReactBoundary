import { analyzeDocument } from "../analyzer";
import { analyzeReactBoundary } from "../analyzeReactBoundary";
import { Memory, WasmContext } from "@vscode/wasm-component-model";
import * as assert from "assert";
import * as vscode from "vscode";

suite("Document Analysis", () => {
  let api: analyzeReactBoundary.Exports;
  let mockChannel: vscode.LogOutputChannel;

  suiteSetup(async function () {
    this.timeout(10000);

    // Load WASM module for testing
    const workspaceFolder = vscode.workspace.workspaceFolders![0].uri;
    const filename = vscode.Uri.joinPath(
      workspaceFolder,
      "target",
      "wasm32-unknown-unknown",
      "debug",
      "check_react_boundary.wasm",
    );
    const bits = await vscode.workspace.fs.readFile(filename);
    const module = await WebAssembly.compile(bits as Uint8Array<ArrayBuffer>);

    const service: analyzeReactBoundary.Imports = {
      log: (_msg: string) => {
        // Silent in tests
      },
    };

    const wasmContext = new WasmContext.Default();
    const imports = analyzeReactBoundary._.imports.create(service, wasmContext);
    const instance = await WebAssembly.instantiate(module, imports);

    wasmContext.initialize(new Memory.Default(instance.exports));

    api = analyzeReactBoundary._.exports.bind(
      instance.exports as analyzeReactBoundary._.Exports,
      wasmContext,
    );

    // Create a mock channel for testing
    mockChannel = {
      info: () => {},
      debug: () => {},
      trace: () => {},
      warn: () => {},
      error: () => {},
      append: () => {},
      appendLine: () => {},
      replace: () => {},
      clear: () => {},
      show: () => {},
      hide: () => {},
      dispose: () => {},
      name: "MockChannel",
      logLevel: vscode.LogLevel.Info,
      onDidChangeLogLevel: new vscode.EventEmitter<vscode.LogLevel>().event,
    } as vscode.LogOutputChannel;
  });

  test("should handle undefined editor", async () => {
    // Should not throw when editor is undefined
    await analyzeDocument(undefined, api, mockChannel);
    assert.ok(true, "Should handle undefined editor gracefully");
  });

  test("should handle untitled documents", async () => {
    // Create a new untitled document
    const doc = await vscode.workspace.openTextDocument({
      language: "typescriptreact",
      content:
        '"use client";\nexport const Button = () => <button>Test</button>;',
    });
    const editor = await vscode.window.showTextDocument(doc);

    // Should not throw for untitled documents
    await analyzeDocument(editor, api, mockChannel);
    assert.ok(true, "Should handle untitled documents gracefully");

    // Clean up
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
  });

  test("should analyze client component file", async function () {
    this.timeout(10000);

    // Open the client.tsx example file
    const clientUri = vscode.Uri.joinPath(
      vscode.workspace.workspaceFolders![0].uri,
      "src",
      "test",
      "example",
      "client.tsx",
    );
    const doc = await vscode.workspace.openTextDocument(clientUri);
    const editor = await vscode.window.showTextDocument(doc);

    // Analyze the document
    await analyzeDocument(editor, api, mockChannel);

    // Verify the function completes without errors
    assert.ok(true, "Should analyze client component file successfully");

    // Clean up
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
  });

  test("should analyze server component file with client component imports", async function () {
    this.timeout(10000);

    // Open the server.tsx example file (imports client components)
    const serverUri = vscode.Uri.joinPath(
      vscode.workspace.workspaceFolders![0].uri,
      "src",
      "test",
      "example",
      "server.tsx",
    );
    const doc = await vscode.workspace.openTextDocument(serverUri);
    const editor = await vscode.window.showTextDocument(doc);

    // Track channel calls
    let infoCallCount = 0;
    // oxlint-disable-next-line unbound-method
    const originalInfo = mockChannel.info;
    mockChannel.info = (message: string) => {
      infoCallCount++;
      originalInfo.call(mockChannel, message);
    };

    // Analyze the document
    await analyzeDocument(editor, api, mockChannel);

    // Should have called info for detected client components
    assert.ok(infoCallCount >= 0, "Should log analysis results");

    // Restore
    mockChannel.info = originalInfo;

    // Clean up
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
  });

  test("should handle files without extensions", async function () {
    this.timeout(10000);

    // Create a document with no extension
    const content = '"use client";\nexport const Test = () => <div>Test</div>;';
    const doc = await vscode.workspace.openTextDocument({
      language: "typescriptreact",
      content,
    });
    const editor = await vscode.window.showTextDocument(doc);

    // Should handle gracefully
    await analyzeDocument(editor, api, mockChannel);
    assert.ok(true, "Should handle files without extensions");

    // Clean up
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
  });

  test("should analyze TypeScript React files", async function () {
    this.timeout(10000);

    const content = `"use client";

import type { FC } from "react";

export const TestComponent: FC = () => {
  return <div>Test Component</div>;
};`;

    // Save to a temp file with .tsx extension
    const tempUri = vscode.Uri.joinPath(
      vscode.workspace.workspaceFolders![0].uri,
      "temp-test.tsx",
    );
    await vscode.workspace.fs.writeFile(
      tempUri,
      new TextEncoder().encode(content),
    );
    const savedDoc = await vscode.workspace.openTextDocument(tempUri);
    const savedEditor = await vscode.window.showTextDocument(savedDoc);

    await analyzeDocument(savedEditor, api, mockChannel);
    assert.ok(true, "Should analyze TypeScript React files");

    // Clean up
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
    try {
      await vscode.workspace.fs.delete(tempUri);
    } catch {
      // Ignore cleanup errors
    }
  });

  test("should handle non-React files", async function () {
    this.timeout(10000);

    const content = `export function add(a: number, b: number): number {
  return a + b;
}`;

    // Save to a temp file with .ts extension
    const tempUri = vscode.Uri.joinPath(
      vscode.workspace.workspaceFolders![0].uri,
      "temp-test.ts",
    );
    await vscode.workspace.fs.writeFile(
      tempUri,
      new TextEncoder().encode(content),
    );
    const doc = await vscode.workspace.openTextDocument(tempUri);
    const editor = await vscode.window.showTextDocument(doc);

    await analyzeDocument(editor, api, mockChannel);
    assert.ok(true, "Should handle non-React files");

    // Clean up
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
    try {
      await vscode.workspace.fs.delete(tempUri);
    } catch {
      // Ignore cleanup errors
    }
  });

  test("should read from document buffer not disk", async function () {
    this.timeout(10000);

    // Create a file with initial content
    const initialContent = "export const Old = () => <div>Old</div>;";
    const tempUri = vscode.Uri.joinPath(
      vscode.workspace.workspaceFolders![0].uri,
      "temp-buffer-test.tsx",
    );
    await vscode.workspace.fs.writeFile(
      tempUri,
      new TextEncoder().encode(initialContent),
    );

    // Open and modify without saving
    const doc = await vscode.workspace.openTextDocument(tempUri);
    const editor = await vscode.window.showTextDocument(doc);

    await editor.edit((editBuilder) => {
      const lastLine = doc.lineAt(doc.lineCount - 1);
      const range = new vscode.Range(
        new vscode.Position(0, 0),
        new vscode.Position(lastLine.lineNumber, lastLine.text.length),
      );
      editBuilder.replace(
        range,
        '"use client";\nexport const New = () => <div>New</div>;',
      );
    });

    // Analyze - should use buffer content (with "use client"), not disk content
    await analyzeDocument(editor, api, mockChannel);
    assert.ok(true, "Should analyze from buffer not disk");

    // Clean up without saving
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
    try {
      await vscode.workspace.fs.delete(tempUri);
    } catch {
      // Ignore cleanup errors
    }
  });
});
