This file provides guidance to AI when working with code in this repository.

## Project Overview

ReactBoundary is a VS Code extension that detects and highlights React Server Component (RSC) boundaries. It identifies when client components are imported and used in React applications, helping developers understand what gets included in the client bundle and think carefully about performance implications (prop sizes, etc).

## Design Philosophy & React RSC Model

### Extension Purpose
This extension **visualizes** client boundaries, not warns or prevents them. Client boundaries are normal and necessary - the goal is to help developers:
- Remember when components are client-side rendering
- Think carefully about prop sizes and data passed across boundaries
- Understand what code gets included in the client bundle

### Critical RSC Insight: Dual-Mode Components
**A component without `"use client"` can run on BOTH server AND client depending on import context.**

Example:
```tsx
// SharedComponent.tsx (no "use client" directive)
export const SharedComponent = () => <div>Shared</div>

// ServerPage.tsx (no "use client")
import { SharedComponent } from './SharedComponent'
export default () => <SharedComponent /> // SharedComponent runs on SERVER

// ClientButton.tsx
"use client"
import { SharedComponent } from './SharedComponent'
export default () => <SharedComponent /> // SharedComponent runs on CLIENT
```

**Implication:** The extension tracks components based on **direct file `"use client"` directive**, NOT transitive dependencies. This is correct per React's model - only files with explicit directives are always-client.

### Why No Transitive Dependency Tracking
Transitive tracking would incorrectly mark dual-mode components as "always client". The current implementation correctly checks only if the imported file itself has `"use client"`, which aligns with React's actual behavior.

## Architecture

This is a **hybrid TypeScript/Rust codebase** that uses WebAssembly to bridge between the two languages:

### Rust Layer (Analysis Engine)
- **Purpose**: Fast AST parsing and React component analysis
- **Entry Point**: `src/lib.rs` - Main WASM interface and coordination logic
- **Core Logic**:
  - `src/component.rs` - React component detection (PascalCase naming, JSX/jsx runtime calls, type annotations)
  - `src/jsx.rs` - JSX usage collection and member expression handling
  - `src/range.rs` - Range/position conversion utilities
- **Build Target**: Compiles to `wasm32-unknown-unknown` WebAssembly module
- **Parser**: Uses OxC (Oxidation Compiler) for JavaScript/TypeScript parsing
- **Special Features**: Supports both JSX syntax and jsx runtime calls (for transpiled/bundled code)

### TypeScript Layer (VS Code Extension)
- **Entry Point**: `src/extension.ts` - initializes WASM module and sets up VS Code integration
- **Core Logic**:
  - `src/analyzer.ts` - Orchestrates document analysis and applies decorations
  - `src/import-resolver.ts` - Resolves import paths and finds implementation files (handles .d.ts → .tsx)
  - `src/decorations.ts` - Decoration type definitions for visual markers
- **WASM Bindings**: `src/analyzeReactBoundary.ts` - auto-generated TypeScript bindings from WIT definitions
- **Build Target**: Bundled to `dist/extension.js` via esbuild

### WebAssembly Interface (WIT)
- **Definition**: `wit/check.wit` defines the contract between Rust and TypeScript
- **Code Generation**: `pnpm run generate` converts WIT to TypeScript bindings using `wit2ts`

### How It Works
1. **Document Change**: Extension monitors active editor via `onDidChangeActiveTextEditor`
2. **Rust Analysis**: Document content passed to WASM `analyze()` function which:
   - Parses source code with OxC
   - Detects `"use client"` directive
   - Identifies React component exports:
     - Variable/const declarations: Checks PascalCase naming + (JSX/jsx runtime calls OR React type annotations)
     - Function declarations: Checks PascalCase naming + (JSX return OR React type annotations)
     - Default exports: Supports both identifier references and inline function declarations
   - Handles transpiled code patterns (jsx runtime calls, `__export()`, sequence expressions)
   - Extracts import statements (filters out type-only imports)
   - Collects JSX element usages (including member expressions like `<AlertDialog.Root>`)
3. **Import Resolution**: TypeScript uses VS Code's `executeDefinitionProvider` to resolve import paths
   - Handles declaration files (.d.ts) by finding actual implementation files (.tsx, .ts, etc.)
   - Supports unsaved changes by checking in-memory documents first
4. **Cross-File Analysis**: For each import, reads target file and analyzes if it contains client components (checks for `"use client"` directive in the imported file)
5. **Context Check**: Determines if current file is a client component
6. **Decoration**: Applies inline decorations showing "⚡ Client Boundary":
   - Gray decoration on component declarations with `"use client"` directive (always shown)
   - Orange decoration on JSX usages of imported client components (only when current file is server context)

## Build & Development Commands

### Building
```bash
pnpm run build                # Build both Rust WASM and TypeScript
pnpm run build:cargo          # Build Rust to WASM only
pnpm run build:esbuild        # Build TypeScript with esbuild only
```

### Development
```bash
pnpm run watch                # Watch mode for both WASM and TypeScript
pnpm run watch:esbuild        # Watch TypeScript only
pnpm run watch:tests          # Watch and recompile tests
```

### Code Generation
```bash
pnpm run generate             # Generate TypeScript bindings from WIT definitions
```

