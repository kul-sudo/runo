#![feature(allocator_api)]

mod constants;

use constants::*;

use crossterm::{
    cursor::{MoveTo, SetCursorStyle},
    event::{read, Event, KeyCode, KeyModifiers},
    style::{Color, Print, PrintStyledContent, Stylize},
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use std::alloc::Global;
use std::io::{stdout, Stdout, Write};

#[derive(Debug)]
enum Mode {
    Normal,
    Insert,
}

enum Actions {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    NewLine,
    Backspace,
    ModeToNormal,
    ModeToInsert,
    AddChar(char),
    Tab,
    DeleteChar,
    Exit,
}

struct Buffer {
    lines: Vec<Vec<char>>,
}

impl Buffer {
    fn insert(&mut self, x: usize, y: usize, char: char) {
        if char != '\n' {
            if self.lines.get_mut(y).is_none() {
                self.lines.resize(y + 1, Vec::new_in(Global))
            }

            let line = self.lines.get_mut(y).unwrap();
            if line.get_mut(x).is_none() {
                line.resize(x + 1, char::default())
            }
            self.lines.get_mut(y).unwrap().insert(x, char);
        }
    }

    fn remove(&mut self, x: usize, y: usize) {
        let line = self.lines.get(y);
        if line.is_some() && line.unwrap().get(x).is_some() {
            self.lines.get_mut(y).unwrap().remove(x);
        }
    }
}

struct Editor {
    /// Cursor X.
    cx: usize,

    /// Cursor Y.
    cy: usize,

    mode: Mode,
    stdout: Stdout,
    debug_text: String,
    size: (u16, u16),
    buffer: Buffer,
    first_print_x: usize,
}

impl Editor {
    fn draw(&mut self) {
        self.status_line();
        for (y, line) in self.buffer.lines.iter().enumerate() {
            for (x, char) in ({
                // TODO: // Shift the lines to the left when the current line is longer than the limit
                // if self.buffer.len(y) > self.first_print_x {
                //     line[self.first_print_x..].to_vec()
                // } else {
                //     Vec::new()
                // }

                line
            })
            .iter()
            .enumerate()
            {
                _ = self.stdout.execute(MoveTo(x as u16, y as u16));
                _ = self.stdout.execute(Print(char));
            }
        }

        _ = self.stdout.execute(match self.mode {
            Mode::Normal => SetCursorStyle::SteadyBlock,
            Mode::Insert => SetCursorStyle::DefaultUserShape,
        });

        // self.cx = match self.buffer.lines.get(self.cy) {
        //     Some(line) if !line.is_empty() => {
        //         // self.debug_text = format!("{:?} {:?}", self.buffer.cx, line);
        //         self.buffer.cx
        //             + ((TAB_SPACES - 1)
        //                 * line[..=self.buffer.cx]
        //                     .iter()
        //                     .filter(|x| **x == '\t')
        //                     .count())
        //     }
        //     _ => 0,
        // };

        _ = self.stdout.execute(MoveTo(
            // TODO: (self.cx.saturating_sub(self.first_print_x)) as u16,
            self.cx as u16,
            self.cy as u16,
        ));
        _ = self.stdout.flush();
    }

    pub fn status_line(&mut self) {
        _ = self.stdout.execute(MoveTo(0, self.size.1));

        _ = self
            .stdout
            .execute(PrintStyledContent(
                format!("{:?}", self.mode)
                    .to_uppercase()
                    .with(Color::Black)
                    .bold()
                    .on_cyan(),
            ))
            .unwrap()
            .execute(PrintStyledContent(
                format!(" {:?}", [self.cx, self.cy])
                    .to_uppercase()
                    .with(Color::Black)
                    .bold()
                    .on_cyan(),
            ))
            .unwrap()
            // .queue(PrintStyledContent(
            //     format!(" {:?}", {
            //         let line = self.buffer.lines.get(self.cy);
            //
            //         if line.is_some() {
            //             let char = line.unwrap().get(self.cx.saturating_sub(1));
            //
            //             if char.is_some() {
            //                 char.unwrap().to_string()
            //             } else {
            //                 "".to_string()
            //             }
            //         } else {
            //             "".to_string()
            //         }
            //     })
            //     .to_uppercase()
            //     .with(Color::Black)
            //     .bold()
            //     .on_cyan(),
            // ))
            // .unwrap()
            .execute(PrintStyledContent(
                self.debug_text.clone().with(Color::Black).bold().on_cyan(),
            ));
    }

    fn work(&mut self) {
        loop {
            // self.draw();
            //
            if let Some(action) = self.handle_event(read().unwrap()) {
                match action {
                    Actions::Exit => break,
                    Actions::MoveUp => {
                        self.cy = self.cy.saturating_sub(1);
                        let previous_line = self.buffer.lines.get(self.cy.saturating_sub(1));
                        if previous_line.is_some() {
                            self.cx = previous_line.unwrap().len();
                            self.cy = self.cy.saturating_sub(1)
                        }
                    }
                    Actions::MoveDown => {
                        let next_line = self.buffer.lines.get(self.cy + 1);
                        if next_line.is_some() {
                            self.cx = next_line.unwrap().len();
                            self.cy += 1;
                        }
                    }
                    Actions::MoveLeft => {
                        let line = self.buffer.lines.get(self.cy).unwrap();
                        let char = line.get(self.cx.saturating_sub(1));
                        if char.is_some() {
                            self.cx = self.cx.saturating_sub(1)
                        }
                    }
                    Actions::MoveRight => {
                        let line = self.buffer.lines.get(self.cy).unwrap();
                        let char = line.get(self.cx + 1);
                        if char.is_some() {
                            self.cx += 1
                        }
                    }
                    Actions::NewLine => {
                        self.buffer.insert(self.cx, self.cy, '\n');
                        self.cy += 1;
                        self.cx = 0;
                    }
                    Actions::Backspace => {
                        if self.cx > 0 {
                            self.buffer.remove(self.cx - 1, self.cy);
                            self.cx -= 1;
                        } else {
                            self.cy = self.cy.saturating_sub(1);
                        }
                    }
                    Actions::ModeToNormal => {
                        self.mode = Mode::Normal;
                    }
                    Actions::ModeToInsert => {
                        self.mode = Mode::Insert;
                    }
                    Actions::AddChar(char) => {
                        self.buffer.insert(self.cx, self.cy, char);
                        self.cx += 1;
                    }
                    Actions::Tab => {
                        for _ in 0..TAB_SPACES {
                            self.buffer.insert(self.cx, self.cy, ' ');
                            self.cx += 4;
                        }
                    }
                    Actions::DeleteChar => {
                        self.buffer.remove(self.cx, self.cy);
                    }
                };

                _ = self.stdout.execute(Clear(ClearType::All));
                self.draw();
            };
        }
    }

    fn handle_event(&mut self, event: Event) -> Option<Actions> {
        match event {
            Event::Key(event) => {
                match event.code {
                    KeyCode::Up => Some(Actions::MoveUp),
                    KeyCode::Down => Some(Actions::MoveDown),
                    KeyCode::Left => Some(Actions::MoveLeft),
                    KeyCode::Right => Some(Actions::MoveRight),
                    _ => {
                        match self.mode {
                            // Handling events for the normal mode
                            Mode::Normal => match event.code {
                                KeyCode::Char('q') => Some(Actions::Exit),
                                KeyCode::Char('i') | KeyCode::Char('i')
                                    if event.modifiers.contains(KeyModifiers::ALT) =>
                                {
                                    Some(Actions::ModeToInsert)
                                }
                                KeyCode::Char('d') | KeyCode::Delete => Some(Actions::DeleteChar),
                                _ => None,
                            },

                            // Handling events for the insert mode
                            Mode::Insert => match event.code {
                                KeyCode::Char('i')
                                    if event.modifiers.contains(KeyModifiers::ALT) =>
                                {
                                    Some(Actions::ModeToNormal)
                                }
                                KeyCode::Backspace => Some(Actions::Backspace),
                                KeyCode::Enter => Some(Actions::NewLine),
                                KeyCode::Char(char) => Some(Actions::AddChar(char)),
                                KeyCode::Tab => Some(Actions::Tab),
                                _ => None,
                            },
                        }
                    }
                }
            }
            Event::Resize(x, y) => {
                self.size = (x, y);
                None
            }
            _ => None,
        }
    }

    fn run(&mut self) {
        _ = enable_raw_mode();
        _ = self.stdout.execute(EnterAlternateScreen);
        _ = self.stdout.execute(Clear(ClearType::All));
    }

    fn drop(mut self) {
        _ = self.stdout.execute(LeaveAlternateScreen);
        _ = disable_raw_mode();
    }
}

fn main() {
    let mut editor = Editor {
        cx: 0,
        cy: 0,
        mode: Mode::Normal,
        stdout: stdout(),
        size: size().unwrap(),
        debug_text: String::new(),
        buffer: Buffer {
            lines: {
                let mut vector = Vec::new();
                vector.resize(1, Vec::new_in(Global));

                vector
            },
        },
        first_print_x: 0,
    };

    editor.run();

    editor.draw();
    editor.work();
    editor.drop();
}
