use spin_app::AppComponent;

use composer::SpinComposer;
mod composer;
mod isolate;

/// Composes a Spin AppComponent using the imports specified in the component's imports section.
/// 
/// To enforce capability isolation (e.g. environment, file system, etc.) each component is
/// preprocessed to prefix the interface name (representing the capability) with the component id
/// defined in the `spin.toml`. For example, a component named `foo` will have its import of
/// "wasi:cli/environment" renamed to "foo-wasi:cli/environment". Therefore each component in a
/// composition will import a unique instance of each of these capabilities that effectively isolate
/// its access to only whats explicity specified in their respective component section in the `spin.toml`.
pub fn compose<L>(component: &AppComponent<'_, L>) -> anyhow::Result<Vec<u8>> {
    SpinComposer::new().compose(component)
}