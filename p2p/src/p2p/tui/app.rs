use ratatui::{
    DefaultTerminal,
    crossterm::{self, event::KeyEvent},
};
use rootcause::Report;

use crate::p2p::{
    model::AppViewModel,
    tui::{
        event::{AppEvent, Event, EventHandler},
        ui::{
            input::{self, InputEvent},
            message,
        },
    },
};

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// p2p节点
    pub view_model: AppViewModel,
    /// Event handler.
    pub events: EventHandler,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            view_model: AppViewModel::new("myself_id".into()),
            events: EventHandler::new(),
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<(), Report> {
        while self.running {
            terminal.draw(|frame| {
                self.ui(frame);
            })?;
            match self.events.next().await? {
                Event::Tick => self.tick(),
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event)
                        if key_event.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        self.handle_key_events(key_event)?
                    }
                    _ => {}
                },
                Event::App(app_event) => match app_event {
                    // AppEvent::MsgSending(message) => todo!(),
                    // AppEvent::MsgReceiving(message) => todo!(),
                    AppEvent::Input(input_event) => match input_event {
                        InputEvent::MsgSending(message) => {
                            self.view_model.message_vm.add_msg(message.clone());
                        }
                        InputEvent::MsgReceiving(message) => {
                            self.view_model.message_vm.add_msg(message);
                        }
                        InputEvent::PeersUpdating(_hash_set) => todo!(),
                        InputEvent::ChangeStatus => {}
                    },
                    AppEvent::Logging(_log) => todo!(),
                    AppEvent::Quit => self.quit(),
                },
            }
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<(), Report> {
        input::key_event(self, &key_event);
        message::key_event(self, &key_event);
        Ok(())
    }

    /// Handles the tick event of the terminal.
    ///
    /// The tick event is where you can update the state of your application with any logic that
    /// needs to be updated at a fixed frame rate. E.g. polling a server, updating an animation.
    pub fn tick(&self) {}

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }
}

// #[cfg(test)]
// mod tests {
//     use std::net::{SocketAddr, SocketAddrV4};

//     use rootcause::Report;

//     use crate::p2p::{self, app::Args, tui::app::App};

//     #[tokio::test]
//     async fn test_main() -> Result<(), Report> {
//         let args = Args {
//             server_name: Some("localhost".into()),
//             local_addr: SocketAddr::V4(SocketAddrV4::new("127.0.0.1".parse()?, 8000)),
//             remote_addr: None,
//             ai_agent: false,
//             without_ui: false,
//         };
//         log::debug!("Args: {:?}", args);

//         let terminal = ratatui::init();
//         let result = App::new().run(terminal).await;
//         ratatui::restore();
//         result
//     }
// }
