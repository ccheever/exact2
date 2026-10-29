//! The taskbar's jump list (LLP 1133.023 D3, D17): the app manifest's
//! `shortcuts` as its tasks, each a shell link running this executable with
//! the item's location as its one argument, which the next launch reads as
//! its launch location (`exact_linux::app::launch_location`). The items
//! come from the build's compatibility record, which carries the manifest's
//! `shortcuts` for every native binary (`bake/src/compat.rs`).
//!
//! The calls are those of Microsoft's taskbar extensions
//! (<https://learn.microsoft.com/windows/win32/shell/taskbar-extensions>):
//! `ICustomDestinationList::BeginList`, `AddUserTasks` with an
//! `IObjectCollection` of `IShellLinkW`, each titled by its property store's
//! `PKEY_Title`, then `CommitList`. An unpackaged app may set one.
#![allow(unsafe_code)]

#[cfg(windows)]
use windows::core::{Interface, HSTRING, PCWSTR};
#[cfg(windows)]
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
#[cfg(windows)]
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
#[cfg(windows)]
use windows::Win32::System::Variant::VT_LPWSTR;
#[cfg(windows)]
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
#[cfg(windows)]
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, SHStrDupW,
    ShellLink,
};

/// A task: its title and the location it opens.
#[derive(Debug, PartialEq)]
pub(crate) struct Task {
    pub title: String,
    pub location: String,
}

/// The manifest's items in `compat`, in order: each with a name and an app
/// path.
pub(crate) fn tasks(compat: &str) -> Vec<Task> {
    let Ok(record) = serde_json::from_str::<serde_json::Value>(compat) else {
        return Vec::new();
    };
    let items = record["inputs"]["shortcuts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    items
        .iter()
        .filter_map(|item| {
            let title = item["name"].as_str().filter(|n| !n.is_empty())?;
            let location = item["url"]
                .as_str()
                .filter(|u| u.starts_with('/') && !u.starts_with("//"))?;
            Some(Task {
                title: title.to_owned(),
                location: location.to_owned(),
            })
        })
        .collect()
}

/// One command-line argument as `CommandLineToArgvW` reads it back: quoted,
/// a quote escaped and the backslashes before one doubled
/// (<https://learn.microsoft.com/cpp/c-language/parsing-c-command-line-arguments>).
pub(crate) fn argument(text: &str) -> String {
    let mut out = String::from('"');
    let mut slashes = 0;
    for c in text.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                out.push('"');
                slashes = 0;
                continue;
            }
            _ => {}
        }
        if c != '\\' {
            out.extend(std::iter::repeat_n('\\', slashes));
            slashes = 0;
            out.push(c);
        }
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}

#[cfg(windows)]
/// Set the jump list's tasks to the manifest's items in `compat`; with none,
/// the app's tasks are cleared. Runs on an owned worker after first pixel,
/// with its own COM apartment. A failure is logged: the app runs without the list.
pub(crate) fn set_from_compat(compat: &str) {
    if let Err(error) = set(&tasks(compat)) {
        eprintln!("exact-windows: the jump list's tasks: {error}");
    }
}

#[cfg(windows)]
fn set(tasks: &[Task]) -> windows::core::Result<()> {
    let exe = std::env::current_exe().map_err(|_| windows::core::Error::from_thread())?;
    let exe = HSTRING::from(exe.as_os_str());
    with_list(|list| set_list(list, tasks, &exe))
}

#[cfg(windows)]
fn with_list(
    apply: impl FnOnce(&ICustomDestinationList) -> windows::core::Result<()>,
) -> windows::core::Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    let result = (|| unsafe {
        let list: ICustomDestinationList =
            CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        apply(&list)
    })();
    unsafe { CoUninitialize() };
    result
}

#[cfg(windows)]
fn set_list(
    list: &ICustomDestinationList,
    tasks: &[Task],
    exe: &HSTRING,
) -> windows::core::Result<()> {
    unsafe {
        let mut slots = 0u32;
        let _removed: IObjectArray = list.BeginList(&mut slots)?;
        // AddUserTasks requires at least one item. Committing without it clears old tasks.
        if tasks.is_empty() {
            return list.CommitList();
        }
        let collection: IObjectCollection =
            CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        for task in tasks {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(exe)?;
            link.SetArguments(&HSTRING::from(argument(&task.location)))?;
            link.SetIconLocation(exe, 0)?;
            let store: IPropertyStore = link.cast()?;
            store.SetValue(&PKEY_Title, &title(&task.title)?)?;
            store.Commit()?;
            collection.AddObject(&link)?;
        }
        let array: IObjectArray = collection.cast()?;
        list.AddUserTasks(&array)?;
        list.CommitList()
    }
}

#[cfg(windows)]
/// A `VT_LPWSTR` value, as the shell reads a link's title (a `BSTR` from
/// `PROPVARIANT::from(&str)` is another type); its drop frees the string.
fn title(text: &str) -> windows::core::Result<PROPVARIANT> {
    let mut value = PROPVARIANT::default();
    unsafe {
        let copy = SHStrDupW(PCWSTR(HSTRING::from(text).as_ptr()))?;
        let inner = &mut *value.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = copy;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn empty_and_populated_task_lists_commit() {
        with_list(|list| {
            let exe = HSTRING::from(std::env::current_exe().unwrap().as_os_str());
            let app = HSTRING::from(format!("Exact.JumpListTest.{}", std::process::id()));
            unsafe {
                list.SetAppID(&app)?;
                let result = (|| {
                    set_list(list, &[], &exe)?;
                    set_list(
                        list,
                        &[Task {
                            title: "Open test item".into(),
                            location: "/item/1?from=jump".into(),
                        }],
                        &exe,
                    )?;
                    set_list(list, &[], &exe)
                })();
                let _ = list.AbortList();
                result.and(list.DeleteList(&app))
            }
        })
        .unwrap();
    }

    #[test]
    fn the_manifests_items_with_an_app_path_are_the_tasks() {
        let compat = r#"{"inputs":{"shortcuts":[{"name":"Add Book","url":"/add"},{"name":"Away","url":"https://example.com"},{"name":"","url":"/x"},{"name":"Book","url":"/book/1?from=jump"}]}}"#;
        assert_eq!(
            tasks(compat),
            [
                Task {
                    title: "Add Book".into(),
                    location: "/add".into()
                },
                Task {
                    title: "Book".into(),
                    location: "/book/1?from=jump".into()
                },
            ]
        );
        assert!(tasks(r#"{"inputs":{}}"#).is_empty());
    }

    #[test]
    fn an_argument_reads_back_as_written() {
        assert_eq!(argument("/add"), r#""/add""#);
        assert_eq!(argument(r#"/a "b"\"#), r#""/a \"b\"\\""#);
        assert_eq!(argument(r"/a\\b"), r#""/a\\b""#);
        assert_eq!(argument(r#"/a\"b"#), r#""/a\\\"b""#);
    }
}
