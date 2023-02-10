use serde::Deserialize;
use swc_core::{
    common::DUMMY_SP,
    ecma::{
        ast::{FnDecl, Id, Ident, JSXAttrValue, Lit, Pat, Program, Stmt},
        atoms::JsWordStaticSet,
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
    plugin::{plugin_transform, proxies::TransformPluginProgramMetadata},
};

pub struct TransformVisitor {
    attr_name: String,
    is_in_child: bool,
    parent_id: Id,
    component_name: Ident,
}
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub attr_name: String,
}

use convert_case::{Case, Casing};
use string_cache::Atom;
use swc_core::ecma::ast::{
    BlockStmt, BlockStmtOrExpr, Expr, JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXClosingElement,
    JSXElementName, JSXOpeningElement, Str, VarDecl,
};

// Crates for Test
use swc_ecma_parser::{Syntax, TsConfig};

fn convert_to_kebab_case(s: Atom<JsWordStaticSet>) -> String {
    return s.clone().to_string().to_case(Case::Kebab);
}

/**
* Check if the expression is Parenthesis Element
* which returns JSXElement like the following example.
*
* (
    <div>
        <Component />
    </div>
  )
*/
fn parse_expr_stmt(expr_stmt: &mut Box<Expr>) -> bool {
    let mut is_jsx_component = false;

    match &mut **expr_stmt {
        // TODO: support for JSX***
        // https://docs.rs/swc_ecma_ast/0.80.0/swc_ecma_ast/enum.Expr.html
        Expr::Paren(paren_expr) => {
            let expr = &mut paren_expr.expr;
            // TODO: support for JSX***
            match &mut **expr {
                Expr::JSXElement(_) => is_jsx_component = true,
                _ => (),
            }
        }
        _ => (),
    }

    return is_jsx_component;
}

/**
 * Check if the block statement is like following examples.
 *
 * <<Pattern 1 (Self Closing)>>
 * return <Component />
 *
 * <<Pattern 2 (Return JSXElement with Parenthesis)>>
 * return (
 *   <div>
 *     <h1>Text</h1>
 *   </div>
 * )
 *
 * <<Pattern 3 (Return JSXElement without Parenthesis)>>
 * return <div><h1>Text</h1></div>
 *
 */
fn parse_block_stmt(block_stmt: &mut BlockStmt) -> bool {
    let mut is_jsx_component = false;

    let stmts = &mut block_stmt.stmts;
    for stmt in stmts.iter_mut() {
        // check type of arg
        if let Stmt::Return(return_stmt) = stmt {
            if let Some(arg) = &mut return_stmt.arg {
                match &mut **arg {
                    // TODO: support for JSX***
                    // <<Pattern 1 (Self Closing)>>
                    Expr::JSXElement(_) => is_jsx_component = true,
                    // <<Pattern 2 (Return JSXElement with Parenthesis)>>
                    Expr::Paren(paren_expr) => {
                        let expr = &mut paren_expr.expr;
                        // TODO: support for JSX***
                        match &mut **expr {
                            Expr::JSXElement(_) => is_jsx_component = true,
                            _ => (),
                        }
                    }
                    _ => (),
                }
            }
        }

        // <<Pattern 3 (Return JSXElement without Parenthesis)>>
        if let Stmt::Expr(expr_stmt) = stmt {
            let expr = &mut expr_stmt.expr;
            match &mut **expr {
                Expr::JSXElement(_) => is_jsx_component = true,
                _ => (),
            }
        }
    }

    return is_jsx_component;
}

impl TransformVisitor {
    fn new() -> Self {
        Self {
            attr_name: "".to_string(),
            is_in_child: false,
            parent_id: Id::default(),
            component_name: Ident {
                span: DUMMY_SP,
                sym: "".into(),
                optional: false,
            },
        }
    }

    fn set_config(&mut self, attr_name: String) {
        self.attr_name = attr_name;
    }
}

