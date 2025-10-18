mod analyze;
mod range;

use crate::analyze_react_boundary::check::types;
use oxc::allocator::Allocator;
use oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier;
use oxc::ast::ast::Statement::{
    ExportDefaultDeclaration, ExportNamedDeclaration, ImportDeclaration, VariableDeclaration,
};
use oxc::ast::ast::{BindingPatternKind, ExportDefaultDeclarationKind};
use oxc::ast::ast::{Declaration, ImportOrExportKind};
use oxc::parser::{ParseOptions, Parser};
use oxc::span::{SourceType, Span};
use std::collections::HashMap;

wit_bindgen::generate!();

struct AnalyzeReactBoundary;

impl Guest for AnalyzeReactBoundary {
    fn analyze(content: Vec<u8>, extension: String) -> Result<AnalysisResult, String> {
        let source_text = String::from_utf8(content).unwrap();
        let source_type = SourceType::from_extension(&extension).map_err(|e| {
            format!(
                "Failed to parse extension: {}",
                e.to_string().replace("\"", "")
            )
        })?;

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
                if let ImportDeclaration(import_declaration) = statement {
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
                                    .filter_map(|specifier| {
                                        if let ImportSpecifier(specifier) = specifier {
                                            if specifier.import_kind == ImportOrExportKind::Type {
                                                return None;
                                            }
                                            Some(specifier.local.name.clone().to_string())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>(),
                        source: import_declaration.source.value.clone().to_string(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Track all React component declarations with their spans
        let mut component_declarations: HashMap<String, Span> = HashMap::new();

        // First pass: identify all React component variable declarations
        for statement in program.body.iter() {
            if let VariableDeclaration(var_decl) = statement {
                for declarator in var_decl.declarations.iter() {
                    if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                        let name = ident.name.to_string();

                        // Check if this is a React component
                        if analyze::is_react_component(&name, &declarator.id, &declarator.init) {
                            component_declarations.insert(name, ident.span);
                        }
                    }
                }
            }
        }

        // Second pass: extract exported component names with their spans
        let mut exported_components: Vec<(String, Span)> = Vec::new();

        for statement in program.body.iter() {
            match statement {
                // Handle default exports: export default ComponentName
                ExportDefaultDeclaration(export_decl) => {
                    if let ExportDefaultDeclarationKind::Identifier(ident) =
                        &export_decl.declaration
                    {
                        let name = ident.name.to_string();
                        if let Some(&span) = component_declarations.get(&name) {
                            exported_components.push((name, span));
                        }
                    }
                }
                // Handle named exports: export const ComponentName = ...
                ExportNamedDeclaration(export_decl) => {
                    if let Some(declaration) = &export_decl.declaration
                        && let Declaration::VariableDeclaration(var_decl) = declaration
                    {
                        for declarator in var_decl.declarations.iter() {
                            if let BindingPatternKind::BindingIdentifier(ident) =
                                &declarator.id.kind
                            {
                                let name = ident.name.to_string();

                                // Check if this is a React component
                                if analyze::is_react_component(
                                    &name,
                                    &declarator.id,
                                    &declarator.init,
                                ) {
                                    let span = ident.span;
                                    exported_components.push((name.clone(), span));
                                    component_declarations.insert(name, span);
                                }
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
                is_client_component: has_use_client_directive,
                range: range::span_to_range(&source_text, span),
            })
            .collect::<Vec<_>>();

        Ok(AnalysisResult {
            imports,
            components,
        })
    }
}

export!(AnalyzeReactBoundary);
