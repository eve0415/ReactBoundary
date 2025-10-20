# React Boundary Visualizer

Visualize Client Component boundaries so you understand what code ships to the client.

## Features

React Boundary Visualizer helps you visualize Client Component boundaries in your codebase by automatically detecting and highlighting components with `"use client"` directive. This extension is designed to help you:

- **Identify Client Components**: Instantly see when components have the `"use client"` directive with inline decorations
- **Visualize Client Boundaries**: See where Client Components are used in files without `"use client"` directive
- **Understand Bundle Impact**: Know what code gets included in your client bundle
- **Think About Performance**: Be reminded to consider prop sizes and data passed to Client Components

### How It Works

The extension provides context-aware decorations:

- **Gray "⚡ Client Boundary"** decorations appear on component declarations with `"use client"` directive (always shown)
- **Orange "⚡ Client Boundary"** decorations appear on JSX usages of Client Components in files without `"use client"` directive (hides when used within other Client Components to reduce noise)

### Smart Component Detection

React Boundary Visualizer uses a high-performance Rust + WebAssembly engine to accurately detect React components, including:

- Function components with JSX or React type annotations
- Arrow function components
- Components wrapped with `forwardRef`, `memo`, and other HOCs
- Default and named exports
- Transpiled/bundled code (supports jsx runtime calls)

### Works with Unsaved Changes

The extension analyzes your code in real-time, including unsaved changes, so you get immediate feedback as you type.

## Requirements

- VS Code version 1.105.0 or higher
- A React project with TypeScript or JavaScript (supports .js, .jsx, .ts, .tsx files)

No additional configuration or dependencies required!

## Extension Settings

This extension currently does not contribute any VS Code settings. It works automatically when you open JavaScript/TypeScript React files.

## Known Issues

- The extension requires valid syntax to analyze files. Syntax errors will prevent analysis until resolved.
- Only detects `"use client"` directives in directly imported files (does not track transitive dependencies). This is correct behavior - components without `"use client"` are dual-mode and can render on either server or client depending on the caller.

Please report any issues on the [GitHub repository](https://github.com/eve0415/ReactBoundaryVisualizer/issues).

## Release Notes

See the [Releases page](https://github.com/eve0415/ReactBoundaryVisualizer/releases) for version history and changelogs.

---

## Contributing

Pull requests are appreciated! Check out the [repository](https://github.com/eve0415/ReactBoundaryVisualizer) for development instructions and contribution guidelines.

This extension is built with:
- **Rust + WebAssembly** for high-performance AST parsing (using OxC)
- **TypeScript** for VS Code integration
- **WebAssembly Component Model** (WIT) for language interop

## License

MIT - See [LICENSE](LICENSE) for details.