impl VisitMut for TransformVisitor {
    fn visit_mut_fn_decl(&mut self, n: &mut FnDecl) {
        // panic!("=====visit_mut_fn_decl=====");
        if !self.is_in_child {
            self.component_name = n.ident.clone();
        }
        // check after updating self.component_name
        n.visit_mut_children_with(self);
    }

    // This function is to get component_name and check variable whether jsx component or not
    fn visit_mut_var_decl(&mut self, n: &mut VarDecl) {
        // panic!("=====visit_mut_var_decl=====");
        let decls = &mut n.decls;
        let mut is_jsx_component = false;

        for decl in decls.iter_mut() {
            if let Some(init) = &mut decl.init {
                // https://swc.rs/docs/plugin/ecmascript/cheatsheet#matching-boxt
                if let Expr::Arrow(arrow_expr) = &mut **init {
                    if let BlockStmtOrExpr::BlockStmt(block_stmt) = &mut arrow_expr.body {
                        // Same as Functions Expression
                        let tmp_is_jsx_component = parse_block_stmt(block_stmt);
                        is_jsx_component = tmp_is_jsx_component;
                    }
                    if let BlockStmtOrExpr::Expr(expr_stmt) = &mut arrow_expr.body {
                        let tmp_is_jsx_component = parse_expr_stmt(expr_stmt);
                        is_jsx_component = tmp_is_jsx_component;
                    }
                }

                // return fn expr which returns JSXElement
                if let Expr::Fn(fn_expr) = &mut **init {
                    if let Some(block_stmt) = &mut fn_expr.function.body {
                        // Same as Arrow Functions
                        let tmp_is_jsx_component = parse_block_stmt(block_stmt);
                        is_jsx_component = tmp_is_jsx_component;
                    }
                }
            }
        }

        if !self.is_in_child && is_jsx_component {
            let first_decl = &mut decls[0];
            if let Pat::Ident(ident) = &first_decl.name {
                // get the function name
                self.component_name = ident.id.clone();
            }
        }

        // check after update self.component_name
        n.visit_mut_children_with(self);
    }

    // visit jsx opening_element
    fn visit_mut_jsx_opening_element(&mut self, n: &mut JSXOpeningElement) {
        // CHECK: support for sef-closing component
        if self.is_in_child {
            return;
        }

        let element_name = &n.name;
        let attrs = &mut n.attrs;
        let is_self_closing = n.self_closing;

        // add "data-testid"(by default) if there is no "data-testid"(by default) attribute.
        let attr_name = self.attr_name.clone();
        let mut has_attr = false;
        for attr_or_spread in attrs.iter_mut() {
            if let JSXAttrOrSpread::JSXAttr(attr) = attr_or_spread {
                if let JSXAttrName::Ident(name) = &mut attr.name {
                    if &*name.sym == attr_name {
                        has_attr = true;
                    }
                }
            }
        }
        if !has_attr {
            // add attribute
            attrs.push(JSXAttrOrSpread::JSXAttr(JSXAttr {
                span: DUMMY_SP,
                name: JSXAttrName::Ident(Ident {
                    span: DUMMY_SP,
                    sym: attr_name.into(),
                    optional: false,
                }),
                value: Some(JSXAttrValue::Lit(Lit::Str(Str {
                    span: DUMMY_SP,
                    value: Atom::from(self.component_name.sym.clone()),
                    raw: Some(
                        format!(
                            "\"{}\"",
                            convert_to_kebab_case(self.component_name.sym.clone()).to_lowercase()
                        )
                        .into(),
                    ),
                }))),
            }));
        }

        for attr_or_spread in attrs.iter_mut() {
            if let JSXAttrOrSpread::JSXAttr(attr) = attr_or_spread {
                // almost same as visit_mut_jsx_attr(update name or value of jsx attribute) function
                if let JSXAttrName::Ident(name) = &mut attr.name {
                    if let Some(JSXAttrValue::Lit(value)) = &mut attr.value {
                        if let Lit::Str(s) = value {
                            if &*name.sym == "lazy-load" {
                                if &*s.value == "false" {
                                    s.span = DUMMY_SP;
                                    s.value = Atom::from("true");
                                    s.raw = Some("\"true\"".into());
                                }
                            }
                        }
                    }
                }
            }
        }

        // check top level of component
        // CHECK: support for sef-closing component
        if !is_self_closing {
            self.is_in_child = true;
        }

        if let JSXElementName::Ident(ident) = &element_name {
            self.parent_id = ident.to_id();
        } else {
            panic!("parent_name is not type Ident");
        }
    }

