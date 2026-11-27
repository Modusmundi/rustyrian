// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) rustyrian contributors

//! `.ots` input scripts, version 1.
//!
//! ```text
//! ots 1
//! joystick "Fake Pad" 2 4 1        # name, axes, buttons, hats
//! @1500 key-down RETURN
//! @1545 key-up RETURN
//! @4000 text "abc"
//! @5000 mouse-move 160 100
//! @5010 mouse-down 1
//! @6000 joy-axis 0 1 -32768
//! @7000 focus-lost
//! @900000 quit
//! ```
//!
//! `@T` is milliseconds on the session clock; the event is delivered by the
//! first poll at or after `T`. Timestamps must be non-decreasing.

use crate::keys;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoystickDecl {
    pub name: String,
    pub axes: i32,
    pub buttons: i32,
    pub hats: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    KeyDown {
        scancode: u16,
        keycode: i32,
    },
    KeyUp {
        scancode: u16,
        keycode: i32,
    },
    Text(String),
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        button: u8,
    },
    MouseUp {
        button: u8,
    },
    JoyAxis {
        device: i32,
        axis: i32,
        value: i16,
    },
    JoyButton {
        device: i32,
        button: i32,
        pressed: bool,
    },
    JoyHat {
        device: i32,
        hat: i32,
        value: u8,
    },
    FocusLost,
    FocusGained,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timed {
    pub at_ms: u32,
    pub action: Action,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Script {
    pub joysticks: Vec<JoystickDecl>,
    pub events: Vec<Timed>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ScriptError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ScriptError {}

struct Tok {
    text: String,
    col: usize,
    quoted: bool,
}

fn tokenize(line: &str, lineno: usize) -> Result<Vec<Tok>, ScriptError> {
    let mut toks = Vec::new();
    let mut chars = line.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '#' {
            break;
        }
        let col = i + 1;
        if c == '"' {
            chars.next();
            let mut s = String::new();
            let mut closed = false;
            while let Some((_, c)) = chars.next() {
                match c {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => match chars.next() {
                        Some((_, e @ ('"' | '\\'))) => s.push(e),
                        Some((j, e)) => {
                            return Err(ScriptError {
                                line: lineno,
                                col: j + 1,
                                message: format!("unknown escape `\\{e}`"),
                            });
                        }
                        None => break,
                    },
                    c => s.push(c),
                }
            }
            if !closed {
                return Err(ScriptError {
                    line: lineno,
                    col,
                    message: "unterminated string".into(),
                });
            }
            toks.push(Tok {
                text: s,
                col,
                quoted: true,
            });
        } else {
            let mut s = String::new();
            while let Some(&(_, c)) = chars.peek() {
                if c.is_whitespace() || c == '#' {
                    break;
                }
                s.push(c);
                chars.next();
            }
            toks.push(Tok {
                text: s,
                col,
                quoted: false,
            });
        }
    }
    Ok(toks)
}

struct Args<'a> {
    toks: &'a [Tok],
    pos: usize,
    line: usize,
    end_col: usize,
}

impl Args<'_> {
    fn err(&self, col: usize, message: impl Into<String>) -> ScriptError {
        ScriptError {
            line: self.line,
            col,
            message: message.into(),
        }
    }

    fn next(&mut self, what: &str) -> Result<&Tok, ScriptError> {
        let t = self
            .toks
            .get(self.pos)
            .ok_or_else(|| self.err(self.end_col, format!("expected {what}")))?;
        self.pos += 1;
        Ok(t)
    }

    fn num<T: std::str::FromStr>(&mut self, what: &str) -> Result<T, ScriptError> {
        let (text, col, quoted) = {
            let t = self.next(what)?;
            (t.text.clone(), t.col, t.quoted)
        };
        if quoted {
            return Err(self.err(col, format!("expected {what}, found string")));
        }
        text.parse()
            .map_err(|_| self.err(col, format!("invalid {what} `{text}`")))
    }

    fn string(&mut self, what: &str) -> Result<String, ScriptError> {
        let (text, col, quoted) = {
            let t = self.next(what)?;
            (t.text.clone(), t.col, t.quoted)
        };
        if !quoted {
            return Err(self.err(col, format!("expected quoted {what}")));
        }
        Ok(text)
    }

    fn key(&mut self) -> Result<(u16, i32), ScriptError> {
        let (text, col) = {
            let t = self.next("key name")?;
            (t.text.clone(), t.col)
        };
        keys::lookup(&text).ok_or_else(|| self.err(col, format!("unknown key `{text}`")))
    }

    fn done(&self) -> Result<(), ScriptError> {
        match self.toks.get(self.pos) {
            Some(t) => Err(self.err(t.col, format!("unexpected `{}`", t.text))),
            None => Ok(()),
        }
    }
}

