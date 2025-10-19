use oxc::ast::ast::TSTypeName::IdentifierReference;
use oxc::ast::ast::{BindingPattern, Expression, Statement, TSType};

// ============================================================================
// PUBLIC API
// ============================================================================

/// Main function to check if a variable declaration is a React component
pub(crate) fn is_react_component(
    name: &str,
    binding: &BindingPattern,
    init: &Option<Expression>,
    jsx_runtime_identifiers: &std::collections::HashSet<String>,
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
        return contains_jsx(init_expr, jsx_runtime_identifiers);
    }

    false
}

/// Check if a function declaration is a React component
pub(crate) fn is_react_function_component(
    name: &str,
    return_type: &Option<oxc::allocator::Box<oxc::ast::ast::TSTypeAnnotation>>,
    body: &Option<oxc::allocator::Box<oxc::ast::ast::FunctionBody>>,
    jsx_runtime_identifiers: &std::collections::HashSet<String>,
) -> bool {
    // Check 1: PascalCase naming convention
    let is_pascal_case = name.chars().next().is_some_and(|c| c.is_uppercase());

    if !is_pascal_case {
        return false;
    }

    // Check 2: Has React return type annotation
    if let Some(type_annotation) = return_type
        && is_react_type_annotation(&type_annotation.type_annotation)
    {
        return true;
    }

    // Check 3: Contains JSX return in the function body
    if let Some(func_body) = body {
        return has_jsx_return(&func_body.statements, jsx_runtime_identifiers);
    }

    false
}

// ============================================================================
// Helper Functions: Type Checking
// ============================================================================

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

/// Check if a binding pattern has React type annotation
fn has_react_type(binding: &BindingPattern) -> bool {
    if let Some(type_annotation) = &binding.type_annotation {
        is_react_type_annotation(&type_annotation.type_annotation)
    } else {
        false
    }
}

// ============================================================================
// Helper Functions: JSX Detection
// ============================================================================

/// Check if an expression contains JSX (or jsx runtime calls for bundled code)
fn contains_jsx(
    expr: &Expression,
    jsx_runtime_identifiers: &std::collections::HashSet<String>,
) -> bool {
    match expr {
        Expression::JSXElement(_) | Expression::JSXFragment(_) => true,
        // Check for jsx/jsxs runtime calls (bundled code)
        expr if is_jsx_runtime_call(expr, jsx_runtime_identifiers) => true,
        // Check for React.forwardRef(() => jsx(...)) or forwardRef(() => jsx(...))
        Expression::CallExpression(call_expr) if is_react_hoc(call_expr) => {
            // Check the first argument (the component function)
            if let Some(first_arg) = call_expr.arguments.first()
                && let Some(arg_expr) = first_arg.as_expression()
            {
                return contains_jsx(arg_expr, jsx_runtime_identifiers);
            }
            false
        }
        Expression::ArrowFunctionExpression(arrow) => {
            // For arrow functions, check if it's an expression body (implicit return)
            // If expression is true, the body contains a single expression
            // If expression is false, it has a block body with statements
            if arrow.expression {
                // Implicit return: () => <div/> or () => jsx("div", {})
                // The body will have a single ExpressionStatement
                arrow.body.statements.iter().any(|stmt| {
                    matches!(
                        stmt,
                        Statement::ExpressionStatement(expr_stmt)
                            if matches!(
                                &expr_stmt.expression,
                                Expression::JSXElement(_) | Expression::JSXFragment(_)
                            ) || is_jsx_runtime_call(&expr_stmt.expression, jsx_runtime_identifiers)
                    )
                })
            } else {
                // Block body: () => { return <div/>; } or () => { return jsx("div", {}); }
                has_jsx_return(&arrow.body.statements, jsx_runtime_identifiers)
            }
        }
        Expression::FunctionExpression(func) => func
            .body
            .as_ref()
            .is_some_and(|body| has_jsx_return(&body.statements, jsx_runtime_identifiers)),
        _ => false,
    }
}

/// Helper function to check if statements contain JSX return (or jsx runtime calls)
fn has_jsx_return(
    statements: &[Statement],
    jsx_runtime_identifiers: &std::collections::HashSet<String>,
) -> bool {
    statements.iter().any(|stmt| {
        if let Statement::ReturnStatement(ret) = stmt {
            if let Some(arg) = &ret.argument {
                matches!(arg, Expression::JSXElement(_) | Expression::JSXFragment(_))
                    || is_jsx_runtime_call(arg, jsx_runtime_identifiers)
            } else {
                false
            }
        } else {
            false
        }
    })
}

