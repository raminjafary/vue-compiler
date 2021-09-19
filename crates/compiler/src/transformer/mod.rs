/*!
Transform IRNode.
This module contains the canonical transformations from vue-next and
the original ones for the parity of features not implemented in Convert.

## Canonical
* hoistStatic
* transformExpression
* ~~vOnce (moved to convert)~~
* ~~vMemo (moved to convert)~~
* trackScopes

## Original
* collect_helper: track all helpers used in AST. Vue track it by helper/helperString.
* collect_asset: track all components/directives used in AST.
* mergeText: merge consecutive text call
* patch_flag: seems patch flag can be extracted out

 */

mod optimize_text;

use super::converter::{
    self as C, BaseConvertInfo, BaseRoot, ConvertInfo, IRNode, IRRoot, RuntimeDir,
};
pub trait Transformer {
    type IR;
    /// transform will change ir node inplace
    /// usually transform will have multiple passes
    fn transform(&mut self, root: &mut Self::IR);
}

use std::marker::PhantomData;
struct NoopTransformer<T>(PhantomData<T>);

impl<T> Transformer for NoopTransformer<T> {
    type IR = T;
    fn transform(&mut self, _root: &mut Self::IR) {
        // noop
    }
}

trait CoreTransformer<T: ConvertInfo>: Transformer {
    fn get_passes(&mut self) -> &mut [Box<dyn CoreTransformPass<T>>];
    #[inline(always)]
    fn enter<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Box<dyn CoreTransformPass<T>>),
    {
        for pass in self.get_passes() {
            f(pass);
        }
    }
    #[inline(always)]
    fn exit<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Box<dyn CoreTransformPass<T>>),
    {
        for pass in self.get_passes().iter_mut().rev() {
            f(pass);
        }
    }
    fn transform_root(&mut self, root: &mut IRRoot<T>);
    fn transform_ir(&mut self, ir: &mut IRNode<T>) {
        use IRNode as I;
        match ir {
            I::TextCall(t) => self.transform_text(t),
            I::If(i) => self.transform_if(i),
            I::For(f) => self.transform_for(f),
            I::VNodeCall(v) => self.transform_vnode(v),
            I::RenderSlotCall(r) => self.transform_slot_outlet(r),
            I::CommentCall(c) => self.transform_comment(c),
            I::VSlotUse(s) => self.transform_v_slot(s),
            I::AlterableSlot(_) => {
                panic!("should not call");
            }
        }
    }
    fn transform_children(&mut self, children: &mut Vec<IRNode<T>>) {
        self.enter(|p| p.enter_children(children));
        for child in children.iter_mut() {
            self.transform_ir(child);
        }
        self.exit(|p| p.exit_children(children));
    }
    fn transform_text(&mut self, t: &mut T::TextType) {
        self.enter(|p| p.enter_text(t));
        self.exit(|p| p.exit_text(t));
    }
    fn transform_if(&mut self, i: &mut C::IfNodeIR<T>) {
        self.enter(|p| p.enter_if(i));
        for branch in i.branches.iter_mut() {
            if let Some(c) = branch.condition.as_mut() {
                self.transform_js_expr(c);
            }
            self.transform_ir(&mut branch.child);
        }
        self.exit(|p| p.exit_if(i));
    }
    fn transform_for(&mut self, f: &mut C::ForNodeIR<T>) {
        self.enter(|p| p.enter_for(f));
        self.transform_js_expr(&mut f.source);
        // TODO val, key, index should not counted as expr?
        self.transform_ir(&mut f.child);
        self.exit(|p| p.exit_for(f));
    }
    fn transform_vnode(&mut self, v: &mut C::VNodeIR<T>) {
        self.enter(|p| p.enter_vnode(v));
        self.transform_js_expr(&mut v.tag);
        if let Some(props) = v.props.as_mut() {
            self.transform_js_expr(props);
        }
        self.transform_children(&mut v.children);
        for dir in v.directives.iter_mut() {
            self.transform_runtime_dir(dir);
        }
        self.exit(|p| p.exit_vnode(v));
    }
    fn transform_runtime_dir(&mut self, dir: &mut RuntimeDir<T>) {
        self.transform_js_expr(&mut dir.name);
        if let Some(expr) = dir.expr.as_mut() {
            self.transform_js_expr(expr);
        }
        if let Some(arg) = dir.arg.as_mut() {
            self.transform_js_expr(arg);
        }
        if let Some(mods) = dir.mods.as_mut() {
            self.transform_js_expr(mods);
        }
    }
    fn transform_slot_outlet(&mut self, r: &mut C::RenderSlotIR<T>) {
        self.enter(|p| p.enter_slot_outlet(r));
        self.transform_js_expr(&mut r.slot_name);
        if let Some(props) = r.slot_props.as_mut() {
            self.transform_js_expr(props);
        }
        self.transform_children(&mut r.fallbacks);
        self.exit(|p| p.exit_slot_outlet(r));
    }
    fn transform_v_slot(&mut self, s: &mut C::VSlotIR<T>) {
        self.enter(|p| p.enter_v_slot(s));
        // TODO slot param should not counted as expr?
        for slot in s.stable_slots.iter_mut() {
            self.transform_js_expr(&mut slot.name);
            self.transform_children(&mut slot.body);
        }
        for slot in s.alterable_slots.iter_mut() {
            self.transform_ir(slot);
        }
        self.exit(|p| p.exit_v_slot(s));
    }
    fn transform_js_expr(&mut self, e: &mut T::JsExpression) {
        self.enter(|p| p.enter_js_expr(e));
        self.exit(|p| p.exit_js_expr(e));
    }
    fn transform_comment(&mut self, c: &mut T::CommentType) {
        self.enter(|p| p.enter_comment(c));
        self.exit(|p| p.exit_comment(c));
    }
}

