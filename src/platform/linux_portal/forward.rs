//! The portal after Oxyde's in `portals.conf`, which answers the calls Oxyde's window
//! cannot: it gets the same arguments, as if xdg-desktop-portal had asked it, and its
//! answer goes back as Oxyde's.
use super::chooser::{Call, Options, ENDED, PATH};
use futures::FutureExt;
use std::sync::OnceLock;
use zbus::zvariant::OwnedObjectPath;

const REQUEST: &str = "org.freedesktop.impl.portal.Request";

/// A connection without Oxyde's own five-second limit: the fallback portal answers only
/// once the user has chosen, however long that takes.
async fn connection() -> zbus::Result<&'static zbus::Connection> {
    static CONNECTION: OnceLock<zbus::Connection> = OnceLock::new();
    if let Some(connection) = CONNECTION.get() {
        return Ok(connection);
    }
    let connection = zbus::Connection::session().await?;
    Ok(CONNECTION.get_or_init(|| connection))
}

/// Withdraws the request the fallback portal serves at `path`.
pub(super) async fn close(backend: &str, path: &OwnedObjectPath) {
    let result = match connection().await {
        Ok(connection) => connection.call_method(Some(backend), path, Some(REQUEST), "Close", &()).await.map(drop),
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        log::warn!("Cannot withdraw the request from {backend}: {error}");
    }
}

/// Asks `backend` to answer `method`; withdraws it when the portal that called leaves
/// `watched`, Oxyde's own connection.
pub(super) async fn ask(backend: &str, method: &str, call: &Call, watched: &zbus::Connection) -> (u32, Options) {
    let connection = match connection().await {
        Ok(connection) => connection,
        Err(error) => {
            log::warn!("Cannot reach {backend} for {method}: {error}");
            return (ENDED, Options::new());
        }
    };
    let arguments = (&call.handle, &call.app_id, &call.parent_window, &call.title, &call.options);
    let answer = connection.call_method(Some(backend), PATH, Some(super::INTERFACE), method, &arguments).fuse();
    let abandoned = call.abandoned(watched).fuse();
    let (mut answer, mut abandoned) = (std::pin::pin!(answer), std::pin::pin!(abandoned));
    futures::select_biased! {
        answer = answer => match answer.and_then(|message| message.body().deserialize::<(u32, Options)>()) {
            Ok(answer) => answer,
            Err(error) => {
                log::warn!("{backend} did not answer {method}: {error}");
                (ENDED, Options::new())
            }
        },
        () = abandoned => {
            close(backend, &call.handle).await;
            (ENDED, Options::new())
        }
    }
}
