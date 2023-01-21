use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use swc_core::{
    common::DUMMY_SP,
    ecma::{
        ast::{FnDecl, Ident, JSXAttrValue, Lit, Program},
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut},
    },
};

pub struct TransformVisitor {
    is_in_child: bool,
    parent_name: JSXElementName,
    component_name: Ident,
}
use string_cache::Atom;
use swc_core::ecma::ast::{
    Callee, Expr, JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXClosingElement, JSXElementName,
    JSXOpeningElement, Str,
};
use swc_core::ecma::visit::VisitMutWith;

// Test
use swc_ecma_parser::{Syntax, TsConfig};

impl TransformVisitor {
    fn new() -> Self {
        Self {
            is_in_child: false,
            parent_name: JSXElementName::Ident(Ident {
                span: DUMMY_SP,
                sym: "".into(),
                optional: false,
            }),
            component_name: Ident {
                span: DUMMY_SP,
                sym: "".into(),
                optional: false,
            },
        }
    }
}

impl VisitMut for TransformVisitor {
    // 関数呼び出し名を変更する
    fn visit_mut_callee(&mut self, callee: &mut Callee) {
        callee.visit_mut_children_with(self);

        if let Callee::Expr(expr) = callee {
            if let Expr::Ident(i) = &mut **expr {
                if &*i.sym == "onePiece" {
                    let replace_name: &str = "twoPiece";
                    i.sym = replace_name.into();
                }
            }
        }
    }

    // 関数定義名を変更する
    fn visit_mut_fn_decl(&mut self, n: &mut FnDecl) {
        // struct
        let Ident { sym, .. } = &mut n.ident;
        {
            if &*sym == "before" {
                let replace_name: &str = "after";
                *sym = replace_name.into();
            }
        }
        if !self.is_in_child {
            self.component_name = n.ident.clone();
        }
        // component_name追加後に走査
        n.visit_mut_children_with(self);
    }

    // JSXのopening_elementを取得する
    fn visit_mut_jsx_opening_element(&mut self, n: &mut JSXOpeningElement) {
        // TODO: self-closingのコンポーネントの調査・対応
        if self.is_in_child {
            return;
        }

        let element_name = &mut n.name;
        let attrs = &mut n.attrs;

        // attrsに特定の要素がなければ追加する
        let upcoming_attr_name = "data-testid";
        let mut has_attr = false;
        for attr_or_spread in attrs.iter_mut() {
            if let JSXAttrOrSpread::JSXAttr(attr) = attr_or_spread {
                if let JSXAttrName::Ident(name) = &mut attr.name {
                    if &*name.sym == upcoming_attr_name {
                        has_attr = true;
                    }
                }
            }
        }
        if !has_attr {
            // 属性を追加する
            attrs.push(JSXAttrOrSpread::JSXAttr(JSXAttr {
                span: DUMMY_SP,
                name: JSXAttrName::Ident(Ident {
                    span: DUMMY_SP,
                    sym: upcoming_attr_name.into(),
                    optional: false,
                }),
                value: Some(JSXAttrValue::Lit(Lit::Str(Str {
                    span: DUMMY_SP,
                    value: Atom::from(self.component_name.sym.clone()),
                    // TODO: Convert to kebab-case
                    raw: Some(
                        format!("\"{}\"", self.component_name.sym.clone().to_lowercase()).into(),
                    ),
                }))),
            }));
        }

        for attr_or_spread in attrs.iter_mut() {
            if let JSXAttrOrSpread::JSXAttr(attr) = attr_or_spread {
                // visit_mut_jsx_attr(JSXの属性名・値を変更する)と同じ
                if let JSXAttrName::Ident(name) = &mut attr.name {
                    if let Some(JSXAttrValue::Lit(value)) = &mut attr.value {
                        if let Lit::Str(s) = value {
                            if &*name.sym == "src" {
                                if &*s.value == "before.png" {
                                    s.span = DUMMY_SP;
                                    s.value = Atom::from("after.png");
                                    s.raw = Some("\"after.png\"".into());
                                }
                            }

                            if &*name.sym == "normal" {
                                let replace_name: &str = "special";
                                name.sym = replace_name.into();

                                s.span = DUMMY_SP;
                                s.value = Atom::from("special_value");
                                s.raw = Some("\"special_value\"".into());
                            }

                            if &*name.sym == "lazy-load" {
                                if &*s.value == "false" {
                                    s.span = DUMMY_SP;
                                    s.value = Atom::from("true");
                                    s.raw = Some("\"true\"".into());
                                }
                            }
                        }
                    }
                    // ===JSXAttr===
                    // JSXExprContainer
                    // JSXElement
                    // JSXFragment
                }
            }
        }

        // 親だけ解析する
        // WARNING: self closing 未対応
        self.is_in_child = true;
        self.parent_name = element_name.clone();
    }

    // JSXのclosing_elementを取得する
    fn visit_mut_jsx_closing_element(&mut self, n: &mut JSXClosingElement) {
        let element_name = &mut n.name;

        // if let JSXElementName::Ident(ident) = element_name {
        //     if &*ident.sym == "h1" {
        //         // h2に変更する
        //         ident.sym = "h2".into();
        //     }
        // }

        // 親のclosingを見つける
        if *element_name == self.parent_name {
            self.is_in_child = false;
        }
    }

    // fn visit_mut_jsx_element_children(&mut self, n: &mut Vec<JSXElementChild>) {
    //     // コンポーネントに子がいる場合
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
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor::new()))
}

test!(
    Default::default(),
    |_| as_folder(TransformVisitor::new()),
    replace_fetch,
    // Input codes
    r#"
    const res = await onePiece('http://localhost:9999');
    "#,
    // Output codes after transformed with plugin
    r#"
    const res = await twoPiece('http://localhost:9999');
    "#
);

test!(
    Default::default(),
    |_| as_folder(TransformVisitor::new()),
    replace_function_name,
    // Input codes
    r#"
    function before(number) {
        return number * number;
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function after(number) {
        return number * number;
    }
    "#
);

// https://github.com/swc-project/swc/blob/main/crates/swc/tests/simple.rs
test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    replace_jsx_attr_name_and_value,
    // Input codes
    r#"
    function TextComponent() {
        return
            <div normal="value">
                <h1>hello</h1>
            </div>
    }
    "#,
    // Output codes after transformed with plugin
    // まだ子要素にも属性が追加される
    r#"
    function TextComponent() {
        return
            <div special="special_value" data-testid="textcomponent">
                <h1>hello</h1>
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
    replace_jsx_attr_value,
    // Input codes
    r#"
    function Component() {
        return
            <img src="before.png" />
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Component() {
        return
            <img src="after.png" data-testid="component" />
    }
    "#
);

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
            <img src="sample.png" lazy-load="true" data-testid="imgcomponent" />
    }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    replace_jsx_element_name,
    // Input codes
    r#"
    function Text() {
        return
            <h1>
                This is Text Element!
            </h1>
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Text() {
        return
            <h1 data-testid="text">
                This is Text Element!
            </h1>
    }
    "#
);

test!(
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor::new()),
    not_insert_jsx_attr,
    // Input codes
    r#"
    function Text() {
        return
            <h3 data-testid="already-data-testid">
                This is Text Element!
            </h3>
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Text() {
        return
            <h3 data-testid="already-data-testid">
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
    add_jsx_attr_only_parent,
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
