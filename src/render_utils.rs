use ratatui::{
    layout::{Constraint, Flex, Layout, Spacing},
    prelude::*,
    symbols::merge::MergeStrategy,
    widgets::{Block, Paragraph},
};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{inode::*, layout::*};
use crate::{logic::*, squarify::HEIGHT_TO_WIDTH_RATIO};

// https://en.wikipedia.org/wiki/Kilobyte
const SI_STEP_SIZE: u128 = 1000;
const IEC_STEP_SIZE: u128 = 1024;

const SELECTED_STYLE: Style = Style::new().fg(Color::White).bg(Color::DarkGray);
const UNSELECTED_STYLE: Style = Style::new().fg(Color::White).bg(Color::Black);

impl State {
    pub fn render_directory_path_block(&self, buf: &mut Buffer) {
        let DrawAreas {
            directory_path_rect,
            ..
        } = self.draw_areas;
        let directory_path = self
            .now
            .directory
            .borrow()
            .full_path_with_trailing_separator();

        Paragraph::new(directory_path)
            .block(Block::bordered().merge_borders(MergeStrategy::Exact))
            .render(directory_path_rect, buf);
    }

    pub fn render_help_block(&self, buf: &mut Buffer) {
        let DrawAreas { help_rect, .. } = self.draw_areas;
        Paragraph::new("Arrows: Select    Enter: Open selected    Backspace/Esc: Open Parent    r: Refresh    d: Delete Selected    q: Quit")
	    .block(Block::bordered())
	    .render(help_rect, buf);
    }

    pub fn render_entry_name_block(&self, full_name: &str, buf: &mut Buffer) {
        let DrawAreas {
            entry_name_rect, ..
        } = self.draw_areas;
        Paragraph::new(full_name)
            .block(Block::bordered().merge_borders(MergeStrategy::Exact))
            .render(entry_name_rect, buf);
    }

    pub fn render_blank_entry_name_block(&self, buf: &mut Buffer) {
        self.render_entry_name_block("", buf);
    }

    pub fn render_entry_size_block(&self, size_string: &str, buf: &mut Buffer) {
        let DrawAreas {
            entry_size_rect, ..
        } = self.draw_areas;
        Paragraph::new(size_string)
            .block(Block::bordered().merge_borders(MergeStrategy::Exact))
            .render(entry_size_rect, buf);
    }

    pub fn render_blank_entry_size_block(&self, buf: &mut Buffer) {
        self.render_entry_size_block("", buf);
    }

    pub fn render_blank_canvas_block(&self, buf: &mut Buffer) {
        let DrawAreas { canvas, .. } = self.draw_areas;
        Block::bordered().render(canvas, buf);
    }

    pub fn render_tiles(&self, tiles: &Vec<Tile>, selected: usize, buf: &mut Buffer) {
        for i in 0..tiles.len() {
            let tile = &tiles[i];

            let text = Text::from(vec![
                Line::from(tile.formatted_name()),
                Line::from(tile.usage_string()),
            ]);

            let outer_block = Block::bordered().merge_borders(MergeStrategy::Exact);
            let inner_block = Paragraph::new(text).style(if i == selected {
                SELECTED_STYLE
            } else {
                UNSELECTED_STYLE
            });
            let inner_area = outer_block.inner(tile.area);

            outer_block.render(tile.area, buf);
            inner_block.render(inner_area, buf);
        }
    }

    pub fn render_scanning_modal(&self, buf: &mut Buffer) {
        let DrawAreas { modal_rect, .. } = self.draw_areas;
        let dots = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            / 250
            % 4;
        let scanning_string = format!("Scanning{}", ".".repeat(dots as usize));
        let modal_text_area = modal_rect.centered(Constraint::Max(12), Constraint::Max(1));

        Block::bordered().render(modal_rect, buf);
        Paragraph::new(scanning_string).render(modal_text_area, buf);
    }

