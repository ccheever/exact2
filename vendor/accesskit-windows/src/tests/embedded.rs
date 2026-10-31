// Copyright 2026 Exact contributors. Apache-2.0 OR MIT.
//! Embedded fragments through the public registration API and COM providers.
use super::*;
use crate::context::Context;
use accesskit::{ActionRequest, Node, NodeId, Rect, Role, TreeId, TreeInfo};
use std::sync::Arc;
use windows::Win32::{
    System::{Com::*, Ole::*, Variant::*},
    UI::Accessibility::*,
};

struct NullActionHandler;
impl ActionHandler for NullActionHandler {
    fn do_action(&mut self, _: ActionRequest) {}
}

const ROOT: NodeId = NodeId(0);
const FIELD: NodeId = NodeId(1);

fn tree(with_field: bool) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_bounds(Rect::new(0., 0., 200., 100.));
    let mut nodes = vec![];
    if with_field {
        root.set_children([FIELD]);
        let mut field = Node::new(Role::TextInput);
        field.set_bounds(Rect::new(10., 10., 190., 50.));
        nodes.push((FIELD, field));
    }
    nodes.push((ROOT, root));
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus: if with_field { FIELD } else { ROOT },
    }
}
struct Activate;
impl ActivationHandler for Activate {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(tree(true))
    }
}

fn scope(f: impl FnOnce(HWND, &mut SubclassingAdapter, &Arc<Context>) -> Result<()>) -> Result<()> {
    let _lock = crate::tests::MUTEX.lock();
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    let _com = scopeguard::guard((), |_| unsafe { CoUninitialize() });
    let hwnd = unsafe {
        CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("Embedded test"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            200,
            100,
            None,
            None,
            None,
            None,
        )?
    };
    let _window = scopeguard::guard(hwnd, |hwnd| unsafe {
        let _ = DestroyWindow(hwnd);
    });
    let mut adapter = SubclassingAdapter::new(hwnd, Activate, NullActionHandler);
    {
        let mut state = adapter.0.state.borrow_mut();
        let _ = state.adapter.handle_wm_getobject(
            WPARAM(0),
            LPARAM(UiaRootObjectId as isize),
            &mut Activate,
        );
        let _ = state.adapter.update_window_focus_state(true);
    }
    let context = Arc::clone(
        adapter
            .0
            .state
            .borrow()
            .adapter
            .embedding_context()
            .unwrap(),
    );
    f(hwnd, &mut adapter, &context)
}

fn node(context: &Arc<Context>, id: NodeId) -> IRawElementProviderFragment {
    let id = context
        .read_tree()
        .state()
        .node_by_tree_local_id(id, TreeId::ROOT)
        .unwrap()
        .id();
    context.get_or_create_platform_node(id).into_interface()
}

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct Native {
    site: IRawElementProviderWindowlessSite,
    refuse_hit: bool,
}
#[allow(non_snake_case)]
impl IRawElementProviderSimple_Impl for Native_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }
    fn GetPatternProvider(&self, _: UIA_PATTERN_ID) -> Result<IUnknown> {
        Err(Error::empty())
    }
    fn GetPropertyValue(&self, _: UIA_PROPERTY_ID) -> Result<VARIANT> {
        Ok(VARIANT::default())
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        Err(Error::empty())
    }
}
#[allow(non_snake_case)]
impl IRawElementProviderFragment_Impl for Native_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        unsafe { self.site.GetAdjacentFragment(direction) }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        unsafe { self.site.GetRuntimeIdPrefix() }
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        Ok(UiaRect::default())
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        Ok(self.to_interface())
    }
}
#[allow(non_snake_case)]
impl IRawElementProviderFragmentRoot_Impl for Native_Impl {
    fn ElementProviderFromPoint(&self, _: f64, _: f64) -> Result<IRawElementProviderFragment> {
        if self.refuse_hit {
            Err(Error::empty())
        } else {
            Ok(self.to_interface())
        }
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        Ok(self.to_interface())
    }
}
#[implement(IRawElementProviderSimple)]
struct NoRoot;
#[allow(non_snake_case)]
impl IRawElementProviderSimple_Impl for NoRoot_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }
    fn GetPatternProvider(&self, _: UIA_PATTERN_ID) -> Result<IUnknown> {
        Err(Error::empty())
    }
    fn GetPropertyValue(&self, _: UIA_PROPERTY_ID) -> Result<VARIANT> {
        Ok(VARIANT::default())
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        Err(Error::empty())
    }
}
fn embed(hwnd: HWND, refuse_hit: bool) -> Result<IRawElementProviderSimple> {
    let provider: IRawElementProviderSimple = Native {
        site: windowless_site(hwnd, FIELD)?,
        refuse_hit,
    }
    .into();
    embed_windowless(hwnd, FIELD, Some(&provider))?;
    Ok(provider)
}

