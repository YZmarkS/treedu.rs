use crate::{inode::*, layout::*, scanner::*, squarify::*};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::prelude::Rect;
use std::fs::remove_dir_all;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::JoinHandle;
use std::time::Duration;
use std::{cell::RefCell, sync::*, thread};

type CellIndex = usize;
type Deleted = bool;

#[derive(Clone)]
pub struct HistoryEntry {
    pub directory: Rc<RefCell<Inode>>,
    pub starting_index: usize,
    pub selected: CellIndex,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeleteChoice {
    Yes,
    No,
}

#[derive(Debug)]
pub enum Status {
    Exit,
    Swapping,
    StartScanning,
    Scanning {
        base_inode_collected: Arc<RwLock<Option<Arc<Mutex<ScannerInode>>>>>,
        scanning_thread_handle: JoinHandle<()>,
    },
    ReturnToHeadOfCurrentDirectory,
    CalcNewLayout,
    DisplayTreemap {
        tiles: Vec<Tile>,
        neighbours: Vec<Neighbours>,
    },
    DeletionConfirmation {
        choice: DeleteChoice,
    },
    DeleteSelected,
    DeletingSelected {
        deleted_p: Arc<RwLock<std::io::Result<Deleted>>>,
        deleting_thread_handle: JoinHandle<()>,
    },
    DeletionFailed,
    DeletionCleanup,
}

pub struct DrawAreas {
    pub directory_path_rect: Rect,
    pub entry_name_rect: Rect,
    pub entry_size_rect: Rect,
    pub canvas: Rect,
    pub help_rect: Rect,
    pub modal_rect: Rect,
}

pub struct State {
    pub status: Status,
    pub root_path: PathBuf,
    pub worker_pool_size: usize,

    pub root_inode: Rc<RefCell<Inode>>,

    pub history: Vec<HistoryEntry>,
    pub now: HistoryEntry,

