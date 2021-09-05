/*!
Transform IRNode, including these original transformation:

hoistStatic
transformExpression
mergeText
vOnce
vMemo
trackScopes
 */
pub trait Transformer {
    type IR;
    /// transform will change ir node inplace
    /// usually transform will have multiple passes
    fn transform(&self, node: &mut Self::IR);
}

// default transforms
pub fn hoist_static() {}
pub fn track_v_for_slot_scopes() {}
pub fn track_slot_scopes() {}
pub fn merge_text_call() {}
pub fn prefix_expression() {}
pub fn transform_memo() {}
pub fn transform_once() {}

enum NodeChange<T: 'static> {
    Replace(Vec<T>),
    Delete,
}

trait TransformOp {}
