import * as vscode from 'vscode';

/**
 * Decoration for local client component declarations
 * Shows gray text annotation on components with "use client" directive
 */
export const componentDecoration = vscode.window.createTextEditorDecorationType(
  {
    after: {
      contentText: '⚡ Client Boundary',
      margin: '0 0 0 1rem',
      color: 'rgba(100, 100, 100, 0.7)',
      fontStyle: 'italic',
    },
  },
);

/**
 * Decoration for JSX usages of imported client components
 * Shows orange text annotation when client components are used in server context
 */
export const usageDecoration = vscode.window.createTextEditorDecorationType({
  after: {
    contentText: '⚡ Client Boundary',
    margin: '0 0 0 1rem',
    color: 'rgba(255, 150, 50, 0.8)',
    fontStyle: 'italic',
  },
});
