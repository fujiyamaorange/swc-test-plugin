use swc_core::ecma::{
    ast::{FnDecl, Ident, JSXAttrName, Program},
    transforms::testing::test,
    visit::{as_folder, FoldWith, VisitMut},
};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

pub struct TransformVisitor;
use swc_core::ecma::ast::Callee;
use swc_core::ecma::ast::Expr;
use swc_core::ecma::visit::VisitMutWith;

// Test
use swc_ecma_parser::{Syntax, TsConfig};

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

    // JSXの属性名を変更する
    fn visit_mut_jsx_attr_name(&mut self, n: &mut JSXAttrName) {
        if let JSXAttrName::Ident(i) = n {
            if &*i.sym == "normal" {
                let replace_name: &str = "special";
                i.sym = replace_name.into();
            }
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
    // Syntax::Typescript(Default::default()),
    // Default::default(),
    |_| as_folder(TransformVisitor),
    replace_jsx_attr_name,
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
            <div special="value">
                <h1>hello</h1>
            </div>
    }
    "#
);
