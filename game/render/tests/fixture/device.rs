//! GPU absence is the only permitted skip; configuration and device errors fail.
pub fn device_or_skip<T>(result: Result<T, String>) -> Option<T> {
    match result {
        Ok(gpu) => Some(gpu),
        Err(reason) if reason.starts_with("no adapter:") => {
            eprintln!("SKIP exact-game-render GPU test: {reason}");
            None
        }
        Err(reason) => panic!("GPU device request failed: {reason}"),
    }
}
#[test]
fn only_classified_no_adapter_may_skip() {
    assert_eq!(device_or_skip(Ok(7)), Some(7));
    assert_eq!(
        device_or_skip::<()>(Err("no adapter: unavailable".into())),
        None
    );
    for reason in ["device lost", "request device: unsupported limits"] {
        assert!(std::panic::catch_unwind(|| device_or_skip::<()>(Err(reason.into()))).is_err());
    }
}
