use serde::Deserialize;
use serde_json::Value;
use swc_core::{
    common::{FileName, DUMMY_SP},
    ecma::{
        ast::{FnDecl, Id, Ident, JSXAttrValue, Lit, Pat, Program, Stmt},
        atoms::JsWordStaticSet,
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
    plugin::{
        metadata::TransformPluginMetadataContextKind, plugin_transform,
        proxies::TransformPluginProgramMetadata,
    },
};

pub struct TransformVisitor {
    attr_name: String,
    ignore_components: Vec<String>,
    filename: FileName,
    is_in_child: bool,
    parent_id: Id,
    component_name: Ident,
}
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub attr_name: String,
    pub ignore_components: Vec<String>,
}

use convert_case::{Case, Casing};
use string_cache::Atom;
use swc_core::ecma::ast::{
    BlockStmt, BlockStmtOrExpr, Expr, JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXClosingElement,
    JSXElementName, JSXOpeningElement, Str, VarDecl,
};

/**
 * Convert to kebab-case from UpperCamelCase(component name)
 */
fn convert_to_kebab_case(s: Atom<JsWordStaticSet>) -> String {
    return s.clone().to_string().to_case(Case::Kebab);
}

/**
 * Whether vec contains item
 * return true if one element of vec is same item(String Compare)
 */
fn vec_contains_string(vec: Vec<String>, item: String) -> bool {
    let mut is_in_vec = false;
    for content in vec.clone().iter_mut() {
        let trimed_content = content.trim_matches('\"');
        if trimed_content == item {
            is_in_vec = true;
        }
    }

    is_in_vec
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
            ignore_components: [].to_vec(),
            filename: FileName::Anon,
            is_in_child: false,
            parent_id: Id::default(),
            component_name: Ident {
                span: DUMMY_SP,
                sym: "".into(),
                optional: false,
            },
        }
    }

    fn set_config(&mut self, config: &Config, filename: FileName) {
        self.attr_name = config.attr_name.clone();
        self.ignore_components = config.ignore_components.clone();
        self.filename = filename;
    }
}

impl VisitMut for TransformVisitor {
    // TODO: CHECK ignoreComponents
    fn visit_mut_fn_decl(&mut self, n: &mut FnDecl) {
        if !self.is_in_child {
            self.component_name = n.ident.clone();
        }
        // check after updating self.component_name
        n.visit_mut_children_with(self);
    }

