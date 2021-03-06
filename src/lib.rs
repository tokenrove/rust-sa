#![feature(plugin_registrar, box_syntax, rustc_private)]
#[macro_use] extern crate rustc;
extern crate rustc_plugin;
extern crate rustc_const_eval;
extern crate syntax;

use syntax::attr::contains_name;
use rustc::lint::{LintPass, LateLintPass, LintArray, LateContext, LintContext};
use rustc_const_eval::ConstContext;
use rustc::middle::const_val::ConstVal::*;
use rustc_plugin::registry::Registry;
use rustc::hir;
use syntax::ext::base::{ExtCtxt, MacResult, DummyResult, MacEager};
use syntax::codemap::Span;
use syntax::ast::{Ident, ItemKind};
use syntax::symbol::Symbol;
use syntax::ext::build::AstBuilder;
use syntax::feature_gate::AttributeType;
use syntax::tokenstream::TokenTree;

declare_lint!(STATIC_ASSERT, Deny,
              "check compile-time information");

struct StaticAssertPass {
    in_static_assert: bool
}

impl LintPass for StaticAssertPass {
    fn get_lints(&self) -> LintArray {
        lint_array!(STATIC_ASSERT)
    }
}

impl<'a, 'tcx> LateLintPass<'a,'tcx> for StaticAssertPass {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, expr: &'tcx hir::Expr) {
        if !self.in_static_assert { return; }
        self.in_static_assert = false;
        let evaluated = ConstContext::with_tables(cx.tcx, cx.tables).eval(expr);
        match evaluated {
            Ok(Bool(true)) => {},
            Ok(Bool(false)) => cx.span_lint(STATIC_ASSERT, expr.span, "static assertion failed"),
            c => cx.sess().struct_span_err(expr.span, &format!("static assertion on {:?}", c)).emit(),
        }
    }

    fn check_item(&mut self, _cx: &LateContext<'a, 'tcx>, it: &'tcx hir::Item) {
        if !contains_name(&it.attrs, "static_assert_helper_attribute") {
            return;
        }
        self.in_static_assert = true;
    }

    fn check_item_post(&mut self, _cx: &LateContext<'a, 'tcx>, _it: &'tcx hir::Item) {
        self.in_static_assert = false;
    }
}

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_late_lint_pass(box StaticAssertPass { in_static_assert: false });
    reg.register_macro("static_assert", static_assert_expand);
    reg.register_attribute("static_assert_helper_attribute".to_owned(), AttributeType::Whitelisted)
}

fn static_assert_expand<'cx>(cx: &'cx mut ExtCtxt, sp: Span, args: &[TokenTree]) -> Box<MacResult + 'cx> {
        match cx.new_parser_from_tts(args).parse_expr() {
            Ok(e) => {
                let item = cx.item(
                    sp,
                    Ident::with_empty_ctxt(Symbol::gensym("ASSERTION")),
                    vec![
                        cx.attribute(sp, cx.meta_word(sp, Symbol::intern("static_assert_helper_attribute"))),
                        cx.attribute(sp, cx.meta_list(sp, Symbol::intern("allow"), vec![cx.meta_list_item_word(sp, Symbol::intern("dead_code"))])),
                    ],
                    ItemKind::Const(cx.ty_ident(sp, cx.ident_of("bool")), e),
                );
                box MacEager {
                    items: Some(syntax::util::small_vector::SmallVector::one(item.clone())),
                    stmts: Some(syntax::util::small_vector::SmallVector::one(cx.stmt_item(sp, item.clone()))),
                    expr: Some(cx.expr_block(cx.block(sp, vec![cx.stmt_item(sp, item)]))),
                    .. MacEager::default()
                }
            },
            Err(mut e) => {
                e.emit();
                DummyResult::any(sp)
            },
        }
}