    pub fn render_deletion_confirmation_modal(
        &self,
        full_name: &str,
        choice: DeleteChoice,
        buf: &mut Buffer,
    ) {
        let DrawAreas { modal_rect, .. } = self.draw_areas;

        let modal_block = Block::bordered();
        let modal_inner_rect = modal_block.inner(modal_rect);
        let [message_rect, selections_rect] =
            Layout::vertical(vec![Constraint::Fill(1); 2]).areas(modal_inner_rect);
        let [yes_rect, no_rect] =
            Layout::horizontal(vec![Constraint::Fill(1); 2]).areas(selections_rect);
        let (yes_style, no_style) = match choice {
            DeleteChoice::Yes => (SELECTED_STYLE, UNSELECTED_STYLE),
            DeleteChoice::No => (UNSELECTED_STYLE, SELECTED_STYLE),
        };
        Paragraph::new(format!(
            "Are you sure you want to delete {:?}? This cannot be reverted.",
            full_name
        ))
            .render(message_rect, buf);
	Paragraph::new("Yes").style(yes_style).render(yes_rect, buf);
	Paragraph::new("No").style(no_style).render(no_rect, buf);
    }

    pub fn render_deleting_modal(&self, buf: &mut Buffer) {
	let DrawAreas { modal_rect, .. } = self.draw_areas;
        let dots = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            / 250
            % 4;
        let deleting_string = format!("Deleting{}", ".".repeat(dots as usize));
        let modal_text_area = modal_rect.centered(Constraint::Max(12), Constraint::Max(1));

        Block::bordered().render(modal_rect, buf);
        Paragraph::new(deleting_string).render(modal_text_area, buf);
    }

    pub fn render_delete_failure_modal(&self, buf: &mut Buffer) {
	let DrawAreas { modal_rect, .. } = self.draw_areas;
	let failure_string = format!("Failed to delete {}", "placeholder");
	let modal_text_area = modal_rect.centered(Constraint::Max(50), Constraint::Max(1));
	Block::bordered().render(modal_rect, buf);
	Paragraph::new(failure_string).render(modal_text_area, buf);
    }
}

