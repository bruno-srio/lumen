use ashpd::desktop::background::Background;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use ashpd::WindowIdentifier;
use futures_util::StreamExt;
use gettextrs::gettext;
use std::sync::OnceLock;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("lumen-portals")
            .build()
            .expect("portal runtime")
    })
}

pub fn spawn(
    task: impl std::future::Future<Output = ()> + Send + 'static,
) -> tokio::task::AbortHandle {
    runtime().spawn(task).abort_handle()
}

pub async fn window_identifier(
    window: &impl gtk::prelude::IsA<gtk::Native>,
) -> Option<WindowIdentifier> {
    WindowIdentifier::from_native(window).await
}

/// `Ok(false)` means the user refused. Other portal errors are returned so the
/// caller can keep a local background mode when no portal is running.
pub async fn request_background(
    identifier: Option<WindowIdentifier>,
) -> Result<bool, ashpd::Error> {
    let reason = gettext(
        "Keep controlling lights after the window is closed, and listen for the Toggle All Lights shortcut.",
    );
    let request = Background::request()
        .identifier(identifier)
        .auto_start(Some(false))
        .reason(Some(reason.as_str()))
        .send()
        .await?;
    Ok(request.response()?.run_in_background())
}

pub async fn listen_for_toggle(
    on_toggle: impl Fn() + Send + Sync + 'static,
) -> Result<(), ashpd::Error> {
    let shortcuts = GlobalShortcuts::new().await?;
    let session = shortcuts.create_session(Default::default()).await?;
    shortcuts
        .bind_shortcuts(
            &session,
            &[NewShortcut::new("toggle-all", gettext("Toggle All Lights"))
                .preferred_trigger(Some("F8"))],
            None,
            BindShortcutsOptions::default(),
        )
        .await?
        .response()?;
    let mut activated = shortcuts.receive_activated().await?;
    while let Some(event) = activated.next().await {
        if event.shortcut_id() == "toggle-all" {
            on_toggle();
        }
    }
    Ok(())
}

pub fn invoke_on_main(callback: impl FnOnce() + Send + 'static) {
    glib::MainContext::default().invoke(callback);
}
