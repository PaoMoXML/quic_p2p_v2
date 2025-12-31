use std::{collections::HashSet, sync::Arc};

use log::Level;
use tokio::sync::RwLock;

use super::model::{AppViewModel, message_view_model::Message};

pub type ArcRwlockAppvm = Arc<RwLock<AppViewModel>>;

#[derive(Debug, Clone)]
pub enum LogicEvent {
    PeerListUpdated(HashSet<String>),
    NewMessageReceived(Message),
    Log(Level, String),
}

#[macro_export]
macro_rules! async_ui_log {
    ($ctrl:expr, $lvl:expr, $($arg:tt)+) => {{
        log::log!($lvl, $($arg)+);
        {
            let mut state = $ctrl.view_model.write().await;
            state.log_vm.add_log(log_vm_log!($lvl, $($arg)+));
            state.log_vm.vertical_scroll_to_end();
        }
        $ctrl.refresh().await;
    }};
}

#[macro_export]
macro_rules! async_notify_ui_log {
    ($notifier:expr, $lvl:expr, $($arg:tt)+) => {{
        log::log!($lvl, $($arg)+);
        if let Some(notifier) = $notifier {
            if let Err(err) = notifier.send($crate::p2p::controller::LogicEvent::Log($lvl, format!($($arg)+))).await {
                log::error!("Notify ui err: [{err:#}]");
            };
        }
    }};
}