    pub draw_areas: DrawAreas,
}

impl State {
    pub fn delta(&mut self, status: Status) -> Status {
        use Status::*;
        match status {
            /* Scanning

            The main challenge is communicating between the main rendering
            thread and the scanning threads. If the only goal is to notify
            completion, the interface does not need to be complicated. Channel,
            mutex are both sufficient.

            [StartScanning] status prepares the interface, spawns the scanning
            threads, and transition to [Scannig] state.

            [Scanning] status repeatedly check for completion. Once scanning is
            complete, convert scanning result and transition to cleanup status.
             */
            StartScanning => {
                let base_inode_collected = Arc::new(RwLock::new(None));
                let base_inode_collected_clone = base_inode_collected.clone();

                let worker_pool_size = self.worker_pool_size;
                let base_path = self.now.directory.borrow().path.clone();

                let scanning_thread_handle = thread::spawn(move || {
                    scan_base_path(worker_pool_size, base_path, base_inode_collected_clone);
                });

                Scanning {
                    base_inode_collected,
                    scanning_thread_handle,
                }
            }
            Scanning {
                base_inode_collected,
                scanning_thread_handle,
            } => {
                let maybe_base_inode_collected = base_inode_collected.read().unwrap().to_owned();
                let new_state = match maybe_base_inode_collected {
                    None => {
                        if let Ok(true) = crossterm::event::poll(Duration::from_millis(20)) {
                            let event = crossterm::event::read().unwrap();

                            if let Event::Key(KeyEvent {
                                code: KeyCode::Char('q'),
                                ..
                            }) = event
                            {
                                Exit
                            } else {
                                Scanning {
                                    base_inode_collected,
                                    scanning_thread_handle,
                                }
                            }
                        } else {
                            Scanning {
                                base_inode_collected,
                                scanning_thread_handle,
                            }
                        }
                    }
                    Some(base_inode) => {
                        scanning_thread_handle.join().unwrap();

                        let new_base_inode =
                            Rc::new(RefCell::new(Inode::try_from(base_inode).unwrap()));
                        self.now.directory.swap(&new_base_inode);

                        ReturnToHeadOfCurrentDirectory
                    }
                };
                new_state
            }

            /* Manage History

            Refreshing directory and deleting files can cause inconsistency.
            Namely, consider we are viewing a directory with 10 elements, with
            the 10th being selected. After deleting the 10th element (whether by
            external forces or internally through app), the previously selected
            element becomes unavailable. Thus, it is crucial to recover from an
            inconsistent state after (re)scanning or file deletion.
             */
            ReturnToHeadOfCurrentDirectory => {
                let curr_directory = self.now.directory.clone();
                while self.history.last().is_some_and(|entry| {
                    entry.directory.borrow().path == curr_directory.borrow().path
                }) {
                    self.history.pop().unwrap();
                }

                self.now.selected = 0;
                self.now.starting_index = 0;

                CalcNewLayout
            }

            /* Getting terminal dimension for Layout Calculation

            We need to know the terminal size on startup and on window resize.
            While [crossterm] provides some interface for getting the terminal
            dimension, I want one source of truth, and is currently using the
             */
            CalcNewLayout => {
                let InodeDetail::Dir(children) = &(self.now.directory.borrow().detail) else {
                    panic!("CalcNewLayout: Current inode is not a directory");
                };

                let DrawAreas { canvas, .. } = self.draw_areas;

                let squarify = squarify(
                    &children[self.now.starting_index..],
                    // The minus one is very annoying because of off-by-one unit
                    canvas.width - 1,
                    canvas.height - 1,
                );
                let rects = squarify_to_rects(&squarify, canvas);
                let tiles = make_tiles(&rects, children, self.now.starting_index);
                let neighbours = make_neighbours(&tiles);

                DisplayTreemap { tiles, neighbours }
            }

            /* Delete Selected

            Simple business logic, but many house keeping
             */
            status @ (DeletionConfirmation { .. }
            | DeleteSelected
            | DeletingSelected { .. }
            | DeletionFailed
            | DeletionCleanup) => self.delta_delete(status),

            /* Displaying and Waiting for User Input

            If there are no computation to be done, then the application is
            waiting for some terminal event to happen.

            The code is ugly but necessary.
             */
            DisplayTreemap { tiles: cells, neighbours } => {
                let event = crossterm::event::read().unwrap();
                match event {
                    Event::Resize(_, _) => CalcNewLayout,
                    /* Key Event Duplication

                    Known [crossterm] issue. The default key press event is [Press].
                    - Official documentation: https://docs.rs/crossterm/latest/crossterm/event/struct.KeyEvent.html#structfield.kind
                    - Double key input on Windows: https://github.com/crossterm-rs/crossterm/issues/772
                    - Proposal to make release event opt-in: https://github.com/crossterm-rs/crossterm/pull/778
                     */
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        match &key_event.code {
                            KeyCode::Up => {
                                self.up(&neighbours);
                                DisplayTreemap { tiles: cells, neighbours }
                            }
                            KeyCode::Down => {
                                self.down(&neighbours);
                                DisplayTreemap { tiles: cells, neighbours }
                            }
                            KeyCode::Left => {
                                self.left(&neighbours);
                                DisplayTreemap { tiles: cells, neighbours }
                            }
                            KeyCode::Right => {
                                self.right(&neighbours);
                                DisplayTreemap { tiles: cells, neighbours }
                            }
                            KeyCode::Esc => {
                                self.go_out();
                                CalcNewLayout
                            }
                            KeyCode::Backspace => {
                                self.go_out();
                                CalcNewLayout
                            }
                            KeyCode::Enter => {
                                self.go_in(&cells);
                                CalcNewLayout
                            }
                            KeyCode::Char('r') => StartScanning,
                            // KeyCode::Char('d') => ConfirmDeleteSelected,
                            KeyCode::Char('q') => Exit,
                            _ => DisplayTreemap { tiles: cells, neighbours },
                        }
                    }
                    _ => DisplayTreemap { tiles: cells, neighbours },
                }
            }

            /* Intermediate States

            The following states should only be used as a mean to transition to
            other states.
             */
            Exit => Exit,
            Swapping => {
                panic!(
                    "Swapping: Status should only be used as a placeholder during delta transition"
                )
            }
        }
    }

    /* Movement: Up, Down, Left, Right
     */
    pub fn up(&mut self, neighbours: &Vec<Neighbours>) {
        match neighbours[self.now.selected].top {
            None => (),
            Some(new_index) => self.now.selected = new_index,
        }
    }
    pub fn down(&mut self, neighbours: &Vec<Neighbours>) {
        match neighbours[self.now.selected].bottom {
            None => (),
            Some(new_index) => self.now.selected = new_index,
        }
    }
    pub fn left(&mut self, neighbours: &Vec<Neighbours>) {
        match neighbours[self.now.selected].left {
            None => (),
            Some(new_index) => self.now.selected = new_index,
        }
    }

    pub fn right(&mut self, neighbours: &Vec<Neighbours>) {
        match neighbours[self.now.selected].right {
            None => (),
            Some(new_index) => self.now.selected = new_index,
        }
    }

    /* Movement: Go in

    Attempts to go into the selected cell.

    There are 4 cases:
    1. The current cell represents a file. Then this is a no-op
    2. The current cell is a directory. Then we save the current state into
    history. [self.now] gets updated with new directory, [starting_index] and
    [selected] both get set to first element in the directory.
    3. The current cell represents "the tail" of the content in current directory.
    Push [self.now] into history stack as in case 2. [self.now] stays in the same
    directory, [starting_index] is updated, and [selected] resets to 0.
    4. The current cell represents nothing. The most likely case is when the
    current directory is empty. Regardless, this is a no-op.
     */
    pub fn go_in(&mut self, cells: &Vec<Tile>) {
        let cell = &cells[self.now.selected];

        match &cell.data {
            TileData::Inode { inode } => {
                let inode_rc = inode.clone();

                if let InodeDetail::Dir(_) = inode_rc.borrow().detail {
                    let now = self.now.clone();
                    let next = HistoryEntry {
                        directory: inode.clone(),
                        starting_index: 0,
                        selected: 0,
                    };
                    self.history.push(now);
                    self.now = next;
                }
            }
            TileData::Other { starting_index, .. } => {
                let now = self.now.clone();
                let next = HistoryEntry {
                    directory: self.now.directory.clone(),
                    starting_index: *starting_index,
                    selected: 0,
                };
                self.history.push(now);
                self.now = next;
            }
            TileData::Empty => {}
        }
    }

