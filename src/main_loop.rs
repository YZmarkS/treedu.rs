use crate::logic::*;

use ratatui::{DefaultTerminal};
use std::mem::replace;


impl Store {
    pub fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| frame.render_widget(&mut *self, frame.area()))?;

            let old_status = replace(&mut self.status, State::Swapping);

            let new_status: State = self.delta(old_status);

            match new_status {
                State::Exit => {
                    break;
                }
                other => {
                    self.status = other;
                }
            }
        }
        Ok(())
    }
}
