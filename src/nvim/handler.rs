use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use nvim_rs::compat::tokio::Compat;
use nvim_rs::{Handler, Neovim, Value};
use tokio::process::ChildStdin;
use winit::event_loop::EventLoopProxy;

use super::redraw::{RedrawEvent, parse_redraw};
use crate::app::UserEvent;

pub type Writer = Compat<ChildStdin>;

#[derive(Clone)]
pub struct RedrawHandler {
    pending: Arc<Mutex<Vec<RedrawEvent>>>,
    proxy: EventLoopProxy<UserEvent>,
    started: Instant,
    first_flush: Arc<std::sync::Once>,
    restart: Arc<Mutex<Option<String>>>,
}

impl RedrawHandler {
    pub fn new(
        proxy: EventLoopProxy<UserEvent>,
        started: Instant,
        restart: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            pending: Arc::default(),
            proxy,
            started,
            first_flush: Arc::new(std::sync::Once::new()),
            restart,
        }
    }

    fn on_redraw(&self, args: Vec<Value>) {
        let mut pending = self.pending.lock().unwrap();
        let mut scan_from = pending.len();
        parse_redraw(args, &mut pending);
        for event in &pending[scan_from..] {
            if let RedrawEvent::Restart(addr) = event {
                log::info!(":restart: new server at {addr}");
                *self.restart.lock().unwrap() = Some(addr.clone());
            }
        }

        while let Some(offset) = pending[scan_from..]
            .iter()
            .position(|e| *e == RedrawEvent::Flush)
        {
            let end = scan_from + offset + 1;
            // eprintln!("batch {} events, pending {}", end, pending.len());
            let batch = if end == pending.len() {
                std::mem::take(&mut *pending)
            } else {
                pending.drain(..end).collect()
            };
            self.first_flush.call_once(|| {
                log::info!("first flush {:?} after startup", self.started.elapsed());
            });
            if self
                .proxy
                .send_event(UserEvent::Redraw(batch, Instant::now()))
                .is_err()
            {
                return;
            }
            scan_from = 0;
        }
    }
}

#[async_trait]
impl Handler for RedrawHandler {
    type Writer = Writer;

    async fn handle_request(
        &self,
        name: String,
        args: Vec<Value>,
        _nvim: Neovim<Writer>,
    ) -> Result<Value, Value> {
        if name != "nvright_image_size" {
            return Err(format!("unknown request {name}").into());
        }
        let arg = |i: usize| {
            args.get(i)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let path = crate::kitty::host_path(&arg(1), &arg(0));
        let size = tokio::task::spawn_blocking(move || crate::kitty::dimensions(&path))
            .await
            .map_err(|e| Value::from(e.to_string()))?;
        match size {
            Ok((w, h)) => Ok(Value::Array(vec![w.into(), h.into()])),
            Err(err) => {
                log::debug!("image size: {err}");
                Ok(Value::Nil)
            }
        }
    }

    async fn handle_notify(&self, name: String, args: Vec<Value>, _nvim: Neovim<Writer>) {
        match name.as_str() {
            "redraw" => self.on_redraw(args),
            "nvright_md" => match crate::preview::parse_notification(args) {
                Some(event) => {
                    let _ = self.proxy.send_event(UserEvent::Preview(event));
                }
                None => log::debug!("preview: unrecognized notification"),
            },
            "nvright_kitty" => {
                let mut args = args.into_iter();
                let kind = args.next();
                let bytes = match args.next() {
                    Some(Value::String(s)) => s.into_bytes(),
                    Some(Value::Binary(b)) => b,
                    _ => Vec::new(),
                };
                let event = match kind.as_ref().and_then(Value::as_str) {
                    Some("data") => UserEvent::KittyData(bytes),
                    Some("distro") => {
                        UserEvent::KittyDistro(String::from_utf8_lossy(&bytes).into_owned())
                    }
                    _ => return log::debug!("kitty: unrecognized notification"),
                };
                let _ = self.proxy.send_event(event);
            }
            "nvright_wheel" => {
                let n = |i: usize| args.get(i).and_then(Value::as_u64).unwrap_or(0) as f64;
                let _ = self.proxy.send_event(UserEvent::WheelLines(n(0), n(1)));
            }
            "nvim_error_event" => log::error!("nvim: {args:?}"),
            _ => log::debug!("ignored notification: {name} {args:?}"),
        }
    }
}
