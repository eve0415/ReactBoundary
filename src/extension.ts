import { analyzeDocument } from './analyzer';
import { analyzeReactBoundary } from './analyzeReactBoundary';
// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import { Memory, WasmContext } from '@vscode/wasm-component-model';
import * as vscode from 'vscode';

// This method is called when your extension is activated
// Your extension is activated the very first time the command is executed
export async function activate(context: vscode.ExtensionContext) {
  // The channel for printing the result.
  const channel = vscode.window.createOutputChannel('ReactBoundary', {
    log: true,
  });
  context.subscriptions.push(channel);

  // Load the Wasm module
  const filename = vscode.Uri.joinPath(
    context.extensionUri,
    'target',
    'wasm32-unknown-unknown',
    process.env.NODE_ENV === 'production' ? 'release' : 'debug',
    'check_react_boundary.wasm',
  );
  const bits = await vscode.workspace.fs.readFile(filename);
  const module = await WebAssembly.compile(bits as Uint8Array<ArrayBuffer>);

  // The implementation of the log function that is called from WASM
  const service: analyzeReactBoundary.Imports = {
    log: (msg: string) => {
      channel.info(msg);
    },
  };

  // The context for the WASM module
  const wasmContext: WasmContext.Default = new WasmContext.Default();
  // Create the bindings to import the log function into the WASM module
  const imports = analyzeReactBoundary._.imports.create(service, wasmContext);
  // Instantiate the module
  const instance = await WebAssembly.instantiate(module, imports);

  // Bind the WASM memory to the context
  wasmContext.initialize(new Memory.Default(instance.exports));

  // Bind the TypeScript Api
  const api = analyzeReactBoundary._.exports.bind(
    instance.exports as analyzeReactBoundary._.Exports,
    wasmContext,
  );

  // Listen to document changes
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(async e => {
      await analyzeDocument(e, api, channel);
    }),
  );

  // Kick off first run when extension is activated
  await analyzeDocument(vscode.window.activeTextEditor, api, channel);
}

// This method is called when your extension is deactivated
export function deactivate() {}
