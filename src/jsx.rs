use oxc::ast::ast::{Expression, JSXElementName, JSXMemberExpressionObject, Statement};
use oxc::span::Span;

// ============================================================================
// PUBLIC API
// ============================================================================

/// Public function to collect all JSX usages from the program body
pub(crate) fn collect_jsx_usages(statements: &[Statement]) -> Vec<(String, Span)> {
    let mut usages = Vec::new();
    for statement in statements {
        collect_jsx_from_statement(statement, &mut usages);
    }
    usages
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Recursively collect JSX element usages from a statement
fn collect_jsx_from_statement(stmt: &Statement, usages: &mut Vec<(String, Span)>) {
    match stmt {
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                collect_jsx_from_expression(arg, usages);
            }
        }
        Statement::ExpressionStatement(expr_stmt) => {
            collect_jsx_from_expression(&expr_stmt.expression, usages);
        }
        Statement::VariableDeclaration(var_decl) => {
            for declarator in var_decl.declarations.iter() {
                if let Some(init) = &declarator.init {
                    collect_jsx_from_expression(init, usages);
                }
            }
        }
        Statement::ExportNamedDeclaration(export_decl) => {
            // Handle: export const Component = () => <div/>
            if let Some(declaration) = &export_decl.declaration {
                match declaration {
                    oxc::ast::ast::Declaration::VariableDeclaration(var_decl) => {
                        for declarator in var_decl.declarations.iter() {
                            if let Some(init) = &declarator.init {
                                collect_jsx_from_expression(init, usages);
                            }
                        }
                    }
                    oxc::ast::ast::Declaration::FunctionDeclaration(func_decl) => {
                        if let Some(body) = &func_decl.body {
                            for stmt in body.statements.iter() {
                                collect_jsx_from_statement(stmt, usages);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Statement::ExportDefaultDeclaration(export_decl) => {
            // Handle: export default () => <div/>
            // ExportDefaultDeclarationKind inherits from Expression, so we use as_expression()
            if let Some(expr) = export_decl.declaration.as_expression() {
                collect_jsx_from_expression(expr, usages);
            } else {
                // Handle FunctionDeclaration case
                if let oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(func_decl) =
                    &export_decl.declaration
                    && let Some(body) = &func_decl.body
                {
                    for stmt in body.statements.iter() {
                        collect_jsx_from_statement(stmt, usages);
                    }
                }
            }
        }
        Statement::BlockStatement(block) => {
            for stmt in block.body.iter() {
                collect_jsx_from_statement(stmt, usages);
            }
        }
        Statement::IfStatement(if_stmt) => {
            collect_jsx_from_statement(&if_stmt.consequent, usages);
            if let Some(alternate) = &if_stmt.alternate {
                collect_jsx_from_statement(alternate, usages);
            }
        }
        _ => {}
    }
}

/// Recursively collect JSX element usages from an expression
fn collect_jsx_from_expression(expr: &Expression, usages: &mut Vec<(String, Span)>) {
    match expr {
        Expression::JSXElement(jsx_elem) => {
            collect_jsx_from_element(jsx_elem, usages);
        }
        Expression::JSXFragment(jsx_frag) => {
            for child in jsx_frag.children.iter() {
                if let oxc::ast::ast::JSXChild::Element(child_elem) = child {
                    collect_jsx_from_element(child_elem, usages);
                }
            }
        }
        Expression::ParenthesizedExpression(paren) => {
            // Unwrap the parentheses and process the inner expression
            collect_jsx_from_expression(&paren.expression, usages);
        }
        Expression::ArrowFunctionExpression(arrow) => {
            for stmt in arrow.body.statements.iter() {
                collect_jsx_from_statement(stmt, usages);
            }
        }
        Expression::FunctionExpression(func) => {
            if let Some(body) = &func.body {
                for stmt in body.statements.iter() {
                    collect_jsx_from_statement(stmt, usages);
                }
            }
        }
        _ => {}
    }
}

/// Recursively collect JSX element usages from a JSXElement
fn collect_jsx_from_element(
    jsx_elem: &oxc::ast::ast::JSXElement,
    usages: &mut Vec<(String, Span)>,
) {
    match &jsx_elem.opening_element.name {
        JSXElementName::Identifier(ident) => {
            let name = ident.name.to_string();
            // Only track PascalCase components (user-defined components)
            if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                // Use the entire JSX element's span so decoration appears after closing tag
                usages.push((name.clone(), jsx_elem.span));
            }
        }
        JSXElementName::IdentifierReference(ident) => {
            let name = ident.name.to_string();
            // Only track PascalCase components (user-defined components)
            if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                // Use the entire JSX element's span so decoration appears after closing tag
                usages.push((name.clone(), jsx_elem.span));
            }
        }
        JSXElementName::MemberExpression(member_expr) => {
            // For member expressions like <AlertDialog.Root>, we need to extract the base object
            // We track the base identifier (e.g., "AlertDialog") so we can match it against imports
            if let JSXMemberExpressionObject::IdentifierReference(base_ident) = &member_expr.object
            {
                let base_name = base_ident.name.to_string();
                if base_name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    // Use the entire JSX element's span so decoration appears after closing tag
                    usages.push((base_name, jsx_elem.span));
                }
            }
        }
        JSXElementName::NamespacedName(_) => {
            // Skip namespaced JSX elements (rarely used)
        }
        JSXElementName::ThisExpression(_) => {
            // Skip this.Component patterns (class component style)
        }
    }
    // Collect from children
    for child in jsx_elem.children.iter() {
        if let oxc::ast::ast::JSXChild::Element(child_elem) = child {
            collect_jsx_from_element(child_elem, usages);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use oxc::allocator::Allocator;
    use oxc::parser::Parser;
    use oxc::span::SourceType;

    #[test]
    fn test_collect_jsx_usages_in_return() {
        let source = r#"
            const App = () => {
                return <ClientComponent />;
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        assert_eq!(usages.len(), 1, "Should find 1 JSX usage");
        assert_eq!(usages[0].0, "ClientComponent");
    }

    #[test]
    fn test_collect_jsx_usages_with_parentheses() {
        let source = r#"
            const App = () => {
                return (
                    <ClientComponent />
                );
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        assert_eq!(usages.len(), 1, "Should find JSX inside parentheses");
        assert_eq!(usages[0].0, "ClientComponent");
    }

    #[test]
    fn test_collect_jsx_usages_nested_elements() {
        let source = r#"
            const App = () => {
                return (
                    <div>
                        <ClientComponent />
                        <AnotherComponent />
                    </div>
                );
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        assert_eq!(usages.len(), 2, "Should find 2 nested JSX components");
        assert!(usages.iter().any(|(name, _)| name == "ClientComponent"));
        assert!(usages.iter().any(|(name, _)| name == "AnotherComponent"));
    }

    #[test]
    fn test_collect_jsx_usages_ignore_html_elements() {
        let source = r#"
            const App = () => {
                return (
                    <div>
                        <span>text</span>
                        <ClientComponent />
                    </div>
                );
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        assert_eq!(usages.len(), 1, "Should ignore lowercase HTML elements");
        assert_eq!(usages[0].0, "ClientComponent");
    }

    #[test]
    fn test_collect_jsx_usages_in_fragment() {
        let source = r#"
            const App = () => {
                return (
                    <>
                        <ClientComponent />
                        <OtherComponent />
                    </>
                );
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        assert_eq!(usages.len(), 2, "Should find JSX in fragments");
    }

    #[test]
    fn test_collect_jsx_usages_in_if_statement() {
        let source = r#"
            const App = () => {
                if (true) {
                    return <ClientComponent />;
                }
                return <OtherComponent />;
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        assert_eq!(usages.len(), 2, "Should find JSX in if statements");
    }

    #[test]
    fn test_collect_jsx_usages_in_variable_declaration() {
        let source = r#"
            const App = () => {
                const element = <ClientComponent />;
                return element;
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        assert_eq!(usages.len(), 1, "Should find JSX in variable declarations");
        assert_eq!(usages[0].0, "ClientComponent");
    }

    #[test]
    fn test_collect_jsx_usages_member_expression() {
        let source = r#"
            const App = () => {
                return (
                    <div>
                        <AlertDialog.Root>
                            <AlertDialog.Trigger>Open</AlertDialog.Trigger>
                            <AlertDialog.Content>
                                <AlertDialog.Title>Title</AlertDialog.Title>
                            </AlertDialog.Content>
                        </AlertDialog.Root>
                    </div>
                );
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let usages = collect_jsx_usages(&ret.program.body);

        // Should find all AlertDialog usages (Root, Trigger, Content, Title),
        // but they all resolve to the base identifier "AlertDialog"
        assert_eq!(usages.len(), 4, "Should find 4 member expression usages");
        assert!(
            usages.iter().all(|(name, _)| name == "AlertDialog"),
            "All usages should be 'AlertDialog'"
        );
    }
}