    // This function is to get component_name and check variable whether jsx component or not
    fn visit_mut_var_decl(&mut self, n: &mut VarDecl) {
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

        //  Check
        //  1. this element has specific attribute
        //  2. this element has component_name(is not child element)
        //  3. this element is not one of ignore components
        if !has_attr
            && &*self.component_name.sym != ""
            && !vec_contains_string(
                self.ignore_components.clone(),
                self.component_name.sym.to_string(),
            )
        {
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
                    value: Atom::from(
                        convert_to_kebab_case(self.component_name.sym.clone()).to_lowercase(),
                    ),
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
}

#[plugin_transform]
pub fn process_transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    let filename = match metadata.get_context(&TransformPluginMetadataContextKind::Filename) {
        Some(s) => FileName::Real(s.into()),
        None => FileName::Anon,
    };
    let plugin_config: Value = serde_json::from_str(
        &metadata
            .get_transform_plugin_config()
            .expect("failed to get plugin config for this swc plugin"),
    )
    .expect("Should provide config for this swc plugin");
    let attr_name = plugin_config["attrName"]
        .as_str()
        .expect("attr_name is expected")
        .to_string();

    let ignore_files = plugin_config["ignoreFiles"]
        .as_array()
        .expect("ignoreFiles is expected")
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>();

    let ignore_components = plugin_config["ignoreComponents"]
        .as_array()
        .expect("ignoreComponent is expected")
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>();

    let config = Config {
        attr_name,
        ignore_components,
    };

    let mut is_ignore = false;
    for ignore_file in ignore_files.clone().iter_mut() {
        let igf = ignore_file.trim_matches('\"');
        if filename.to_string().contains(igf) {
            is_ignore = true;
        }
    }

    let mut visitor = TransformVisitor::new();
    visitor.set_config(&config, filename);
    if is_ignore {
        program
    } else {
        program.fold_with(&mut as_folder(visitor))
    }
}

fn make_test_visitor() -> TransformVisitor {
    let mut visitor = TransformVisitor::new();
    let config = Config {
        attr_name: "data-testid".to_string(),
        ignore_components: [].to_vec(),
    };
    visitor.set_config(&config, FileName::Anon);
    return visitor;
}

// https://github.com/swc-project/swc/blob/main/crates/swc/tests/simple.rs
test!(
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
    data_testid_like_practice_with_parenthesis,
    // Input codes
    r#"
    import { UserProfile } from './user';
    import { User } from './types/user';

    const onClickFn = () => {
        console.log('Hello, World!');
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
        console.log('Hello, World!');
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
    data_testid_like_practice_without_parenthesis,
    // Input codes
    r#"
    import { UserProfile } from './user';
    import { User } from './types/user';

    const onClickFn = () => {
        console.log('Hello, World!');
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
        console.log('Hello, World!');
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
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
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
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

test!(
    swc_core::ecma::parser::Syntax::Typescript(swc_core::ecma::parser::TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(make_test_visitor()),
    exact_code_no_change,
    // Input codes
    r#"
export const SettingProfilePage: FC = () => {

  if (!user) return <LoadingPage />

  return (
    <>
      <UserNavbar />
      <div className="max-w-md mx-4 sm:mx-auto">
        <h1 className="mt-8 text-2xl font-bold text-gray-800">Setting</h1>
        <SettingsTab />
        <div className="my-8">
          <h3 className="inline-flex items-center text-lg font-semibold text-gray-700">
            Setting Profile
          </h3>
          <p className="mt-1 mb-4 text-sm text-gray-500">
            Use your data
          </p>
          <TextField
            name="parent"
            label="parent name"
            required
            control={control}
          />
          <TextField
            name="email"
            label="email"
            required
            control={control}
          />
          <Border className="my-8" />

          <div>
            <button
              className={clsx(
                'relative flex justify-center w-full px-4 py-2 text-sm font-medium text-white transition duration-150 bg-blue-600 border border-transparent rounded-md group hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500',
                { 'cursor-not-allowed': isLoading }
              )}
              disabled={isLoading}
              onClick={handleSubmit(onSubmit)}
            >
              Button
            </button>
          </div>
        </div>
      </div>
    </>
  )
}
    "#,
    // Output codes after transformed with plugin
    r#"
export const SettingProfilePage: FC = () => {

  if (!user) return <LoadingPage />

  return <>
      <UserNavbar />
      <div className="max-w-md mx-4 sm:mx-auto">
        <h1 className="mt-8 text-2xl font-bold text-gray-800">Setting</h1>
        <SettingsTab />
        <div className="my-8">
          <h3 className="inline-flex items-center text-lg font-semibold text-gray-700">
            Setting Profile
          </h3>
          <p className="mt-1 mb-4 text-sm text-gray-500">
            Use your data
          </p>
          <TextField
            name="parent"
            label="parent name"
            required
            control={control}
          />
          <TextField
            name="email"
            label="email"
            required
            control={control}
          />
          <Border className="my-8" />

          <div>
            <button
              className={clsx(
                'relative flex justify-center w-full px-4 py-2 text-sm font-medium text-white transition duration-150 bg-blue-600 border border-transparent rounded-md group hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500',
                { 'cursor-not-allowed': isLoading }
              )}
              disabled={isLoading}
              onClick={handleSubmit(onSubmit)}
            >
              Button
            </button>
          </div>
        </div>
      </div>
    </>
}

    "#
);