    // visit jsx closing_element
    fn visit_mut_jsx_closing_element(&mut self, n: &mut JSXClosingElement) {
        let element_name = &mut n.name;
        // find parent closing_element
        if let JSXElementName::Ident(ident) = &*element_name {
            if ident.to_id() == self.parent_id {
                self.is_in_child = false;
            }
        }
    }

    // fn visit_mut_jsx_element_children(&mut self, n: &mut Vec<JSXElementChild>) {
    //     // in case component has children
    //     if n.len() > 0 {
    //         panic!("===visit_mut_jsx_element_children==={:?}", n);
    //     }
    // }

    // Implement necessary visit_mut_* methods for actual custom transform.
    // A comprehensive list of possible visitor methods can be found here:
    // https://rustdoc.swc.rs/swc_ecma_visit/trait.VisitMut.html
}

/// An example plugin function with macro support.
/// `plugin_transform` macro interop pointers into deserialized structs, as well
/// as returning ptr back to host.
///
/// It is possible to opt out from macro by writing transform fn manually
/// if plugin need to handle low-level ptr directly via
/// `__transform_plugin_process_impl(
///     ast_ptr: *const u8, ast_ptr_len: i32,
///     unresolved_mark: u32, should_enable_comments_proxy: i32) ->
///     i32 /*  0 for success, fail otherwise.
///             Note this is only for internal pointer interop result,
///             not actual transform result */`
///
/// This requires manual handling of serialization / deserialization from ptrs.
/// Refer swc_plugin_macro to see how does it work internally.
#[plugin_transform]
pub fn process_transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    let mut visitor = TransformVisitor::new();
    let config = serde_json::from_str::<Config>(
        &metadata
            .get_transform_plugin_config()
            .expect("failed to get plugin config for this swc plugin"),
    )
    .expect("invalid config for this swc plugin");
    visitor.set_config(config.attr_name);
    program.fold_with(&mut as_folder(visitor))
}

