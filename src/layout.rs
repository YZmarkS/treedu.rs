use ratatui::layout::*;
use std::{cell::RefCell, cmp, collections::HashMap, rc::Rc};

use crate::inode::*;
use crate::squarify::*;

pub fn squarify_to_rects(squarify: &Squarify, canvas: Rect) -> Vec<Rect> {
    let mut list = squarify_to_rects_aux(squarify, canvas);
    list.reverse();
    list
}

fn squarify_to_rects_aux(squarify: &Squarify, canvas: Rect) -> Vec<Rect> {
    match &(squarify.detail) {
        LayoutDetail::Whole => {
            let mut rects = Vec::new();
            if 0 < squarify.width && 0 < squarify.height {
                rects.push(canvas);
            }
            rects
        }
        LayoutDetail::Split(detail) => {
            let is_landscape = is_landscape(squarify.width, squarify.height);
            let breadth = detail.breadth;
            let other_breadth = if is_landscape {
                squarify.width - breadth
            } else {
                squarify.height - breadth
            };

            /* [+ 1]s

            There are many [+ 1] in the calculation. It is a result of how we
            interpret the value of width and height.
            */
            let split_constraints = vec![
                Constraint::Length(detail.breadth + 1),
                Constraint::Length(other_breadth + 1),
            ];
            let stack_constrains_iter = detail
                .spans
                .iter()
                .map(|span| Constraint::Length(span.to_owned() + 1));

            use ratatui::prelude::Direction::*;
            let split_rects = Layout::default()
                .direction(if is_landscape { Horizontal } else { Vertical })
                .spacing(Spacing::Overlap(1))
                .constraints(split_constraints)
                .split(canvas);

            let stack = split_rects[0];
            let other = split_rects[1];

            let stack_rects = Layout::default()
                .direction(if is_landscape { Vertical } else { Horizontal })
                .spacing(Spacing::Overlap(1))
                .constraints(stack_constrains_iter)
                .split(stack);

            let mut sub_rects = squarify_to_rects_aux(&detail.other, other);
            for stack_rect in stack_rects.iter().rev() {
                sub_rects.push(*stack_rect);
            }

            sub_rects
        }
    }
}

fn x_interval(rect: Rect) -> (Measure, Measure) {
    (rect.x, rect.x + rect.width)
}

fn y_interval(rect: Rect) -> (Measure, Measure) {
    (rect.y, rect.y + rect.height)
}

fn calc_overlap((low1, high1): (Measure, Measure), (low2, high2): (Measure, Measure)) -> i32 {
    let max_low = cmp::max(low1, low2);
    let min_high = cmp::min(high1, high2);
    i32::from(min_high) - i32::from(max_low)
}

fn calc_offset((low1, high1): (Measure, Measure), (low2, high2): (Measure, Measure)) -> i32 {
    let mid1 = i32::from((high1 - low1) / 2 + low1);
    let mid2 = i32::from((high2 - low2) / 2 + low2);
    (mid2 - mid1).abs()
}

#[derive(Debug)]
pub struct Tile {
    pub area: Rect,
    pub data: TileData,
}

#[derive(Debug)]
pub enum TileData {
    Inode {
        inode: Rc<RefCell<Inode>>,
    },
    Other {
        starting_index: usize,
        usage: SizeUnit,
    },
    Empty,
}

pub type TileIndex = usize;

#[derive(Debug)]
pub struct Neighbours {
    pub top: Option<TileIndex>,
    pub bottom: Option<TileIndex>,
    pub left: Option<TileIndex>,
    pub right: Option<TileIndex>,
}