    /* Movement: Go out

    Attempt to get out one level

    Refreshing the directory or deleting a file can change the disk usage.
    We recomputing the total usage on exiting a directory to reflect the change.
    */
    pub fn go_out(&mut self) {
        if self.history.is_empty() {
            return;
        }

        self.now = self.history.pop().unwrap();

        let Inode {
            usage,
            detail: InodeDetail::Dir(children),
            ..
        } = &mut (*(self.now.directory.borrow_mut()))
        else {
            panic!("Go Out: Exited to a non directory inode")
        };
        *usage = children.iter().map(|inode| inode.borrow().usage).sum();
        children.sort_by_key(|inode| inode.borrow().usage);
        children.reverse();
    }

    fn delta_delete(&mut self, status: Status) -> Status {
        use Status::*;
        match status {
            DeletionConfirmation { choice } => {
                use crossterm::event::{
                    Event::Key,
                    KeyCode::{Enter, Left, Right},
                };
                use DeleteChoice::*;

                let event = crossterm::event::read().unwrap();
                match (event, choice) {
                    (Key(KeyEvent { code: Enter, .. }), Yes) => DeleteSelected,
                    (Key(KeyEvent { code: Enter, .. }), No) => CalcNewLayout,
                    (Key(KeyEvent { code: Left, .. }), _) => DeletionConfirmation { choice: Yes },
                    (Key(KeyEvent { code: Right, .. }), _) => DeletionConfirmation { choice: No },
                    (_, choice) => DeletionConfirmation { choice },
                }
            }
            DeleteSelected => {
                let index_in_directory = self.now.starting_index + self.now.selected;
                let current_directory = self.now.directory.borrow();
                let children = match &(current_directory.detail) {
                    InodeDetail::Dir(children) => children,
                    _ => panic!(
                        "delta_delete | DeleteSelected: Durrent \"directory\" is not a directory"
                    ),
                };
                let child = match children.get(index_in_directory) {
                    Some(child) => child,
                    None => panic!("delta_delete | DeleteSelected: Cannot find inode to delete"),
                };
                let path_to_delete = child.borrow().path.clone();

                let deleted_p = Arc::new(RwLock::new(Ok(false)));
                let deleted_p_clone = Arc::clone(&deleted_p);
                let deleting_thread_handle = thread::spawn(move || {
                    let remove_result = remove_dir_all(path_to_delete);
                    let remove_result = remove_result.map(|_| true);
                    *deleted_p_clone.write().unwrap() = remove_result;
                });

                DeletingSelected {
                    deleted_p,
                    deleting_thread_handle,
                }
            }
            DeletingSelected {
                deleted_p,
                deleting_thread_handle,
            } => {
                let status = match *deleted_p.read().unwrap() {
                    Ok(b) => Ok(b),
                    Err(_) => Err(()),
                };

                match status {
                    Ok(false) => DeletingSelected {
                        deleted_p,
                        deleting_thread_handle,
                    },
                    Ok(true) => {
                        deleting_thread_handle.join().unwrap();
                        DeletionCleanup
                    }
                    Err(_) => DeletionFailed,
                }
            }
            DeletionFailed => {
                use crossterm::event::Event::Key;
                let event = crossterm::event::read().unwrap();
                match event {
                    Key(KeyEvent { .. }) => CalcNewLayout,
                    _ => DeletionFailed,
                }
            }
            /* Clean up

            Remove the logical inode from memory.
            Modify the history to remain consistent.
             */
            DeletionCleanup => {
                let index_in_directory = self.now.starting_index + self.now.selected;
                let Inode {
                    usage,
                    detail: InodeDetail::Dir(children),
                    ..
                } = &mut *self.now.directory.borrow_mut()
                else {
                    unreachable!(
                        "delta_delete | DeletionCleanup: Durrent \"directory\" is not a directory"
                    )
                };
                children.remove(index_in_directory);
                *usage = children.iter().map(|inode| inode.borrow().usage).sum();

                /* TODO: Is modifying the history stack really necessary?

                Is the state still incoherent?
                If the state is still coherent, is it possible to enter an
                incoherent state?
                 */

                CalcNewLayout
            }
            _ => {
                unreachable!("delta_delete: Impossible state {:?}", status);
            }
        }
    }
}
