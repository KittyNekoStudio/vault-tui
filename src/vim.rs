use tui_textarea::{CursorMove, Input, Key, Scrolling, TextArea};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Insert,
    Normal,
    Visual,
    Operator(char),
}

pub enum Transition {
    Nop,
    Mode(Mode),
    Pending(Input),
    Command,
}

pub struct Vim {
    pub mode: Mode,
    pending: Input,
}

impl Vim {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            pending: Input::default(),
        }
    }

    pub fn with_pending(self, pending: Input) -> Self {
        Self {
            mode: self.mode,
            pending,
        }
    }

    pub fn exec(&mut self, input: Input, textarea: &mut TextArea) -> Transition {
        if input.key == Key::Null {
            return Transition::Nop;
        }

        match self.mode {
            Mode::Normal | Mode::Visual | Mode::Operator(_) => {
                match input {
                    Input {
                        key: Key::Char('h'),
                        ..
                    } => textarea.move_cursor(CursorMove::Back),
                    Input {
                        key: Key::Char('j'),
                        ..
                    } => textarea.move_cursor(CursorMove::Down),
                    Input {
                        key: Key::Char('k'),
                        ..
                    } => textarea.move_cursor(CursorMove::Up),
                    Input {
                        key: Key::Char('l'),
                        ..
                    } => textarea.move_cursor(CursorMove::Forward),
                    Input {
                        key: Key::Char('w'),
                        ..
                    } => textarea.move_cursor(CursorMove::WordForward),
                    Input {
                        key: Key::Char('e'),
                        ctrl: false,
                        ..
                    } => {
                        textarea.move_cursor(CursorMove::WordEnd);
                        if matches!(self.mode, Mode::Operator(_)) {
                            textarea.move_cursor(CursorMove::Forward); // Include the text under the cursor
                        }
                    }
                    Input {
                        key: Key::Char('b'),
                        ctrl: false,
                        ..
                    } => textarea.move_cursor(CursorMove::WordBack),
                    Input {
                        key: Key::Char('^'),
                        ..
                    } => textarea.move_cursor(CursorMove::Head),
                    Input {
                        key: Key::Char('$'),
                        ..
                    } => textarea.move_cursor(CursorMove::End),
                    Input {
                        key: Key::Char('D'),
                        ..
                    } => {
                        textarea.delete_line_by_end();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('C'),
                        ..
                    } => {
                        textarea.delete_line_by_end();
                        textarea.cancel_selection();
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('p'),
                        ..
                    } => {
                        textarea.paste();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('u'),
                        ctrl: false,
                        ..
                    } => {
                        textarea.undo();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('r'),
                        ctrl: true,
                        ..
                    } => {
                        textarea.redo();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('x'),
                        ..
                    } => {
                        textarea.delete_next_char();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('i'),
                        ..
                    } => {
                        textarea.cancel_selection();
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('a'),
                        ..
                    } => {
                        textarea.cancel_selection();
                        textarea.move_cursor(CursorMove::Forward);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('A'),
                        ..
                    } => {
                        textarea.cancel_selection();
                        textarea.move_cursor(CursorMove::End);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('o'),
                        ..
                    } => {
                        textarea.move_cursor(CursorMove::End);
                        textarea.insert_newline();
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('O'),
                        ..
                    } => {
                        textarea.move_cursor(CursorMove::Head);
                        textarea.insert_newline();
                        textarea.move_cursor(CursorMove::Up);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('I'),
                        ..
                    } => {
                        textarea.cancel_selection();
                        textarea.move_cursor(CursorMove::Head);
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char('e'),
                        ctrl: true,
                        ..
                    } => textarea.scroll((1, 0)),
                    Input {
                        key: Key::Char('y'),
                        ctrl: true,
                        ..
                    } => textarea.scroll((-1, 0)),
                    Input {
                        key: Key::Char('d'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::HalfPageDown),
                    Input {
                        key: Key::Char('u'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::HalfPageUp),
                    Input {
                        key: Key::Char('f'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::PageDown),
                    Input {
                        key: Key::Char('b'),
                        ctrl: true,
                        ..
                    } => textarea.scroll(Scrolling::PageUp),
                    Input {
                        key: Key::Char('v'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Normal => {
                        textarea.start_selection();
                        return Transition::Mode(Mode::Visual);
                    }
                    Input {
                        key: Key::Char('V'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Normal => {
                        textarea.move_cursor(CursorMove::Head);
                        textarea.start_selection();
                        textarea.move_cursor(CursorMove::End);
                        return Transition::Mode(Mode::Visual);
                    }
                    Input { key: Key::Esc, .. }
                    | Input {
                        key: Key::Char('v'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.cancel_selection();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('g'),
                        ctrl: false,
                        ..
                    } if matches!(
                        self.pending,
                        Input {
                            key: Key::Char('g'),
                            ctrl: false,
                            ..
                        }
                    ) =>
                    {
                        textarea.move_cursor(CursorMove::Top)
                    }
                    Input {
                        key: Key::Char('G'),
                        ctrl: false,
                        ..
                    } => textarea.move_cursor(CursorMove::Bottom),
                    Input {
                        key: Key::Char(c),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Operator(c) => {
                        // Handle yy, dd, cc. (This is not strictly the same behavior as Vim)
                        textarea.move_cursor(CursorMove::Head);
                        textarea.start_selection();
                        let cursor = textarea.cursor();
                        textarea.move_cursor(CursorMove::Down);
                        if cursor == textarea.cursor() {
                            textarea.move_cursor(CursorMove::End); // At the last line, move to end of the line instead
                        }
                    }
                    Input {
                        key: Key::Char(op @ ('y' | 'd' | 'c')),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Normal => {
                        textarea.start_selection();
                        return Transition::Mode(Mode::Operator(op));
                    }
                    Input {
                        key: Key::Char('y'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.move_cursor(CursorMove::Forward); // Vim's text selection is inclusive
                        textarea.copy();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('d'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.move_cursor(CursorMove::Forward); // Vim's text selection is inclusive
                        textarea.cut();
                        return Transition::Mode(Mode::Normal);
                    }
                    Input {
                        key: Key::Char('c'),
                        ctrl: false,
                        ..
                    } if self.mode == Mode::Visual => {
                        textarea.move_cursor(CursorMove::Forward); // Vim's text selection is inclusive
                        textarea.cut();
                        return Transition::Mode(Mode::Insert);
                    }
                    Input {
                        key: Key::Char(':'),
                        ..
                        // Do not wait until next key press, return Transition directly
                    } if self.mode == Mode::Normal => return Transition::Command,
                    input => return Transition::Pending(input),
                }

                match self.mode {
                    Mode::Operator('y') => {
                        textarea.copy();
                        Transition::Mode(Mode::Normal)
                    }
                    Mode::Operator('d') => {
                        textarea.cut();
                        Transition::Mode(Mode::Normal)
                    }
                    Mode::Operator('c') => {
                        textarea.cut();
                        Transition::Mode(Mode::Insert)
                    }
                    _ => Transition::Nop,
                }
            }
            Mode::Insert => match input {
                Input { key: Key::Esc, .. } => Transition::Mode(Mode::Normal),
                Input {
                    key: Key::Char(char),
                    ..
                } => {
                    textarea.insert_char(char);
                    Transition::Mode(Mode::Insert)
                }
                input => {
                    textarea.input(input);
                    Transition::Mode(Mode::Insert)
                }
            },
        }
    }
}