### Quality Checks
```bash
pnpm run lint                 # Lint with oxlint (includes type-aware checks)
pnpm run check-types          # TypeScript type checking (alias for tsc)
```

### Testing
```bash
pnpm test                     # Run all tests (builds first, then runs both test suites)
pnpm run test:rust            # Run Rust tests only (cargo test)
pnpm run test:vscode          # Run VS Code extension tests only
```

For **running a single Rust test**:
```bash
cargo test test_name          # Run specific Rust test
cargo test --lib              # Run all library tests
```

For **running a single VS Code test**:
```bash
pnpm run test:vscode -- --grep "test pattern"
```

## Rust Development Notes

### Adding WASM Targets
If building WASM for the first time or on a new machine:
```bash
rustup target add wasm32-unknown-unknown
rustup target add wasm32-wasip1
rustup target add wasm32-wasip2
```

### Modifying the WIT Interface
1. Edit `wit/check.wit` to change the WASM interface contract
2. Run `pnpm run generate` to regenerate TypeScript bindings
3. Update `src/lib.rs` to match the new interface
4. Update `src/analyzer.ts` to use the new bindings

### Component Detection Logic (src/component.rs)
A function/variable is detected as a React component if:
1. **PascalCase naming** (first letter uppercase)
2. AND one of:
   - Has React type annotation (`FC`, `FunctionComponent`, `VFC`, `ReactElement`, `ReactNode`, `Component`)
   - Contains JSX in return statement or arrow function body
   - Contains jsx runtime calls (`jsx()`, `jsxs()`, `jsxDEV()`) for transpiled/bundled code
   - Is wrapped in React HOCs (`React.forwardRef`, `React.memo`, `forwardRef`, `memo`)
   - Returns JSX from function body

**Special handling for transpiled code:**
- Detects jsx runtime imports from `"react/jsx-runtime"` (handles renamed imports like `import { jsx as foobar }`)
- Recognizes compiled patterns like `(0, _jsx)("div", {})` (sequence expressions)
- Parses `__export()` calls found in bundled code

## TypeScript Development Notes

### Import Resolution (src/import-resolver.ts)
The extension uses VS Code's `vscode.executeDefinitionProvider` to resolve import paths. This ensures imports are resolved the same way TypeScript language service does (respects tsconfig paths, node_modules, etc).

**Special features:**
- Handles declaration files (.d.ts, .d.mts, .d.cts) by attempting to find the actual implementation file
- Tries multiple extensions (.tsx, .ts, .jsx, .js, .mts, .mjs, .cts, .cjs) to locate source files
- Searches backward in the document to find import statements that may span multiple lines

### Unsaved Changes
The extension reads from the in-memory document buffer (via `document.getText()`) rather than disk, so it works with unsaved changes. When analyzing imported files, it checks `vscode.workspace.textDocuments` first before reading from disk.

### Decoration Types (src/decorations.ts)
Two decoration types are used:
- `componentDecoration`: Gray "⚡ Client Boundary", applied to local component declarations with `"use client"` (always shown)
- `usageDecoration`: Orange "⚡ Client Boundary", applied to JSX usages of imported client components (only when in server context)

### Context-Aware Decoration Logic
The extension implements context-aware decorations to avoid visual noise:
- **In server files** (no `"use client"`): Shows orange decorations on client component usages (marks server→client boundary)
- **In client files** (has `"use client"`): Hides orange decorations on client component usages (client→client is not a boundary)
- **Gray decorations**: Always shown on component declarations with `"use client"` directive

This is implemented in `src/analyzer.ts` by checking `analyzed.components.some(c => c.isClientComponent)` before applying usage decorations.

## Testing Strategy

### Rust Tests
Located in `src/lib.rs`, `src/component.rs`, and `src/jsx.rs` under `#[cfg(test)]` modules. Tests cover:
- **Component detection** (`src/component.rs`):
  - Arrow functions, function declarations, with/without type annotations
  - JSX syntax vs jsx runtime calls
  - React HOCs (forwardRef, memo)
  - Transpiled code patterns (sequence expressions, renamed imports)
- **Import parsing** (`src/lib.rs`):
  - Default, named, namespace imports
  - Type import filtering (both statement-level and specifier-level)
- **JSX usage collection** (`src/jsx.rs`):
  - Nested elements, fragments, member expressions
  - HTML elements vs React components (PascalCase filtering)
- **Export detection** (`src/lib.rs`):
  - Named exports, default exports, inline function exports
  - `__export()` calls in bundled code
- **Range/position tracking**: Accurate source location for decorations

### VS Code Integration Tests
Located in `src/test/`:
- `analyzer.test.ts` - Unit tests for TypeScript analysis logic
- `extension.test.ts` - Integration tests for VS Code extension behavior
- `example/` - Sample React files for testing:
  - `client.tsx` - Client component with `"use client"` directive
  - `server.tsx` - Server component importing client components
  - `client-uses-client.tsx` - Client component importing another client component (tests context-aware decorations)

## Important Constraints

- The extension only activates for JavaScript/TypeScript files (see `activationEvents` in package.json)
- WASM module must be compiled to `target/wasm32-unknown-unknown/debug/check_react_boundary.wasm`
- The extension bundle goes to `dist/extension.js` (not `out/`)
- OxC parser requires valid syntax - syntax errors will cause analysis to fail
