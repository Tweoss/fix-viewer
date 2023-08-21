use std::sync::{mpsc::Sender, Arc};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;

use crate::handle::{Handle, Operation, Task};

pub(crate) enum Response {
    Parents(Option<Vec<Task>>),
    Child(Option<Handle>),
    Dependees(Vec<Task>),
}

pub(crate) fn get<T, S, F>(
    client: Arc<Client>,
    ctx: egui::Context,
    handle: Handle,
    url: String,
    map: F,
    tx: Sender<(Handle, Result<S>)>,
) where
    T: DeserializeOwned + Send,
    S: Send + 'static,
    F: FnOnce(T) -> Result<S> + Send + 'static,
{
    let task = async move {
        let result = client.get(url).send().await;
        match result {
            Ok(ok) => {
                let json = ok.json::<T>().await;
                let _ = tx.send((handle, json.context("parsing json").and_then(map)));
            }
            Err(e) => {
                let _ = tx.send((
                    handle,
                    Err(anyhow::anyhow!(format!(
                        "request failed: {} error",
                        match () {
                            () if e.is_builder() => "building url",
                            () if e.is_request() => "request",
                            () if e.is_redirect() => "redirect",
                            () if e.is_status() => "status code",
                            () if e.is_body() => "body",
                            () if e.is_decode() => "decode",
                            () if e.is_timeout() => "timeout",
                            () => "unknown",
                        }
                    ))),
                ));
            }
        }
        ctx.request_repaint();
    };
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(task);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = tokio::spawn(task);
}

#[derive(serde::Deserialize)]
struct JsonTask {
    handle: String,
    operation: String,
}

pub(crate) fn get_parents(
    client: Arc<Client>,
    ctx: egui::Context,
    handle: &Handle,
    tx: Sender<(Handle, Result<Response>)>,
    url_base: &str,
) {
    #[derive(serde::Deserialize)]
    struct JsonResponse {
        parents: Option<Vec<JsonTask>>,
    }

    get(
        client,
        ctx,
        handle.clone(),
        format!("http://{url_base}/parents?handle={}", handle.to_hex()),
        |json: JsonResponse| {
            let Some(json_parents )= json.parents else {
                return Ok(Response::Parents(None));
            };
            Ok(Response::Parents(Some(
                json_parents
                    .iter()
                    .map(|json_task| {
                        Ok::<Task, anyhow::Error>(Task {
                            handle: Handle::from_hex(&json_task.handle)
                                .context("parsing handle")?,
                            operation: json_task
                                .operation
                                .parse::<u8>()
                                .context("parsing operation as u8")?
                                .try_into()
                                .context("casting u8 to operation")?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            )))
        },
        tx,
    );
}

pub(crate) fn get_dependees(
    client: Arc<Client>,
    ctx: egui::Context,
    handle: Handle,
    tx: Sender<(Handle, Result<Response>)>,
    url_base: &str,
) {
    #[derive(serde::Deserialize)]
    struct JsonResponse {
        dependees: Vec<JsonTask>,
    }

    get(
        client,
        ctx,
        handle.clone(),
        format!("http://{url_base}/dependees?handle={}", handle.to_hex()),
        |json: JsonResponse| {
            Ok(Response::Parents(Some(
                json.dependees
                    .iter()
                    .map(|json_task| {
                        Ok::<Task, anyhow::Error>(Task {
                            handle: Handle::from_hex(&json_task.handle)
                                .context("parsing handle")?,
                            operation: json_task
                                .operation
                                .parse::<u8>()
                                .context("parsing operation as u8")?
                                .try_into()
                                .context("casting u8 to operation")?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            )))
        },
        tx,
    );
}

pub(crate) fn get_child(
    client: Arc<Client>,
    ctx: egui::Context,
    handle: Handle,
    operation: Operation,
    tx: Sender<(Handle, Result<Response>)>,
    url_base: &str,
) {
    #[derive(serde::Deserialize)]
    struct JsonResponse {
        child: Option<String>,
    }

    get(
        client,
        ctx,
        handle.clone(),
        format!(
            "http://{url_base}/child?handle={}+op={}",
            handle.to_hex(),
            operation as u8
        ),
        |json: JsonResponse| {
            Ok(Response::Child(json.child.and_then(|handle| {
                Handle::from_hex(&handle).context("parsing handle").ok()
            })))
        },
        tx,
    );
}
