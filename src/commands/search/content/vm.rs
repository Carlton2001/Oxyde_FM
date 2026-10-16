//! Boolean Thompson simulation. State sets, unlike a haystack, do not grow with
//! line length. Parsing, UTF-8 transitions and assertions come from regex itself.
use regex_automata::{
    nfa::thompson::{State, NFA},
    util::primitives::StateID,
};
use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct Vm<'a> {
    nfa: &'a NFA,
    seeds: Vec<StateID>,
    stack: Vec<StateID>,
    next: Vec<StateID>,
    visited: Vec<u64>,
    generation: u64,
    previous: Option<char>,
    pending: Option<char>,
    pub(super) matched: bool,
}

fn reserve<T>(length: usize) -> io::Result<Vec<T>> {
    let mut items = Vec::new();
    items.try_reserve_exact(length).map_err(io::Error::other)?;
    Ok(items)
}

impl<'a> Vm<'a> {
    pub(super) fn new(nfa: &'a NFA) -> io::Result<Self> {
        let count = nfa.states().len();
        let mut seeds = reserve(count)?;
        seeds.push(nfa.start_unanchored());
        let mut visited = reserve(count)?;
        visited.resize(count, 0);
        Ok(Self {
            nfa,
            seeds,
            stack: reserve(count)?,
            next: reserve(count)?,
            visited,
            generation: 0,
            previous: None,
            pending: None,
            matched: false,
        })
    }

    pub(super) fn push(&mut self, c: char, cancel: &AtomicBool) -> io::Result<()> {
        if self.matched {
            return Ok(());
        }
        if let Some(current) = self.pending {
            self.advance(current, Some(c), cancel)?;
            self.previous = Some(current);
        }
        self.pending = Some(c);
        Ok(())
    }

    pub(super) fn finish(&mut self, cancel: &AtomicBool) -> io::Result<bool> {
        if !self.matched {
            if let Some(current) = self.pending.take() {
                self.advance(current, None, cancel)?;
                self.previous = Some(current);
            }
            let mut buffer = [0; 4];
            let previous = self
                .previous
                .map(|c| c.encode_utf8(&mut buffer).as_bytes())
                .unwrap_or_default();
            self.step(previous, previous.len(), cancel)?;
        }
        Ok(self.matched)
    }

    fn advance(
        &mut self,
        current: char,
        following: Option<char>,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        // Complete preceding/following scalars suffice for every supported look
        // assertion. The current byte is never at an artificial start/end.
        let mut context = [0; 12];
        let mut length = 0;
        if let Some(c) = self.previous {
            length += c.encode_utf8(&mut context[length..]).len();
        }
        let start = length;
        length += current.encode_utf8(&mut context[length..]).len();
        let end = length;
        if let Some(c) = following {
            length += c.encode_utf8(&mut context[length..]).len();
        }
        for at in start..end {
            self.step(&context[..length], at, cancel)?;
            if self.matched {
                break;
            }
        }
        Ok(())
    }

    fn enqueue(&mut self, id: StateID) {
        if self.visited[id.as_usize()] != self.generation {
            self.visited[id.as_usize()] = self.generation;
            self.stack.push(id);
        }
    }

    fn step(&mut self, context: &[u8], at: usize, cancel: &AtomicBool) -> io::Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("Search cancelled"));
        }
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.visited.fill(0);
            self.generation = 1;
        }
        for index in 0..self.seeds.len() {
            self.enqueue(self.seeds[index]);
        }
        self.next.clear();
        let mut work = 0;
        while let Some(id) = self.stack.pop() {
            work += 1;
            if work % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                return Err(io::Error::other("Search cancelled"));
            }
            let nfa = self.nfa;
            let transition = match nfa.state(id) {
                State::ByteRange { trans } => context
                    .get(at)
                    .filter(|&&b| trans.matches_byte(b))
                    .map(|_| trans.next),
                State::Sparse(trans) => trans.matches(context, at),
                State::Dense(trans) => trans.matches(context, at),
                State::Look { look, next } => {
                    if self.nfa.look_matcher().matches(*look, context, at) {
                        self.enqueue(*next);
                    }
                    None
                }
                State::Union { alternates } => {
                    for &next in alternates.iter() {
                        self.enqueue(next);
                    }
                    None
                }
                State::BinaryUnion { alt1, alt2 } => {
                    self.enqueue(*alt1);
                    self.enqueue(*alt2);
                    None
                }
                State::Capture { next, .. } => {
                    self.enqueue(*next);
                    None
                }
                State::Match { .. } => {
                    // Regex on str never reports an empty match inside a scalar,
                    // even for byte-mode assertions such as (?-u:\B).
                    if context.get(at).is_none_or(|b| b & 0xc0 != 0x80) {
                        self.matched = true;
                    }
                    None
                }
                State::Fail => None,
            };
            if let Some(next) = transition {
                self.next.push(next);
            }
        }
        std::mem::swap(&mut self.seeds, &mut self.next);
        Ok(())
    }
}