pub trait CoreTransformPass<T: ConvertInfo> {
    fn enter_children(&mut self, cs: &mut Vec<IRNode<T>>) {}
    fn exit_children(&mut self, cs: &mut Vec<IRNode<T>>) {}
    fn enter_text(&mut self, t: &mut T::TextType) {}
    fn exit_text(&mut self, t: &mut T::TextType) {}
    fn enter_if(&mut self, i: &mut C::IfNodeIR<T>) {}
    fn exit_if(&mut self, i: &mut C::IfNodeIR<T>) {}
    fn enter_for(&mut self, f: &mut C::ForNodeIR<T>) {}
    fn exit_for(&mut self, f: &mut C::ForNodeIR<T>) {}
    fn enter_vnode(&mut self, v: &mut C::VNodeIR<T>) {}
    fn exit_vnode(&mut self, v: &mut C::VNodeIR<T>) {}
    fn enter_slot_outlet(&mut self, r: &mut C::RenderSlotIR<T>) {}
    fn exit_slot_outlet(&mut self, r: &mut C::RenderSlotIR<T>) {}
    fn enter_v_slot(&mut self, s: &mut C::VSlotIR<T>) {}
    fn exit_v_slot(&mut self, s: &mut C::VSlotIR<T>) {}
    fn enter_js_expr(&mut self, e: &mut T::JsExpression) {}
    fn exit_js_expr(&mut self, e: &mut T::JsExpression) {}
    fn enter_comment(&mut self, c: &mut T::CommentType) {}
    fn exit_comment(&mut self, c: &mut T::CommentType) {}
}

type BaseTransformPass<'a> = dyn CoreTransformPass<BaseConvertInfo<'a>>;
pub struct BaseTransformer<'a, const N: usize> {
    passes: [Box<BaseTransformPass<'a>>; N],
}

impl<'a, const N: usize> Transformer for BaseTransformer<'a, N> {
    type IR = BaseRoot<'a>;
    fn transform(&mut self, node: &mut Self::IR) {
        self.transform_root(node);
    }
}

impl<'a, const N: usize> CoreTransformer<BaseConvertInfo<'a>> for BaseTransformer<'a, N> {
    fn get_passes(&mut self) -> &mut [Box<dyn CoreTransformPass<BaseConvertInfo<'a>>>] {
        &mut self.passes
    }

    fn transform_root(&mut self, root: &mut IRRoot<BaseConvertInfo<'a>>) {
        self.transform_children(&mut root.body);
    }
}

// default transforms
pub fn post_process_v_for_child() {
    // 1. inject key to slot
    // 2. Reuse the child's codegenNode but mark it as a block.
}

#[cfg(test)]
mod test {
    use super::*;
    pub use crate::converter::test::base_convert;
    pub fn get_transformer<'a, P>(pass: P) -> BaseTransformer<'a, 1>
    where
        P: CoreTransformPass<BaseConvertInfo<'a>> + 'static,
    {
        BaseTransformer {
            passes: [Box::new(pass)],
        }
    }
}