#[test]
fn embedded_navigation_and_roots() -> Result<()> {
    scope(|hwnd, _, context| {
        let field = node(context, FIELD);
        let provider = embed(hwnd, false)?;
        let fragment: IRawElementProviderFragment = provider.cast()?;
        unsafe {
            assert_eq!(field.Navigate(NavigateDirection_FirstChild)?, fragment);
            assert_eq!(field.Navigate(NavigateDirection_LastChild)?, fragment);
            assert_eq!(fragment.Navigate(NavigateDirection_Parent)?, field);
            let roots = field.GetEmbeddedFragmentRoots()?;
            assert!(!roots.is_null());
            let _array = scopeguard::guard(roots, |p| {
                let _ = SafeArrayDestroy(p);
            });
            assert_eq!(SafeArrayGetUBound(roots, 1)?, 0);
            let mut value = std::ptr::null_mut::<c_void>();
            SafeArrayGetElement(roots, &0, (&mut value as *mut *mut c_void).cast())?;
            let root = IUnknown::from_raw(value);
            assert_eq!(root, provider.cast::<IUnknown>()?);
            let prefix = fragment.GetRuntimeId()?;
            assert!(!prefix.is_null());
            SafeArrayDestroy(prefix)?;
        }
        Ok(())
    })
}
#[test]
fn embedded_hit_test_and_fallback() -> Result<()> {
    scope(|hwnd, _, context| {
        let root: IRawElementProviderFragmentRoot = node(context, ROOT).cast()?;
        let field = node(context, FIELD);
        let origin = context.client_top_left();
        let hit = || unsafe { root.ElementProviderFromPoint(origin.x + 20., origin.y + 20.) };
        assert_eq!(hit()?, field);
        let provider = embed(hwnd, false)?;
        assert_eq!(hit()?, provider.cast()?);
        let _ = embed(hwnd, true)?;
        assert_eq!(hit()?, field);
        let no_root: IRawElementProviderSimple = NoRoot.into();
        embed_windowless(hwnd, FIELD, Some(&no_root))?;
        assert_eq!(hit()?, field);
        Ok(())
    })
}
#[test]
fn embedded_focus() -> Result<()> {
    scope(|hwnd, _, context| {
        let root: IRawElementProviderFragmentRoot = node(context, ROOT).cast()?;
        let provider = embed(hwnd, false)?;
        assert_eq!(unsafe { root.GetFocus()? }, provider.cast()?);
        embed_windowless(hwnd, FIELD, None)?;
        assert_eq!(unsafe { root.GetFocus()? }, node(context, FIELD));
        Ok(())
    })
}
#[test]
fn embedded_removal() -> Result<()> {
    scope(|hwnd, adapter, context| {
        let field = node(context, FIELD);
        let _ = embed(hwnd, false)?;
        embed_windowless(hwnd, FIELD, None)?;
        assert!(unsafe { field.GetEmbeddedFragmentRoots()? }.is_null());
        assert!(unsafe { field.Navigate(NavigateDirection_FirstChild) }.is_err());
        let provider = embed(hwnd, false)?;
        let fragment: IRawElementProviderFragment = provider.cast()?;
        let _ = adapter.update_if_active(|| tree(false));
        assert!(context.embedded.lock().unwrap().is_empty());
        assert!(unsafe { fragment.Navigate(NavigateDirection_Parent) }.is_err());
        assert!(embed_windowless(hwnd, FIELD, Some(&provider)).is_err());
        assert!(windowless_site(hwnd, FIELD).is_err());
        Ok(())
    })
}
