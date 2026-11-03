//! Namespace identity follows the selected backend, including widget groups.
use exact_data::secrets::{Backend, Namespace};
use ibex2::host::Secrets;

pub(super) struct SecretBackend(pub Secrets, pub Option<std::path::PathBuf>);
impl Backend for SecretBackend {
    fn read(&self, name: &str) -> Result<Option<String>, String> {
        self.0.get(name).map_err(|error| error.to_string())
    }
    fn write(&self, name: &str, value: Option<&str>) -> Result<(), String> {
        match value {
            Some(value) => self.0.set(name, value),
            None => self.0.forget(name),
        }
        .map_err(|error| error.to_string())?;
        if let Some(directory) = &self.1 {
            exact_data::secrets::sync_directory_chain(directory)?;
        }
        Ok(())
    }
}

pub(super) fn secret_namespace(app: &str) -> Namespace {
    let agent = std::env::var_os("EXACT_AGENT").is_some();
    let store = std::env::var("EXACT_STORE").unwrap_or_default();
    if (agent && store != "real") || store == "memory" {
        return crate::picker::agent_secret_root(app)
            .map(|root| Namespace::disk(root.join("secret-leases")))
            .unwrap_or_else(Namespace::memory);
    }
    let service = if super::group_root(app).is_some() {
        app.to_owned()
    } else {
        ibex2::secrets::app_identity()
    };
    if let Some(root) = super::group_root(&service) {
        return Namespace::named(root.join("secret-leases"), &service);
    }
    // The ordinary Keychain backend uses the bundle's identity, which can
    // differ from an embedded session's app id.
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute());
    let root = home
        .map(|home| home.join("Library/Application Support/exact/secret-leases"))
        .unwrap_or_else(|| std::path::PathBuf::from("/dev/null/exact-secret-leases"));
    Namespace::named(root, &service)
}

pub(super) fn secret_directory(app: &str) -> Option<std::path::PathBuf> {
    let agent = std::env::var_os("EXACT_AGENT").is_some();
    let store = std::env::var("EXACT_STORE").unwrap_or_default();
    ((agent && store != "real") || store == "memory")
        .then(|| crate::picker::agent_secret_root(app))
        .flatten()
        .map(|root| root.join("secrets"))
}