pub fn parse(text: &str) -> Result<Script, ScriptError> {
    let mut script = Script::default();
    let mut seen_header = false;
    let mut last_ms = 0u32;
    for (idx, raw) in text.lines().enumerate() {
        let lineno = idx + 1;
        let toks = tokenize(raw, lineno)?;
        if toks.is_empty() {
            continue;
        }
        let mut a = Args {
            toks: &toks,
            pos: 0,
            line: lineno,
            end_col: raw.len() + 1,
        };
        if !seen_header {
            let (t, col) = {
                let t = a.next("header")?;
                (t.text.clone(), t.col)
            };
            if t != "ots" {
                return Err(a.err(col, "expected header `ots 1`"));
            }
            let v: u32 = a.num("version")?;
            if v != 1 {
                return Err(a.err(toks[1].col, format!("unsupported version {v}")));
            }
            a.done()?;
            seen_header = true;
            continue;
        }
        let (head, head_col) = {
            let t = a.next("statement")?;
            (t.text.clone(), t.col)
        };
        if head == "joystick" {
            if !script.events.is_empty() {
                return Err(a.err(head_col, "joystick declarations must precede events"));
            }
            let name = a.string("joystick name")?;
            let axes = a.num("axis count")?;
            let buttons = a.num("button count")?;
            let hats = a.num("hat count")?;
            a.done()?;
            script.joysticks.push(JoystickDecl {
                name,
                axes,
                buttons,
                hats,
            });
            continue;
        }
        let Some(ms) = head.strip_prefix('@') else {
            return Err(a.err(
                head_col,
                format!("expected `@<ms>` or `joystick`, found `{head}`"),
            ));
        };
        let at_ms: u32 = ms
            .parse()
            .map_err(|_| a.err(head_col, format!("invalid time `{head}`")))?;
        if at_ms < last_ms {
            return Err(a.err(head_col, format!("time {at_ms} precedes {last_ms}")));
        }
        last_ms = at_ms;
        let (verb, verb_col) = {
            let t = a.next("action")?;
            (t.text.clone(), t.col)
        };
        let action = match verb.as_str() {
            "key-down" => {
                let (scancode, keycode) = a.key()?;
                Action::KeyDown { scancode, keycode }
            }
            "key-up" => {
                let (scancode, keycode) = a.key()?;
                Action::KeyUp { scancode, keycode }
            }
            "text" => {
                let s = a.string("text")?;
                if s.len() > 31 {
                    return Err(a.err(verb_col, "text longer than 31 bytes"));
                }
                Action::Text(s)
            }
            "mouse-move" => Action::MouseMove {
                x: a.num("x")?,
                y: a.num("y")?,
            },
            "mouse-down" => Action::MouseDown {
                button: a.num("button")?,
            },
            "mouse-up" => Action::MouseUp {
                button: a.num("button")?,
            },
            "joy-axis" => Action::JoyAxis {
                device: a.num("device")?,
                axis: a.num("axis")?,
                value: a.num("axis value")?,
            },
            "joy-button" => {
                let device = a.num("device")?;
                let button = a.num("button")?;
                let state: u8 = a.num("button state")?;
                if state > 1 {
                    return Err(a.err(verb_col, "button state must be 0 or 1"));
                }
                Action::JoyButton {
                    device,
                    button,
                    pressed: state == 1,
                }
            }
            "joy-hat" => Action::JoyHat {
                device: a.num("device")?,
                hat: a.num("hat")?,
                value: a.num("hat value")?,
            },
            "focus-lost" => Action::FocusLost,
            "focus-gained" => Action::FocusGained,
            "quit" => Action::Quit,
            other => return Err(a.err(verb_col, format!("unknown action `{other}`"))),
        };
        a.done()?;
        script.events.push(Timed { at_ms, action });
    }
    if !seen_header {
        return Err(ScriptError {
            line: 1,
            col: 1,
            message: "missing header `ots 1`".into(),
        });
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_action() {
        let s = parse(
            "# demo\nots 1\njoystick \"Fake \\\"Pad\\\"\" 2 4 1\n@0 key-down LALT\n@10 key-down RETURN # alt-enter\n@20 text \"abc\"\n@30 mouse-move 160 100\n@31 mouse-down 1\n@32 mouse-up 1\n@40 joy-axis 0 1 -32768\n@40 joy-button 0 0 1\n@41 joy-hat 0 0 1\n@50 focus-lost\n@51 focus-gained\n@60 key-up 40\n@99 quit\n",
        )
        .expect("valid script");
        assert_eq!(s.joysticks[0].name, "Fake \"Pad\"");
        assert_eq!(s.events.len(), 13);
        assert_eq!(
            s.events[1].action,
            Action::KeyDown {
                scancode: 40,
                keycode: 13
            }
        );
        assert_eq!(
            s.events[6].action,
            Action::JoyAxis {
                device: 0,
                axis: 1,
                value: -32768
            }
        );
    }

    #[test]
    fn reports_line_and_column() {
        let e = parse("ots 1\n@10 key-down NOPE\n").unwrap_err();
        assert_eq!((e.line, e.col), (2, 14));
        let e = parse("ots 1\n@10 quit\n@5 quit\n").unwrap_err();
        assert_eq!((e.line, e.col), (3, 1));
        let e = parse("ots 2\n").unwrap_err();
        assert_eq!(e.line, 1);
    }
}
