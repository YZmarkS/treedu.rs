use crate::logic::*;

use ratatui::{DefaultTerminal};
use std::mem::replace;


impl State {
    pub fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| frame.render_widget(&mut *self, frame.area()))?;

            let old_status = replace(&mut self.status, Status::Swapping);

            let new_status: Status = self.delta(old_status);

            match new_status {
                Status::Exit => {
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