pub fn make_tiles(
    rects: &Vec<Rect>,
    inodes: &Vec<Rc<RefCell<Inode>>>,
    starting_index: usize,
) -> Vec<Tile> {
    assert!(0 < rects.len());
    use TileData::*;
    let mut tiles: Vec<Tile> = Vec::new();
    if inodes.len() <= starting_index {
        tiles.push(Tile {
            area: rects[0],
            data: Empty,
        });
        return tiles;
    }

    for i in 0..(rects.len() - 1) {
        tiles.push(Tile {
            area: rects[i].to_owned(),
            data: Inode {
                inode: inodes[starting_index + i].clone(),
            },
        });
    }
    tiles.push(Tile {
        area: rects[rects.len() - 1].to_owned(),
        data: if starting_index + rects.len() == inodes.len() {
            Inode {
                inode: inodes[starting_index + rects.len() - 1].clone(),
            }
        } else {
            Other {
                starting_index: starting_index + rects.len() - 1,
                usage: inodes[starting_index + rects.len() - 1..]
                    .iter()
                    .map(|inode| inode.borrow().usage)
                    .sum(),
            }
        },
    });
    tiles
}

type TileIndexByCoord = HashMap<Measure, Vec<TileIndex>>;

fn push_by_key<K: Eq + core::hash::Hash, V>(map: &mut HashMap<K, Vec<V>>, key: K, value: V) {
    if map.contains_key(&key) {
        map.get_mut(&key).unwrap().push(value);
    } else {
        map.insert(key, vec![value]);
    }
}

enum Ends {
    TopBottom,
    LeftRight,
}

fn group_by_endpoint(tiles: &Vec<Tile>, ends_to_group_by: Ends) -> (TileIndexByCoord, TileIndexByCoord) {
    let (mut by_low, mut by_high) = (HashMap::new(), HashMap::new());

    for i in 0..(tiles.len()) {
        let (low, high) = match ends_to_group_by {
            Ends::TopBottom => y_interval(tiles[i].area),
            Ends::LeftRight => x_interval(tiles[i].area),
        };
        push_by_key(&mut by_low, low, i);
        push_by_key(&mut by_high, high, i);
    }

    (by_low, by_high)
}

fn find_most_overlapping(
    tiles: &Vec<Tile>,
    candidates: Option<&Vec<TileIndex>>,
    tile: &Tile,
    ends_to_compare_by: Ends,
) -> Option<TileIndex> {
    let candidates = candidates?;

    let mut best_candidate = None;
    let mut best_overlap = -1;
    let mut best_offset = i32::MAX;

    let tile_interval = match ends_to_compare_by {
        Ends::TopBottom => y_interval(tile.area),
        Ends::LeftRight => x_interval(tile.area),
    };

    for candidate in candidates {
        let candidate_interval = match ends_to_compare_by {
            Ends::TopBottom => y_interval(tiles[*candidate].area),
            Ends::LeftRight => x_interval(tiles[*candidate].area),
        };

        let overlap = calc_overlap(candidate_interval, tile_interval);
        let offset = calc_offset(candidate_interval, tile_interval);

        if best_overlap < overlap {
            best_overlap = overlap;
            best_candidate = Some(*candidate);
        } else if best_overlap == overlap && offset < best_offset {
            best_offset = offset;
            best_candidate = Some(*candidate);
        }
    }
    best_candidate
}

pub fn make_neighbours(tiles: &Vec<Tile>) -> Vec<Neighbours> {
    use Ends::*;
    let mut neighbours = Vec::new();

    let (by_top, by_bottom) = group_by_endpoint(tiles, TopBottom);
    let (by_left, by_right) = group_by_endpoint(tiles, LeftRight);

    for tile in tiles.iter() {
        let (top, bottom) = y_interval(tile.area);
        let (left, right) = x_interval(tile.area);

        let top_neighbour =
            find_most_overlapping(tiles, by_bottom.get(&(top + 1)), tile, LeftRight);
        let bottom_neighbour =
            find_most_overlapping(tiles, by_top.get(&(bottom - 1)), tile, LeftRight);
        let left_neighbour =
            find_most_overlapping(tiles, by_right.get(&(left + 1)), tile, TopBottom);
        let right_neighbour =
            find_most_overlapping(tiles, by_left.get(&(right - 1)), tile, TopBottom);

        neighbours.push(Neighbours {
            top: top_neighbour,
            bottom: bottom_neighbour,
            left: left_neighbour,
            right: right_neighbour,
        });
    }
    neighbours
}
