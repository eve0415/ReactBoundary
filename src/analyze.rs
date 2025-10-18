use oxc::ast::ast::TSTypeName::IdentifierReference;
use oxc::ast::ast::{BindingPattern, Expression, Statement, TSType};

/// Check if a type annotation is a React component type
pub(crate) fn is_react_type_annotation(ts_type: &TSType) -> bool {
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
pub(crate) fn contains_jsx(expr: &Expression) -> bool {
    match expr {
        Expression::JSXElement(_) | Expression::JSXFragment(_) => true,
        Expression::ArrowFunctionExpression(arrow) => has_jsx_return(&arrow.body.statements),
        Expression::FunctionExpression(func) => func
            .body
            .as_ref()
            .is_some_and(|body| has_jsx_return(&body.statements)),
        _ => false,
    }
}

/// Check if a binding pattern has React type annotation
pub(crate) fn has_react_type(binding: &BindingPattern) -> bool {
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
