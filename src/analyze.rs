use oxc::ast::ast::TSTypeName::IdentifierReference;
use oxc::ast::ast::{BindingPattern, Expression, JSXElementName, Statement, TSType};
use oxc::span::Span;

/// Check if a type annotation is a React component type
fn is_react_type_annotation(ts_type: &TSType) -> bool {
    match ts_type {
        TSType::TSTypeReference(type_ref) => {
            // Check if the type name is a React component type
            if let IdentifierReference(ident) = &type_ref.type_name {
                matches!(
                    ident.name.as_str(),
                    "FC" | "FunctionComponent" | "VFC" | "ReactElement" | "ReactNode" | "Component"
                )
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Helper function to check if statements contain JSX return
fn has_jsx_return(statements: &[Statement]) -> bool {
    statements.iter().any(|stmt| {
        if let Statement::ReturnStatement(ret) = stmt {
            if let Some(arg) = &ret.argument {
                matches!(arg, Expression::JSXElement(_) | Expression::JSXFragment(_))
            } else {
                false
            }
        } else {
            false
        }
    })
}

/// Check if an expression contains JSX
fn contains_jsx(expr: &Expression) -> bool {
    match expr {
        Expression::JSXElement(_) | Expression::JSXFragment(_) => true,
        Expression::ArrowFunctionExpression(arrow) => {
            // For arrow functions, check if it's an expression body (implicit return)
            // If expression is true, the body contains a single expression
            // If expression is false, it has a block body with statements
            if arrow.expression {
                // Implicit return: () => <div/>
                // The body will have a single ExpressionStatement
                arrow.body.statements.iter().any(|stmt| {
                    matches!(
                        stmt,
                        Statement::ExpressionStatement(expr_stmt)
                            if matches!(
                                &expr_stmt.expression,
                                Expression::JSXElement(_) | Expression::JSXFragment(_)
                            )
                    )
                })
            } else {
                // Block body: () => { return <div/>; }
                has_jsx_return(&arrow.body.statements)
            }
        }
        Expression::FunctionExpression(func) => func
            .body
            .as_ref()
            .is_some_and(|body| has_jsx_return(&body.statements)),
        _ => false,
    }
}

/// Check if a binding pattern has React type annotation
fn has_react_type(binding: &BindingPattern) -> bool {
    if let Some(type_annotation) = &binding.type_annotation {
        is_react_type_annotation(&type_annotation.type_annotation)
    } else {
        false
    }
}

/// Main function to check if a variable declaration is a React component
pub(crate) fn is_react_component(
    name: &str,
    binding: &BindingPattern,
    init: &Option<Expression>,
) -> bool {
    // Check 1: PascalCase naming convention (the first letter is uppercase)
    let is_pascal_case = name.chars().next().is_some_and(|c| c.is_uppercase());

    if !is_pascal_case {
        return false;
    }

    // Check 2: Has React type annotation
    if has_react_type(binding) {
        return true;
    }

    // Check 3: Contains JSX in the initialization
    if let Some(init_expr) = init {
        return contains_jsx(init_expr);
    }

    false
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
                usages.push((name.clone(), ident.span));
            }
        }
        JSXElementName::IdentifierReference(ident) => {
            let name = ident.name.to_string();
            // Only track PascalCase components (user-defined components)
            if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                usages.push((name.clone(), ident.span));
            }
        }
        _ => {}
    }
    // Collect from children
    for child in jsx_elem.children.iter() {
        if let oxc::ast::ast::JSXChild::Element(child_elem) = child {
            collect_jsx_from_element(child_elem, usages);
        }
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
                match &export_decl.declaration {
                    oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(func_decl) => {
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

/// Public function to collect all JSX usages from program body
pub(crate) fn collect_jsx_usages(statements: &[Statement]) -> Vec<(String, Span)> {
    let mut usages = Vec::new();
    for statement in statements {
        collect_jsx_from_statement(statement, &mut usages);
    }
    usages
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc::allocator::Allocator;
    use oxc::ast::ast::{BindingPatternKind, Statement};
    use oxc::parser::{ParseOptions, Parser};
    use oxc::span::SourceType;

    #[test]
    fn test_is_react_component_with_jsx_return() {
        let source = r#"
            const MyComponent = () => {
                return <div>Hello</div>;
            };
        "#;

        let allocator = Allocator::default();
        let source_type = SourceType::tsx();
        let ret = Parser::new(&allocator, source, source_type)
            .with_options(ParseOptions {
                parse_regular_expression: true,
                ..ParseOptions::default()
            })
            .parse();
        let program = ret.program;

        if let Statement::VariableDeclaration(var_decl) = &program.body[0] {
            let declarator = &var_decl.declarations[0];
            if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                let result =
                    is_react_component(&ident.name.to_string(), &declarator.id, &declarator.init);
                assert!(result, "PascalCase component with JSX should be detected");
            }
        }
    }

    #[test]
    fn test_is_react_component_camelcase_should_fail() {
        let source = r#"
            const myComponent = () => {
                return <div>Hello</div>;
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let program = ret.program;

        if let Statement::VariableDeclaration(var_decl) = &program.body[0] {
            let declarator = &var_decl.declarations[0];
            if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                let result =
                    is_react_component(&ident.name.to_string(), &declarator.id, &declarator.init);
                assert!(!result, "camelCase should not be detected as component");
            }
        }
    }

    #[test]
    fn test_is_react_component_with_type_annotation() {
        let source = r#"
            const MyComponent: FC = () => {
                return null;
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let program = ret.program;

        if let Statement::VariableDeclaration(var_decl) = &program.body[0] {
            let declarator = &var_decl.declarations[0];
            if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                let result =
                    is_react_component(&ident.name.to_string(), &declarator.id, &declarator.init);
                assert!(
                    result,
                    "Component with FC type annotation should be detected"
                );
            }
        }
    }

    #[test]
    fn test_is_react_component_no_jsx_no_type() {
        let source = r#"
            const MyFunction = () => {
                return "hello";
            };
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let program = ret.program;

        if let Statement::VariableDeclaration(var_decl) = &program.body[0] {
            let declarator = &var_decl.declarations[0];
            if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                let result =
                    is_react_component(&ident.name.to_string(), &declarator.id, &declarator.init);
                assert!(
                    !result,
                    "PascalCase without JSX or type should not be detected"
                );
            }
        }
    }

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
    fn test_arrow_function_with_jsx() {
        let source = r#"
            const MyComponent = () => <div>Hello</div>;
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let program = ret.program;

        if let Statement::VariableDeclaration(var_decl) = &program.body[0] {
            let declarator = &var_decl.declarations[0];
            if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                let result =
                    is_react_component(&ident.name.to_string(), &declarator.id, &declarator.init);
                assert!(
                    result,
                    "Arrow function with direct JSX return should be detected"
                );
            }
        }
    }
}