/// Check if an expression is a jsx/jsxs runtime call (for bundled code)
/// Bundled code uses jsx("div", {...}) instead of <div>
///
/// This properly handles renamed imports like: import { jsx as foobar } from "react/jsx-runtime"
/// by checking if the called identifier is in the jsx_runtime_identifiers set.
fn is_jsx_runtime_call(
    expr: &Expression,
    jsx_runtime_identifiers: &std::collections::HashSet<String>,
) -> bool {
    if let Expression::CallExpression(call) = expr {
        // Handle direct calls: jsx(...), foobar(...) where foobar is imported from react/jsx-runtime
        if let Expression::Identifier(callee) = &call.callee {
            let name = callee.name.as_str();
            return jsx_runtime_identifiers.contains(name);
        }

        // Unwrap ParenthesizedExpression to get to the actual expression
        // Pattern: ((0, jsx))(...) or (0, jsx)(...)
        let actual_callee = if let Expression::ParenthesizedExpression(paren) = &call.callee {
            &paren.expression
        } else {
            &call.callee
        };

        // Handle compiled pattern: (0, jsx)(...) or (0, import_jsx_runtime.jsx)(...)
        // This is a SequenceExpression where the last expression is either:
        // - An Identifier (e.g., jsx, foobar)
        // - A MemberExpression (e.g., import_jsx_runtime.jsx)
        if let Expression::SequenceExpression(seq) = actual_callee
            && let Some(last_expr) = seq.expressions.last()
        {
            // Case 1: Direct identifier - (0, jsx) or (0, foobar)
            if let Expression::Identifier(ident) = last_expr {
                let name = ident.name.as_str();
                return jsx_runtime_identifiers.contains(name);
            }
            // Case 2: StaticMemberExpression - (0, import_jsx_runtime.jsx)
            // Check if the property name is a jsx runtime function
            if let Expression::StaticMemberExpression(member) = last_expr {
                let prop_name = member.property.name.as_str();
                // For member expressions, we check standard jsx runtime names
                // since the member access happens on the imported module object
                return matches!(prop_name, "jsx" | "jsxs" | "jsxDEV" | "Fragment");
            }
            // Case 3: ComputedMemberExpression (rare, but handle it)
            if let Expression::ComputedMemberExpression(member) = last_expr
                && let Expression::StringLiteral(lit) = &member.expression
            {
                return matches!(lit.value.as_str(), "jsx" | "jsxs" | "jsxDEV" | "Fragment");
            }
        }
    }
    false
}

/// Check if a CallExpression is React.forwardRef or similar HOC patterns
fn is_react_hoc(call_expr: &oxc::ast::ast::CallExpression) -> bool {
    use oxc::ast::ast::Expression;

    // Check for React.forwardRef, React.memo, etc.
    if let Expression::StaticMemberExpression(member) = &call_expr.callee
        && let Expression::Identifier(obj) = &member.object
        && obj.name == "React"
        && matches!(member.property.name.as_str(), "forwardRef" | "memo")
    {
        return true;
    }

    // Check for forwardRef, memo (direct imports)
    if let Expression::Identifier(callee) = &call_expr.callee {
        return matches!(callee.name.as_str(), "forwardRef" | "memo");
    }

    false
}

// ============================================================================
// Tests
// ============================================================================

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

        let jsx_runtime_identifiers = std::collections::HashSet::new();

        if let Statement::VariableDeclaration(var_decl) = &program.body[0] {
            let declarator = &var_decl.declarations[0];
            if let BindingPatternKind::BindingIdentifier(ident) = &declarator.id.kind {
                let result = is_react_component(
                    ident.name.as_ref(),
                    &declarator.id,
                    &declarator.init,
                    &jsx_runtime_identifiers,
                );
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
                let result = is_react_component(
                    ident.name.as_ref(),
                    &declarator.id,
                    &declarator.init,
                    &std::collections::HashSet::new(),
                );
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
                let result = is_react_component(
                    ident.name.as_ref(),
                    &declarator.id,
                    &declarator.init,
                    &std::collections::HashSet::new(),
                );
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
                let result = is_react_component(
                    ident.name.as_ref(),
                    &declarator.id,
                    &declarator.init,
                    &std::collections::HashSet::new(),
                );
                assert!(
                    !result,
                    "PascalCase without JSX or type should not be detected"
                );
            }
        }
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
                let result = is_react_component(
                    ident.name.as_ref(),
                    &declarator.id,
                    &declarator.init,
                    &std::collections::HashSet::new(),
                );
                assert!(
                    result,
                    "Arrow function with direct JSX return should be detected"
                );
            }
        }
    }

    #[test]
    fn test_function_declaration_with_jsx() {
        let source = r#"
            function MyComponent() {
                return <div>Hello</div>;
            }
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let program = ret.program;

        let jsx_runtime_identifiers = std::collections::HashSet::new();

        if let Statement::FunctionDeclaration(func_decl) = &program.body[0]
            && let Some(id) = &func_decl.id
        {
            let result = is_react_function_component(
                id.name.as_ref(),
                &func_decl.return_type,
                &func_decl.body,
                &jsx_runtime_identifiers,
            );
            assert!(
                result,
                "Function declaration with JSX return should be detected"
            );
        }
    }

    #[test]
    fn test_function_declaration_camelcase_should_fail() {
        let source = r#"
            function myFunction() {
                return <div>Hello</div>;
            }
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let program = ret.program;

        let jsx_runtime_identifiers = std::collections::HashSet::new();

        if let Statement::FunctionDeclaration(func_decl) = &program.body[0]
            && let Some(id) = &func_decl.id
        {
            let result = is_react_function_component(
                id.name.as_ref(),
                &func_decl.return_type,
                &func_decl.body,
                &jsx_runtime_identifiers,
            );
            assert!(
                !result,
                "camelCase function should not be detected as component"
            );
        }
    }

    #[test]
    fn test_function_declaration_no_jsx_should_fail() {
        let source = r#"
            function MyFunction() {
                return "hello";
            }
        "#;

        let allocator = Allocator::default();
        let ret = Parser::new(&allocator, source, SourceType::tsx()).parse();
        let program = ret.program;

        let jsx_runtime_identifiers = std::collections::HashSet::new();

        if let Statement::FunctionDeclaration(func_decl) = &program.body[0]
            && let Some(id) = &func_decl.id
        {
            let result = is_react_function_component(
                id.name.as_ref(),
                &func_decl.return_type,
                &func_decl.body,
                &jsx_runtime_identifiers,
            );
            assert!(
                !result,
                "Function without JSX should not be detected as component"
            );
        }
    }
}
