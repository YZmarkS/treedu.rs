use ratatui::prelude::*;
use std::mem::replace;

use crate::{logic::*, render_utils::*};

impl Widget for &mut Store {
    /* Render Function (Weird Code Ahead)

    Due to the choice of single source of truth, the [render] method also serves
    the purpose of passing the canvas size to the computation stage.
     */
    fn render(self, area: Rect, buf: &mut Buffer) {
        use State::*;

        self.draw_areas = area_to_draw_areas(area);

        match &self.status {
            Scanning { .. } => {
                self.render_directory_path_block(buf);
                self.render_blank_entry_name_block(buf);
                self.render_blank_entry_size_block(buf);
                self.render_help_block(buf);
                self.render_scanning_modal(buf);
            }
            DisplayTreemap { tiles, .. } => {
                let selected = self.now.selected;
                let selected_tile = &tiles[selected];
                self.render_directory_path_block(buf);
                self.render_entry_name_block(&selected_tile.full_name(), buf);
                self.render_entry_size_block(&selected_tile.usage_string(), buf);
                self.render_tiles(tiles, selected, buf);
                self.render_help_block(buf);
            }
            Delete { inode, state } => {
                use crate::logic::DeleteState::*;
		let full_name = inode.borrow().base_name_formatted_string();
                match state {
                    Confirmation { choice } => {
                        self.render_directory_path_block(buf);
                        self.render_entry_name_block(&full_name, buf);
                        self.render_blank_entry_size_block(buf);
                        // self.render_tiles(tiles, selected, buf);
                        self.render_help_block(buf);

                        self.render_delete_confirmation_modal(&full_name, *choice, buf);
                    }
                    InProgress { .. } => {
                        self.render_directory_path_block(buf);
                        self.render_blank_entry_name_block(buf);
                        self.render_blank_entry_size_block(buf);
                        self.render_help_block(buf);
                        self.render_delete_in_progress_modal(buf);
                    }
                    Failed => {
                        self.render_directory_path_block(buf);
                        self.render_blank_entry_name_block(buf);
                        self.render_blank_entry_size_block(buf);
                        self.render_help_block(buf);
                        self.render_delete_failure_modal(&full_name, buf);
                    }
                    _other_delete_states => {}
                }
            }
            _other_states => {}
        };
    }
}
