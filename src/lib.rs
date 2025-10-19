mod component;
mod jsx;
mod range;

use crate::analyze_react_boundary::check::types;
use oxc::allocator::Allocator;
use oxc::ast::ast::{BindingPatternKind, ExportDefaultDeclarationKind, ImportDeclarationSpecifier};
use oxc::ast::ast::{
    Declaration, Expression, ImportOrExportKind, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc::parser::{ParseOptions, Parser};
use oxc::span::{SourceType, Span};
use std::collections::{HashMap, HashSet};

wit_bindgen::generate!();

struct AnalyzeReactBoundary;

impl Guest for AnalyzeReactBoundary {
    fn analyze(content: Vec<u8>, extension: String) -> Result<AnalysisResult, String> {
        let source_text = String::from_utf8(content).unwrap();
        let source_type = SourceType::from_extension(&extension)
            .map_err(|e| format!("{}: {}", e.to_string().replace("\"", ""), extension))?;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, &source_text, source_type)
            .with_options(ParseOptions {
                parse_regular_expression: true,
                ..ParseOptions::default()
            })
            .parse();

        if ret.panicked
            && let Some(error) = ret.errors.into_iter().next()
        {
            let source_code_error = error.clone().with_source_code(source_text.clone());
            #[cfg(not(test))]
            log(&format!(
                "Error: {} with code {}",
                error.message, source_code_error
            ));

            return Err(format!(
                "Error: {} with code {}",
                error.message, source_code_error
            ));
        }

        let program = ret.program;

        let has_use_client_directive = program
            .directives
            .iter()
            .any(|directive| directive.directive == "use client");

        let imports = program
            .body
            .iter()
            .filter_map(|statement| {
                if let Statement::ImportDeclaration(import_declaration) = statement {
                    // We can just ignore type imports as it doesn't have a runtime impact
                    if import_declaration.import_kind == ImportOrExportKind::Type {
                        return None;
                    }
                    Some(types::ImportAnalysis {
                        identifier: import_declaration
                            .specifiers
                            .iter()
                            .flat_map(|specifier| {
                                specifier
                                    .into_iter()
                                    .filter_map(|specifier| match specifier {
                                        ImportDeclarationSpecifier::ImportSpecifier(spec) => {
                                            if spec.import_kind == ImportOrExportKind::Type {
                                                return None;
                                            }
                                            Some(spec.local.name.clone().to_string())
                                        }
                                        ImportDeclarationSpecifier::ImportDefaultSpecifier(
                                            spec,
                                        ) => Some(spec.local.name.clone().to_string()),
                                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(
                                            spec,
                                        ) => Some(spec.local.name.clone().to_string()),
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>(),
                        source: import_declaration.source.value.clone().to_string(),
                        source_span: range::string_literal_to_range(
                            &source_text,
                            import_declaration.source.span,
                        ),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Collect jsx runtime identifiers (functions imported from "react/jsx-runtime")
        // These can be renamed: import { jsx as foobar } from "react/jsx-runtime"
        let jsx_runtime_identifiers: HashSet<String> = imports
            .iter()
            .filter(|import| import.source == "react/jsx-runtime")
            .flat_map(|import| import.identifier.iter().cloned())
            .collect();

        // Track all React component declarations with their spans
        let mut component_declarations: HashMap<String, Span> = HashMap::new();

        // First pass: identify all React component variable declarations and function declarations
        for statement in program.body.iter() {
            match statement {
                Statement::VariableDeclaration(var_decl) => {
                    for declarator in var_decl.declarations.iter() {
                        if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                            let name = ident.name.to_string();

                            // This now handles both JSX syntax and jsx/jsxs runtime calls
                            let is_component = component::is_react_component(
                                &name,
                                &declarator.id,
                                &declarator.init,
                                &jsx_runtime_identifiers,
                            );

                            if is_component {
                                component_declarations.insert(name, ident.span);
                            }
                        }
                    }
                }
                Statement::FunctionDeclaration(func_decl) => {
                    if let Some(id) = &func_decl.id {
                        let name = id.name.to_string();

                        // Check if this is a React function component
                        if component::is_react_function_component(
                            &name,
                            &func_decl.return_type,
                            &func_decl.body,
                            &jsx_runtime_identifiers,
                        ) {
                            component_declarations.insert(name, id.span);
                        }
                    }
                }
                _ => {}
            }
        }

        // Second pass: extract exported component names with their spans
        let mut exported_components: Vec<(String, Span)> = Vec::new();

        // Helper function to register a component as exported
        fn register_component(
            name: String,
            span: Span,
            exported_components: &mut Vec<(String, Span)>,
            component_declarations: &mut HashMap<String, Span>,
        ) {
            exported_components.push((name.clone(), span));
            component_declarations.insert(name, span);
        }

        // Parse __export() calls to extract exports (common in bundled/compiled code)
        for statement in program.body.iter() {
            if let Statement::ExpressionStatement(expr_stmt) = statement
                && let Expression::CallExpression(call_expr) = &expr_stmt.expression
                && let Expression::Identifier(callee) = &call_expr.callee
                && callee.name == "__export"
                && call_expr.arguments.len() >= 2
                && let Some(second_arg) = call_expr.arguments.get(1)
                && let Some(expr) = second_arg.as_expression()
                && let Expression::ObjectExpression(obj_expr) = expr
            {
                // Extract export names from object properties
                for property in obj_expr.properties.iter() {
                    if let ObjectPropertyKind::ObjectProperty(prop) = property
                        && let PropertyKey::StaticIdentifier(key) = &prop.key
                    {
                        let export_name = key.name.to_string();

                        // Check if this export is a component we detected
                        if let Some(&span) = component_declarations.get(&export_name) {
                            exported_components.push((export_name, span));
                        }
                    }
                }
            }
        }

        for statement in program.body.iter() {
            match statement {
                // Handle default exports: export default ComponentName
                Statement::ExportDefaultDeclaration(export_decl) => {
                    match &export_decl.declaration {
                        ExportDefaultDeclarationKind::Identifier(ident) => {
                            let name = ident.name.to_string();
                            if let Some(&span) = component_declarations.get(&name) {
                                exported_components.push((name, span));
                            }
                        }
                        // Handle inline function declaration: export default function MyComponent() {}
                        ExportDefaultDeclarationKind::FunctionDeclaration(func_decl) => {
                            if let Some((name, span)) = component::analyze_function_declaration(
                                func_decl,
                                &jsx_runtime_identifiers,
                            ) {
                                register_component(
                                    name,
                                    span,
                                    &mut exported_components,
                                    &mut component_declarations,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                // Handle named exports: export const ComponentName = ... or export function ComponentName() {}
                Statement::ExportNamedDeclaration(export_decl) => {
                    if let Some(declaration) = &export_decl.declaration {
                        match declaration {
                            Declaration::VariableDeclaration(var_decl) => {
                                for declarator in var_decl.declarations.iter() {
                                    if let BindingPatternKind::BindingIdentifier(ident) =
                                        &declarator.id.kind
                                    {
                                        let name = ident.name.to_string();

                                        // Check if this is a React component
                                        if component::is_react_component(
                                            &name,
                                            &declarator.id,
                                            &declarator.init,
                                            &jsx_runtime_identifiers,
                                        ) {
                                            register_component(
                                                name,
                                                ident.span,
                                                &mut exported_components,
                                                &mut component_declarations,
                                            );
                                        }
                                    }
                                }
                            }
                            Declaration::FunctionDeclaration(func_decl) => {
                                if let Some((name, span)) = component::analyze_function_declaration(
                                    func_decl,
                                    &jsx_runtime_identifiers,
                                ) {
                                    register_component(
                                        name,
                                        span,
                                        &mut exported_components,
                                        &mut component_declarations,
                                    );
                                }
                            }
                            _ => {}
                        }
                    } else if !export_decl.specifiers.is_empty() {
                        // Handle export { ComponentName } (re-export of already declared variable)
                        use oxc::ast::ast::ModuleExportName;
                        for specifier in export_decl.specifiers.iter() {
                            // Get the exported name from the specifier
                            let exported_name = match &specifier.exported {
                                ModuleExportName::IdentifierName(ident) => ident.name.to_string(),
                                ModuleExportName::IdentifierReference(ident) => {
                                    ident.name.to_string()
                                }
                                ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
                            };
                            // Check if this is a component we already declared
                            if let Some(&span) = component_declarations.get(&exported_name) {
                                exported_components.push((exported_name, span));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let components = exported_components
            .into_iter()
            .map(|(name, span)| types::ComponentAnalysis {
                name,
                // Mark as client component ONLY if the "use client" directive is present
                is_client_component: has_use_client_directive,
                range: range::span_to_range(&source_text, span),
            })
            .collect::<Vec<_>>();

        #[cfg(not(test))]
        if !components.is_empty() && has_use_client_directive {
            log(&format!(
                "✓ Detected {} client component{}: {}",
                components.len(),
                if components.len() == 1 { "" } else { "s" },
                components
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Collect all imported identifiers
        let imported_identifiers: HashSet<String> = imports
            .iter()
            .flat_map(|import| import.identifier.iter().cloned())
            .collect();

        // Collect JSX element usages
        let jsx_usages_raw = jsx::collect_jsx_usages(&program.body);

        // Filter JSX usages to only those that match imports
        let jsx_usages = jsx_usages_raw
            .into_iter()
            .filter(|(name, _)| imported_identifiers.contains(name))
            .map(|(name, span)| types::JsxUsage {
                component_name: name,
                range: range::span_to_range(&source_text, span),
            })
            .collect::<Vec<_>>();

        Ok(AnalysisResult {
            imports,
            components,
            jsx_usages,
        })
    }
}

export!(AnalyzeReactBoundary);

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze_tsx(source: &str) -> Result<AnalysisResult, String> {
        AnalyzeReactBoundary::analyze(source.as_bytes().to_vec(), "tsx".to_string())
    }

    fn analyze_with_extension(source: &str, ext: &str) -> Result<AnalysisResult, String> {
        AnalyzeReactBoundary::analyze(source.as_bytes().to_vec(), ext.to_string())
    }

    #[test]
    fn test_analyze_client_component_file() {
        let source = r#"
"use client";
import type { FC } from "react";

const ClientComponent: FC = () => {
  return <div>Client</div>;
};

export default ClientComponent;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Should detect "use client" directive
        assert_eq!(result.components.len(), 1);
        assert_eq!(result.components[0].name, "ClientComponent");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_server_component_file() {
        let source = r#"
import type { FC } from "react";

const ServerComponent: FC = () => {
  return <div>Server</div>;
};

export default ServerComponent;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Should detect a component but not mark as a client
        assert_eq!(result.components.len(), 1);
        assert_eq!(result.components[0].name, "ServerComponent");
        assert!(!result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_named_export() {
        let source = r#"
"use client";

export const Button = () => {
  return <button>Click</button>;
};
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.components.len(), 1);
        assert_eq!(result.components[0].name, "Button");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_multiple_exports() {
        let source = r##"
"use client";

export const Button = () => <button>Click</button>;
export const Link = () => <a href="#">Link</a>;

const Header = () => <header>Header</header>;
export default Header;
        "##;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.components.len(), 3);

        let names: Vec<&str> = result.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Button"));
        assert!(names.contains(&"Link"));
        assert!(names.contains(&"Header"));

        // All should be client components
        assert!(result.components.iter().all(|c| c.is_client_component));
    }

    #[test]
    fn test_analyze_imports_default_specifier() {
        let source = r#"
import ClientComponent from "./client";
import AnotherComponent from "./another";

const App = () => {
  return <div>App</div>;
};

export default App;
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.imports.len(), 2);

        // Check first import
        assert_eq!(result.imports[0].identifier.len(), 1);
        assert_eq!(result.imports[0].identifier[0], "ClientComponent");
        assert_eq!(result.imports[0].source, "./client");

        // Check second import
        assert_eq!(result.imports[1].identifier.len(), 1);
        assert_eq!(result.imports[1].identifier[0], "AnotherComponent");
        assert_eq!(result.imports[1].source, "./another");
    }

    #[test]
    fn test_analyze_imports_named_specifier() {
        let source = r#"
import { Button, Link } from "./components";
import { FC } from "react";

const App: FC = () => <div>App</div>;
export default App;
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.imports.len(), 2);

        // Check named imports from components
        assert_eq!(result.imports[0].identifier.len(), 2);
        assert!(result.imports[0].identifier.contains(&"Button".to_string()));
        assert!(result.imports[0].identifier.contains(&"Link".to_string()));
        assert_eq!(result.imports[0].source, "./components");

        // Check FC import from react
        assert_eq!(result.imports[1].identifier.len(), 1);
        assert_eq!(result.imports[1].identifier[0], "FC");
        assert_eq!(result.imports[1].source, "react");
    }

    #[test]
    fn test_analyze_imports_namespace_specifier() {
        let source = r#"
import * as React from "react";
import * as Components from "./components";

const App = () => <div>App</div>;
export default App;
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.imports.len(), 2);

        // Check namespace imports
        assert_eq!(result.imports[0].identifier[0], "React");
        assert_eq!(result.imports[1].identifier[0], "Components");
    }

    #[test]
    fn test_analyze_ignores_type_imports() {
        let source = r#"
import type { FC } from "react";
import type { Props } from "./types";
import { Button } from "./button";

const App: FC = () => <div>App</div>;
export default App;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Should only include the Button import, not type imports
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].identifier[0], "Button");
        assert_eq!(result.imports[0].source, "./button");
    }

    #[test]
    fn test_analyze_jsx_usages() {
        let source = r#"
import ClientComponent from "./client";
import { Button, Link } from "./components";

const App = () => {
  return (
    <div>
      <ClientComponent />
      <Button />
      <Link />
    </div>
  );
};

export default App;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Should find all three JSX usages
        assert_eq!(result.jsx_usages.len(), 3);

        let usage_names: Vec<&str> = result
            .jsx_usages
            .iter()
            .map(|u| u.component_name.as_str())
            .collect();

        assert!(usage_names.contains(&"ClientComponent"));
        assert!(usage_names.contains(&"Button"));
        assert!(usage_names.contains(&"Link"));
    }

    #[test]
    fn test_analyze_jsx_usages_filtered_to_imports() {
        let source = r#"
import { Button } from "./components";

// LocalComponent is NOT imported, so it should NOT be in jsx_usages
const LocalComponent = () => <div>Local</div>;

const App = () => {
  return (
    <div>
      <Button />
      <LocalComponent />
    </div>
  );
};

export default App;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Should only include Button (imported), not LocalComponent (local)
        assert_eq!(result.jsx_usages.len(), 1);
        assert_eq!(result.jsx_usages[0].component_name, "Button");
    }

    #[test]
    fn test_analyze_complete_flow() {
        let source = r#"
"use client";
import { ServerButton } from "./server";
import type { FC } from "react";

export const ClientButton: FC = () => {
  return (
    <button>
      <ServerButton />
    </button>
  );
};

const ClientHeader = () => {
  return <header>Header</header>;
};

export default ClientHeader;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Check imports
        assert_eq!(result.imports.len(), 1); // type imports are ignored
        assert_eq!(result.imports[0].identifier[0], "ServerButton");

        // Check components
        assert_eq!(result.components.len(), 2);
        assert!(result.components.iter().all(|c| c.is_client_component));

        // Check JSX usages
        assert_eq!(result.jsx_usages.len(), 1);
        assert_eq!(result.jsx_usages[0].component_name, "ServerButton");
    }

    #[test]
    fn test_analyze_invalid_syntax() {
        let source = "const x = {{{";

        let result = analyze_tsx(source);

        // Should return an error
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_invalid_extension() {
        let source = "const x = 10;";

        let result = analyze_with_extension(source, "invalid");

        // Should return an error for invalid extension
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_range_positions() {
        let source = r#"
const MyComponent = () => <div />;
export default MyComponent;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Check that ranges are set correctly
        assert_eq!(result.components.len(), 1);
        let component = &result.components[0];

        // Component name should be on line 1 (0-indexed)
        assert_eq!(component.range.start.line, 1);
        assert!(component.range.start.character > 0);
        assert_eq!(component.range.end.line, 1);
    }

    #[test]
    fn test_analyze_import_source_span() {
        let source = r#"import X from "./client";"#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.imports.len(), 1);

        // source_span should point inside the string (after opening quote)
        let import = &result.imports[0];
        assert_eq!(import.source_span.start.line, 0);

        // Character should be after the opening quote
        // "import X from "./client";"
        //                ^--- should be around character 15
        assert!(import.source_span.start.character >= 14);
        assert!(import.source_span.start.character <= 16);
    }

    #[test]
    fn test_analyze_function_declaration_export() {
        let source = r#"
"use client";

export function MyComponent() {
  return <div>Function Declaration</div>;
}
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.components.len(), 1);
        assert_eq!(result.components[0].name, "MyComponent");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_mixed_exports() {
        let source = r#"
"use client";

export const ArrowComponent = () => <div>Arrow</div>;
export function FunctionComponent() {
  return <div>Function</div>;
}
const DefaultComponent = () => <div>Default</div>;
export default DefaultComponent;
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(result.components.len(), 3);

        let names: Vec<&str> = result.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ArrowComponent"));
        assert!(names.contains(&"FunctionComponent"));
        assert!(names.contains(&"DefaultComponent"));

        // All should be client components
        assert!(result.components.iter().all(|c| c.is_client_component));
    }

    #[test]
    fn test_analyze_jsx_runtime_calls() {
        // Test that bundled code using jsx() runtime calls is detected
        let source = r#"
"use client";
import { jsx as _jsx } from "react/jsx-runtime";

const Button = () => {
  return _jsx("button", { children: "Click me" });
};

export default Button;
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            1,
            "Should detect component using jsx runtime call"
        );
        assert_eq!(result.components[0].name, "Button");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_jsxs_runtime_calls() {
        // Test jsxs (for multiple children)
        let source = r#"
"use client";
import { jsxs as _jsxs } from "react/jsx-runtime";

export const Container = () => {
  return _jsxs("div", { children: ["Hello", "World"] });
};
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            1,
            "Should detect component using jsxs runtime call"
        );
        assert_eq!(result.components[0].name, "Container");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_react_forwardref() {
        // Test React.forwardRef with jsx runtime calls
        let source = r#"
"use client";
import React from "react";
import { jsx as _jsx } from "react/jsx-runtime";

const Button = React.forwardRef((props, ref) => {
  return _jsx("button", { ref, ...props });
});

export default Button;
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            1,
            "Should detect React.forwardRef component"
        );
        assert_eq!(result.components[0].name, "Button");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_forwardref_direct_import() {
        // Test forwardRef (direct import) with jsx runtime calls
        let source = r#"
"use client";
import { forwardRef } from "react";
import { jsx as _jsx } from "react/jsx-runtime";

const Input = forwardRef((props, ref) => {
  return _jsx("input", { ref, ...props });
});

export { Input };
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            1,
            "Should detect forwardRef component"
        );
        assert_eq!(result.components[0].name, "Input");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_jsx_member_expressions() {
        let source = r#"
import { AlertDialog } from "radix-ui";

const App = () => {
  return (
    <div>
      <AlertDialog.Root>
        <AlertDialog.Trigger>Open</AlertDialog.Trigger>
        <AlertDialog.Content>
          <AlertDialog.Title>Alert</AlertDialog.Title>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </div>
  );
};

export default App;
        "#;

        let result = analyze_tsx(source).unwrap();

        // Should detect the AlertDialog import
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].identifier[0], "AlertDialog");
        assert_eq!(result.imports[0].source, "radix-ui");

        // Should detect all 4 member expression usages (Root, Trigger, Content, Title)
        // All of them should be tracked as "AlertDialog" (the base identifier)
        assert_eq!(
            result.jsx_usages.len(),
            4,
            "Should detect 4 AlertDialog member expression usages"
        );
        assert!(
            result
                .jsx_usages
                .iter()
                .all(|u| u.component_name == "AlertDialog")
        );
    }

    #[test]
    fn test_analyze_compiled_jsx_sequence_expression() {
        // Test the compiled pattern found in npm packages like radix-ui:
        // (0, _jsx)("div", props) - SequenceExpression wrapped in parentheses
        let source = r#"
"use client";
import { jsx as _jsx } from "react/jsx-runtime";

const AlertDialog = (props) => {
  return (0, _jsx)("div", {children: "test"});
};

export default AlertDialog;
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            1,
            "Should detect component using compiled jsx runtime pattern"
        );
        assert_eq!(result.components[0].name, "AlertDialog");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_renamed_jsx_import() {
        // Test that renamed jsx runtime imports work correctly
        // import { jsx as foobar } from "react/jsx-runtime"
        let source = r#"
"use client";
import { jsx as foobar, jsxs as barfoo } from "react/jsx-runtime";

const MyComponent = () => {
  return foobar("div", {children: "Hello"});
};

const MyOtherComponent = () => {
  return barfoo("div", {children: ["Hello", "World"]});
};

export { MyComponent, MyOtherComponent };
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            2,
            "Should detect both components with renamed jsx imports"
        );
        assert!(result.components.iter().any(|c| c.name == "MyComponent"));
        assert!(
            result
                .components
                .iter()
                .any(|c| c.name == "MyOtherComponent")
        );
        assert!(result.components.iter().all(|c| c.is_client_component));
    }

    #[test]
    fn test_analyze_inline_export_default_function() {
        // Test export default function (inline declaration, not reference)
        let source = r#"
"use client";

export default function MyComponent() {
  return <div>Hello</div>;
}
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            1,
            "Should detect inline export default function declaration"
        );
        assert_eq!(result.components[0].name, "MyComponent");
        assert!(result.components[0].is_client_component);
    }

    #[test]
    fn test_analyze_multiple_components_with_inline_default() {
        // Test file with both named exports and inline default export
        let source = r#"
"use client";
import type { FC } from "react";

export const ClientUsesClientNamedFunction: FC = () => {
  return <div>Named export</div>;
};

export default function ClientUsesClientDefaultFunction() {
  return <div>Default export</div>;
}
        "#;

        let result = analyze_tsx(source).unwrap();

        assert_eq!(
            result.components.len(),
            2,
            "Should detect both named and inline default export"
        );

        let names: Vec<&str> = result.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ClientUsesClientNamedFunction"));
        assert!(names.contains(&"ClientUsesClientDefaultFunction"));

        // All should be client components
        assert!(result.components.iter().all(|c| c.is_client_component));
    }
}
