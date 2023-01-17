use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};
use swc_core::{
    common::DUMMY_SP,
    ecma::{
        ast::{FnDecl, Ident, JSXAttrValue, Lit, Program},
        transforms::testing::test,
        visit::{as_folder, FoldWith, VisitMut},
    },
};

pub struct TransformVisitor;
use string_cache::Atom;
use swc_core::ecma::ast::{Callee, Expr, JSXAttr, JSXAttrName};
use swc_core::ecma::visit::VisitMutWith;

// Test
use swc_ecma_parser::{EsConfig, Syntax, TsConfig};

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
        n.visit_mut_children_with(self);

        // struct
        let Ident { sym, .. } = &mut n.ident;
        {
            if &*sym == "before" {
                let replace_name: &str = "after";
                *sym = replace_name.into();
            }
        }
    }

    // JSXの属性名・値を変更する
    fn visit_mut_jsx_attr(&mut self, n: &mut JSXAttr) {
        if let JSXAttrName::Ident(name) = &mut n.name {
            if let Some(JSXAttrValue::Lit(value)) = &mut n.value {
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
    program.fold_with(&mut as_folder(TransformVisitor))
}

test!(
    Default::default(),
    |_| as_folder(TransformVisitor),
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
    |_| as_folder(TransformVisitor),
    replace_fn_name,
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
    |_| as_folder(TransformVisitor),
    replace_jsx_attr_name_value,
    // Input codes
    r#"
    function Component() {
        return
            <div normal="value">
                <h1>hello</h1>
            </div>
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Component() {
        return
            <div special="special_value">
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
    |_| as_folder(TransformVisitor),
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
            <img src="after.png" />
    }
    "#
);

test!(
    // Syntax::Es(EsConfig {
    //     jsx: true,
    //     ..Default::default()
    // }),
    Syntax::Typescript(TsConfig {
        tsx: true,
        ..Default::default()
    }),
    |_| as_folder(TransformVisitor),
    replace_jsx_attr_value_bool,
    // Input codes
    r#"
    function Component() {
        return
            <img src="sample.png" lazy-load="false" />
    }
    "#,
    // Output codes after transformed with plugin
    r#"
    function Component() {
        return
            <img src="sample.png" lazy-load="true" />
    }
    "#
);