pub fn area_to_draw_areas(area: Rect) -> DrawAreas {
    let [info_rect, canvas, help_rect] = Layout::vertical(vec![
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .areas(area);

    let [directory_path_rect, entry_name_rect, entry_size_rect] =
        Layout::horizontal(vec![Constraint::Fill(1); 3])
            .spacing(Spacing::Overlap(1))
            .areas(info_rect);

    let [_, mid_col, _] = Layout::new(
        Direction::Horizontal,
        vec![
            Constraint::Length(2 * HEIGHT_TO_WIDTH_RATIO),
            Constraint::Fill(1),
            Constraint::Length(2 * HEIGHT_TO_WIDTH_RATIO),
        ],
    )
    .flex(Flex::Center)
    .areas(area);
    let [_, modal_rect, _] = Layout::new(
        Direction::Vertical,
        vec![
            Constraint::Length(2),
            Constraint::Max(9),
            Constraint::Length(2),
        ],
    )
    .flex(Flex::Center)
    .areas(mid_col);

    DrawAreas {
        directory_path_rect,
        entry_name_rect,
        entry_size_rect,
        canvas,
        help_rect,
        modal_rect,
    }
}

fn human_readable_usage(usage: SizeUnit) -> String {
    let mut unit_size = 1;
    while usage / (unit_size * IEC_STEP_SIZE) != 0 {
        unit_size *= IEC_STEP_SIZE;
    }

    let value = usage / unit_size;
    let unit = if unit_size == IEC_STEP_SIZE.pow(0) {
        String::from("B")
    } else if unit_size == IEC_STEP_SIZE.pow(1) {
        String::from("KiB")
    } else if unit_size == IEC_STEP_SIZE.pow(2) {
        String::from("MiB")
    } else if unit_size == IEC_STEP_SIZE.pow(3) {
        String::from("GiB")
    } else if unit_size == IEC_STEP_SIZE.pow(4) {
        String::from("TiB")
    } else if unit_size == IEC_STEP_SIZE.pow(5) {
        String::from("PiB")
    } else if unit_size == IEC_STEP_SIZE.pow(6) {
        String::from("EiB")
    } else if unit_size == IEC_STEP_SIZE.pow(7) {
        String::from("ZiB")
    } else if unit_size == IEC_STEP_SIZE.pow(8) {
        String::from("YiB")
    } else if unit_size == IEC_STEP_SIZE.pow(9) {
        String::from("RiB")
    } else if unit_size == IEC_STEP_SIZE.pow(10) {
        String::from("QiB")
    } else {
        String::from("???")
    };

    format!("{value} {unit}")
}

impl TileData {
    pub fn full_name(&self) -> String {
        match self {
            TileData::Inode { inode } => inode.borrow().base_name_formatted_string(),
            TileData::Other { .. } => String::from("More Entries in Directory..."),
            TileData::Empty => String::from("Empty Directory"),
        }
    }

    pub fn usage_string(&self) -> String {
        match self {
            TileData::Inode { inode } => human_readable_usage(inode.clone().borrow().usage),
            TileData::Other { usage, .. } => human_readable_usage(*usage),
            TileData::Empty => human_readable_usage(0),
        }
    }
}

impl Tile {
    pub fn full_name(&self) -> String {
        self.data.full_name()
    }
    pub fn usage_string(&self) -> String {
        self.data.usage_string()
    }
    pub fn formatted_name(&self) -> String {
        let internal_width: usize = (self.area.width - 2) as usize;
        match &self.data {
            TileData::Inode { inode } => {
                let inode = inode.borrow();
                let base_name_pathbuf = inode.base_name_pathbuf();
                let base_name_string: String = base_name_pathbuf.to_string_lossy().into();

                /* File or Link File Name

                Attempts to include the starting character and the full extension.
                If not possible, shrink the entire file name.
                 */
                if inode.is_reg() || inode.is_sym() {
                    if base_name_string.len() <= internal_width as usize {
                        base_name_string
                    } else {
                        let extension = base_name_pathbuf.extension().and_then(|ext| ext.to_str());
                        // If there is room for first char, a dot, and the extension
                        if extension
                            .clone()
                            .is_some_and(|ext| 1 + "[..]".len() + 1 + ext.len() <= internal_width)
                        {
                            if let Some(extension) = extension {
                                let stem = base_name_pathbuf.file_stem().unwrap().to_str().unwrap();
                                let room = internal_width - ("[..]".len() + 1 + extension.len());
                                let back_len = room / 2;
                                let front_len = room - back_len;
                                format!(
                                    "{}[..]{}.{}",
                                    stem[..front_len].to_string(),
                                    stem[stem.len() - back_len..].to_string(),
                                    extension
                                )
                            } else {
                                panic!(
                                    "Render - cell_base_name: Cannot find extension for {:?}",
                                    base_name_pathbuf
                                )
                            }
                        } else {
                            let room = internal_width - "[..]".len();
                            let back_len = room / 2;
                            let front_len = room - back_len;
                            format!(
                                "{}[..]{}",
                                base_name_string[..front_len].to_string(),
                                base_name_string[base_name_string.len() - back_len..].to_string()
                            )
                        }
                    }
                }
                /* Directory Name

                Only attempt to include at the first and last character.
                 */
                else if inode.is_dir() {
                    if base_name_string.len() <= (internal_width - 1) as usize {
                        base_name_pathbuf.join("").to_string_lossy().into_owned()
                    } else {
                        let room = internal_width - ("[..]".len() + 1);
                        let back_len = room / 2;
                        let front_len = room - back_len;
                        format!(
                            "{}[..]{}{}",
                            base_name_string[..front_len].to_string(),
                            base_name_string[base_name_string.len() - back_len..].to_string(),
                            std::path::MAIN_SEPARATOR
                        )
                    }
                } else {
                    panic!("Render - cell_base_name: Unknown inode type")
                }
            }
            TileData::Other { .. } => String::from("[...]"),
            TileData::Empty => String::from("There's nothing more..."),
        }
    }
}
