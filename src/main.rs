#![feature(allocator_api)]

use crossterm::{
    cursor::{self, MoveTo, SetCursorStyle},
    event::{read, Event, KeyCode},
    style::{Color, Print, PrintStyledContent, Stylize},
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand, QueueableCommand,
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
    DeleteChar,
    Exit,
}

struct Buffer {
    lines: Vec<Vec<char>>,
}

impl Buffer {
    fn insert(&mut self, x: usize, y: usize, char: char) {
        if self.lines.get_mut(y).is_none() {
            self.lines.resize(y + 1, Vec::new_in(Global))
        }

        let line = self.lines.get_mut(y).unwrap();
        if line.get_mut(x).is_none() {
            line.resize(x + 1, char::default())
        }
        self.lines.get_mut(y).unwrap().insert(x, char);
    }

    fn remove(&mut self, x: usize, y: usize) {
        let line = self.lines.get(y);
        if line.is_some() && line.unwrap().get(x).is_some() {
            self.lines.get_mut(y).unwrap().remove(x);
        }
    }

    // fn max_line_len(&self) -> usize {
    //     self.lines
    //         .iter()
    //         .max_by_key(|line| line.len())
    //         .unwrap()
    //         .len()
    // }
}

struct Editor {
    cx: usize,
    cy: usize,
    mode: Mode,
    stdout: Stdout,
    size: (u16, u16),
    buffer: Buffer,
}

impl Editor {
    fn draw(&mut self) {
        self.status_line();
        for (y, line) in self.buffer.lines.iter().enumerate() {
            for (x, char) in line.iter().enumerate() {
                _ = self.stdout.queue(MoveTo(x as u16, y as u16));
                _ = self.stdout.queue(Print(char));
            }
        }

        _ = self.stdout.queue(match self.mode {
            Mode::Normal => SetCursorStyle::SteadyBlock,
            Mode::Insert => SetCursorStyle::DefaultUserShape,
        });
        _ = self.stdout.queue(MoveTo(self.cx as u16, self.cy as u16));
        _ = self.stdout.flush();
    }

    pub fn status_line(&mut self) {
        _ = self.stdout.queue(MoveTo(0, self.size.1));

        _ = self
            .stdout
            .queue(PrintStyledContent(
                format!("{:?}", self.mode)
                    .to_uppercase()
                    .with(Color::Black)
                    .bold()
                    .on_cyan(),
            ))
            .unwrap()
            .queue(PrintStyledContent(
                format!(" {:?}", self.buffer.lines.len())
                    .to_uppercase()
                    .with(Color::Black)
                    .bold()
                    .on_cyan(),
            ))
            .unwrap()
            .queue(PrintStyledContent(
                format!(" {:?}", [self.cx, self.cy])
                    .to_uppercase()
                    .with(Color::Black)
                    .bold()
                    .on_cyan(),
            ))
            .unwrap()
            .queue(PrintStyledContent(
                format!(
                    " {:?}%",
                    (self.cy as f64 / self.buffer.lines.len() as f64) * 100_f64
                )
                .to_uppercase()
                .with(Color::Black)
                .bold()
                .on_cyan(),
            ));
    }

    fn work(&mut self) {
        loop {
            self.draw();

            if let Some(action) = self.handle_event(read().unwrap()) {
                match action {
                    Actions::Exit => break,
                    Actions::MoveUp => {
                        self.cy = self.cy.saturating_sub(1);
                        self.cx = self.buffer.lines.get(self.cy).unwrap().len();
                        _ = self.stdout.execute(Clear(ClearType::All));
                    }
                    Actions::MoveDown => {
                        let next_line = self.buffer.lines.get(self.cy + 1);
                        if next_line.is_some() {
                            self.cx = next_line.unwrap().len();
                            self.cy += 1;
                        }
                        _ = self.stdout.execute(Clear(ClearType::All));
                    }
                    Actions::MoveLeft => {
                        self.cx = self.cx.saturating_sub(1);
                        _ = self.stdout.execute(Clear(ClearType::All));
                    }
                    Actions::MoveRight => {
                        let line = self.buffer.lines.get(self.cy);
                        if line.is_some() && line.unwrap().len() > self.cx + 1 {
                            self.cx += 1
                        }
                        _ = self.stdout.execute(Clear(ClearType::All));
                    }
                    Actions::NewLine => {
                        // if self.buffer.lines.get(self.cy).unwrap().len() == self.cx {
                        self.buffer.insert(self.cx, self.cy, '\n');
                        self.cy += 1;
                        self.cx = 0;
                        // };
                        _ = self.stdout.execute(Clear(ClearType::All));
                    }
                    Actions::Backspace => {
                        if self.cx > 0 {
                            self.buffer.remove(self.cx - 1, self.cy);
                            self.cx -= 1;
                        } else {
                            self.cy = self.cy.saturating_sub(1);
                        }
                        _ = self.stdout.execute(Clear(ClearType::All));
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
                        _ = self.stdout.execute(Clear(ClearType::All));
                    }
                    Actions::DeleteChar => {
                        self.buffer.remove(self.cx, self.cy);
                        _ = self.stdout.execute(Clear(ClearType::All));
                    }
                };
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
                                KeyCode::Char('i') => Some(Actions::ModeToInsert),
                                KeyCode::Char('d') => Some(Actions::DeleteChar),
                                _ => None,
                            },

                            // Handling events for the insert mode
                            Mode::Insert => match event.code {
                                KeyCode::Esc => Some(Actions::ModeToNormal),
                                KeyCode::Backspace => Some(Actions::Backspace),
                                KeyCode::Enter => Some(Actions::NewLine),
                                KeyCode::Char(char) => Some(Actions::AddChar(char)),
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
        buffer: Buffer {
            lines: {
                let mut vector = Vec::new();
                vector.resize(1, Vec::new_in(Global));

                vector
            },
        },
    };

    editor.run();
    editor.work();
    editor.drop();
}
