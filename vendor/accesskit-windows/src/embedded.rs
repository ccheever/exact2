// Copyright 2026 Exact contributors. Apache-2.0 OR MIT.
// @ref LLP 1115.004 D7.1.2: one tree containing a native text fragment.
use crate::{context::Context, node::runtime_id_from_node_id, util::safe_array_from_i32_slice};
use accesskit::{NodeId, TreeId};
use accesskit_consumer::FullNodeId;
use std::sync::{Arc, Weak};
use windows::{
    Win32::{System::Com::SAFEARRAY, UI::Accessibility::*},
    core::*,
};

fn id(context: &Context, local: NodeId) -> Result<FullNodeId> {
    context
        .read_tree()
        .state()
        .node_by_tree_local_id(local, TreeId::ROOT)
        .map(|node| node.id())
        .ok_or_else(|| Error::from_hresult(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32)))
}

pub(crate) fn get(context: &Context, id: FullNodeId) -> Result<Option<IRawElementProviderSimple>> {
    let reference = context.embedded.lock().unwrap().get(&id).cloned();
    reference.map(|reference| reference.resolve()).transpose()
}

pub(crate) fn set(
    context: &Arc<Context>,
    local: NodeId,
    provider: Option<&IRawElementProviderSimple>,
) -> Result<()> {
    let id = id(context, local)?;
    let reference = provider.map(AgileReference::new).transpose()?;
    let mut embedded = context.embedded.lock().unwrap();
    if let Some(reference) = reference {
        embedded.insert(id, reference);
    } else {
        embedded.remove(&id);
    }
    Ok(())
}

pub(crate) fn site(
    context: &Arc<Context>,
    local: NodeId,
) -> Result<IRawElementProviderWindowlessSite> {
    Ok(Site {
        context: Arc::downgrade(context),
        id: id(context, local)?,
    }
    .into())
}

#[implement(IRawElementProviderWindowlessSite)]
struct Site {
    context: Weak<Context>,
    id: FullNodeId,
}

#[allow(non_snake_case)]
impl IRawElementProviderWindowlessSite_Impl for Site_Impl {
    fn GetAdjacentFragment(
        &self,
        direction: NavigateDirection,
    ) -> Result<IRawElementProviderFragment> {
        if direction != NavigateDirection_Parent {
            return Err(Error::empty());
        }
        let context = self
            .context
            .upgrade()
            .ok_or_else(|| Error::from_hresult(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32)))?;
        if context.read_tree().state().node_by_id(self.id).is_none() {
            return Err(Error::from_hresult(HRESULT(
                UIA_E_ELEMENTNOTAVAILABLE as i32,
            )));
        }
        Ok(context
            .get_or_create_platform_node(self.id)
            .into_interface())
    }
    fn GetRuntimeIdPrefix(&self) -> Result<*mut SAFEARRAY> {
        Ok(safe_array_from_i32_slice(&runtime_id_from_node_id(self.id)))
    }
}