// https://github.com/swc-project/swc/blob/main/crates/swc/tests/simple.rs
test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    replace_jsx_attr_bool,
    // Input codes
    r#"
    function ImgComponent() {
        return
            <img src="sample.png" lazy-load="false" />
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function ImgComponent() {
        return
            <img src="sample.png" lazy-load="true" data-testid="img-component" />
    }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_already_has_attr,
    // Input codes
    r#"
    function Text() {
        return
            <h3 data-testid="current-data-testid">
                This is Text Element!
            </h3>
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Text() {
        return
            <h3 data-testid="current-data-testid">
                This is Text Element!
            </h3>
    }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_has_children,
    // Input codes
    r#"
    function Component() {
        return
            <div>
                <div>
                    <h3>This is nested text!</h3>
                </div>
            </div>
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Component() {
        return
            <div data-testid="component">
                <div>
                    <h3>This is nested text!</h3>
                </div>
            </div>
    }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_like_practice_with_parenthesis,
    // Input codes
    r#"
    import { UserProfile } from './user';
    import { User } from './types/user';

    const onClickFn = () => {
        console.log('Button is clicked!!');
        return "hello";
    }

    type Props = {
        user: User
    }

    const UserComponent = ({ user }: Props) => {
        return (
            <User user={user}>
                <div>
                    <button onClick={onClickFn}>This is button!</button>
                    <h3>This is nested text!</h3>
                </div>
            </User>
        )
            
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    import { UserProfile } from './user';
    import { User } from './types/user';

    const onClickFn = () => {
        console.log('Button is clicked!!');
        return "hello";
    }

    type Props = {
        user: User
    }

    const UserComponent = ({ user }: Props) => {
        return  <User user={user} data-testid="user-component">
                <div>
                    <button onClick={onClickFn}>This is button!</button>
                    <h3>This is nested text!</h3>
                </div>
            </User>
    }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_like_practice_without_parenthesis,
    // Input codes
    r#"
    import { UserProfile } from './user';
    import { User } from './types/user';

    const onClickFn = () => {
        console.log('Button is clicked!!');
        return "hello";
    }

    type Props = {
        user: User
    }

    const UserComponent = ({ user }: Props) => {
        return <User user={user} />
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    import { UserProfile } from './user';
    import { User } from './types/user';

    const onClickFn = () => {
        console.log('Button is clicked!!');
        return "hello";
    }

    type Props = {
        user: User
    }

    const UserComponent = ({ user }: Props) => {
        return <User user={user} data-testid="user-component" />
    }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_function_declaration,
    // Input codes
    r#"
    function Div() {
      return <div />
    }

    function Nested() {
      return (
        <div>
          hello
          <div>world</div>
        </div>
      )
    }

    function NoReturn () { <div /> }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Div() {
      return <div data-testid="div" />
    }
    
    function Nested() {
      return <div data-testid="nested">
          hello
          <div>world</div>
        </div>
    }

    // This might be strange but OK in this case.
    function NoReturn () { <div data-testid="no-return" /> }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_function_expression,
    // Input codes
    r#"
    const Div = function() {
      return <div />
    }

    const Nested = function() {
      return (
        <div>
          hello
          <div>world</div>
        </div>
      )
    }

    const NoReturn = function() { <div /> }

    const NoJSXReturn = function() { return 0 }
    "#,
    // Output codes after transformed with plugin
    r#"
    const Div = function() {
      return <div data-testid="div" />
    }

    const Nested = function() {
      return <div data-testid="nested">
          hello
          <div>world</div>
        </div>
    }

    // This might be strange but OK in this case.
    const NoReturn = function() { <div data-testid="no-return" /> }

    const NoJSXReturn = function() { return 0 }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_arrow_function_expression,
    // Input codes
    r#"
    const Div = () => {
      return <div />
    }

    const Nested = () => {
      return (
        <div>
          hello
          <div>world</div>
        </div>
      )
    }

    const WithoutReturn = () => (
      <div>hello</div>
    )

    const WithoutReturnJSXFragment = () => (
      <>
        <div />
      </>
    )

    const NoReturn = () => { <div /> }
    "#,
    // Output codes after transformed with plugin
    r#"
    const Div = () => {
      return <div data-testid="div" />
    }

    const Nested = () => {
      return <div data-testid="nested">
          hello
          <div>world</div>
        </div>
    }

    const WithoutReturn = () => <div data-testid="without-return">hello</div>;

    // This might be strange but OK in this case.
    const WithoutReturnJSXFragment = () => <>
        <div data-testid="without-return" />
      </>;

    // This might be strange but OK in this case.
    const NoReturn = () => { <div data-testid="no-return" /> }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    data_testid_arrow_function_expression_with_children,
    // Input codes
    r#"
    const Parent = ({ children }) => (
      <div>{children}</div>
    )

    const Child = () => (
      <Parent>
        <div>
          child
        </div>
      </Parent>
    )
    "#,
    // Output codes after transformed with plugin
    r#"
    const Parent = ({ children }) => <div data-testid="parent">{children}</div>;

    const Child = () =>
      <Parent data-testid="child">
        <div>
          child
        </div>
      </Parent>
    "#
);
