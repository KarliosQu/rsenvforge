use std::collections::VecDeque;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Mutex, OnceLock};
use std::thread;

use super::error::ForgeError;

static INPUT: OnceLock<Mutex<InputState>> = OnceLock::new();

struct InputState {
    receiver: Receiver<String>,
    pending: VecDeque<String>,
}

pub fn read_user_line() -> Result<String, ForgeError> {
    let mut input = input_state()
        .lock()
        .map_err(|_| ForgeError::Command("读取用户输入时发生内部错误".to_string()))?;
    if let Some(line) = input.pending.pop_front() {
        return Ok(line);
    }
    input.receiver.recv().map_err(|error| ForgeError::Io {
        path: PathBuf::from("stdin"),
        source: io::Error::new(io::ErrorKind::UnexpectedEof, error),
    })
}

pub(crate) fn try_read_skip_request() -> Result<bool, ForgeError> {
    let mut input = input_state()
        .lock()
        .map_err(|_| ForgeError::Command("读取用户输入时发生内部错误".to_string()))?;
    if input
        .pending
        .front()
        .is_some_and(|line| is_skip_request(line))
    {
        input.pending.pop_front();
        return Ok(true);
    }
    if !input.pending.is_empty() {
        return Ok(false);
    }

    match input.receiver.try_recv() {
        Ok(line) if is_skip_request(&line) => Ok(true),
        Ok(line) => {
            input.pending.push_back(line);
            Ok(false)
        }
        Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(false),
    }
}

fn input_state() -> &'static Mutex<InputState> {
    INPUT.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Mutex::new(InputState {
            receiver,
            pending: VecDeque::new(),
        })
    })
}

fn is_skip_request(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("T")
}
