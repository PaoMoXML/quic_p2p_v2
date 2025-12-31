use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};

use crate::p2p::model::{
    input_view_model::InputViewModel, log_view_model::LogViewModel,
    message_view_model::MessageViewModel, peer_view_model::PeerViewModel,
};

pub mod input_view_model;
pub mod log_view_model;
pub mod message_view_model;
pub mod peer_view_model;

#[derive(Clone, Debug, Default)]
pub struct AppViewModel {
    pub input_vm: InputViewModel,
    pub message_vm: MessageViewModel,
    pub peer_vm: PeerViewModel,
    pub log_vm: LogViewModel,
}

impl AppViewModel {
    pub fn new(peer_id: String) -> Self {
        Self {
            input_vm: InputViewModel::new(peer_id.clone()),
            message_vm: MessageViewModel::new(peer_id),
            ..Default::default()
        }
    }
}

pub trait ViewModelFmt {
    fn formart(&self) -> (Vec<String>, Option<String>);
}

pub trait AutoScrollableViewModel {
    type Msg: ViewModelFmt;

    fn get_inner_msgs(&self) -> &Vec<Self::Msg>;

    fn get_vertical_scroll(&self) -> Arc<AtomicUsize>;

    fn vertical_scroll_to_end(&self);

    fn get_vertical_scroll_at_end(&self) -> Arc<AtomicBool>;

    fn reset_vertical_scroll_at_end(&self);

    fn vertical_scroll_to(&self, pos: usize);
}

#[macro_export]
macro_rules! log_vm_log {
    // log!(Level::Info, "a log event")
    ($lvl:expr, $($arg:tt)+) => ({
        Log::new($lvl, format!($($arg)+))
    });
}
