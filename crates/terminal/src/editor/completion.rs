use crate::client::GameClient;
use cloud_provider_sim::{DeviceId, RemoteRequest, RemoteResponse};
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::Validator;
use rustyline::{Changeset, Context, Helper};
use std::borrow::Cow;

pub struct ConsoleHelper {
    client: GameClient,
    device: DeviceId,
    hinter: HistoryHinter,
}

impl ConsoleHelper {
    pub fn new(client: GameClient, device: DeviceId) -> Self {
        Self {
            client,
            device,
            hinter: HistoryHinter::new(),
        }
    }
}

impl Completer for ConsoleHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        position: usize,
        _: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let Some(prefix) = line.get(..position) else {
            return Ok((position, Vec::new()));
        };
        if prefix.contains(['\n', '\r']) {
            return Ok((position, Vec::new()));
        }
        let response = self
            .client
            .send(RemoteRequest::Complete {
                device: self.device,
                input: prefix.into(),
            })
            .map_err(std::io::Error::other)?;
        let RemoteResponse::Completions(mut completions) = response else {
            return Err(std::io::Error::other(match response {
                RemoteResponse::Error(error) => error,
                _ => "unexpected completion response".into(),
            })
            .into());
        };
        if completions.start > position || !line.is_char_boundary(completions.start) {
            return Err(std::io::Error::other("invalid completion position").into());
        }
        if completions.start == 0 {
            completions.candidates.extend(
                ["~.", "logout", "exit"]
                    .into_iter()
                    .filter(|command| command.starts_with(prefix))
                    .map(str::to_string),
            );
        }
        completions.candidates.sort();
        completions.candidates.dedup();
        let candidates = completions
            .candidates
            .into_iter()
            .map(|candidate| Pair {
                replacement: if position == line.len() {
                    format!("{candidate} ")
                } else {
                    candidate.clone()
                },
                display: candidate,
            })
            .collect();
        Ok((completions.start, candidates))
    }

    fn update(
        &self,
        line: &mut rustyline::line_buffer::LineBuffer,
        start: usize,
        elected: &str,
        changes: &mut Changeset,
    ) {
        let cursor = line.pos();
        let end = cursor
            + line.as_str()[cursor..]
                .find(char::is_whitespace)
                .unwrap_or(line.len() - cursor);
        line.replace(start..end, elected, changes);
    }
}

impl Hinter for ConsoleHelper {
    type Hint = String;
    fn hint(&self, line: &str, position: usize, context: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, position, context)
    }
}

impl Highlighter for ConsoleHelper {
    fn highlight_hint<'a>(&self, hint: &'a str) -> Cow<'a, str> {
        Cow::Owned(format!("\x1b[2m{hint}\x1b[0m"))
    }
}
impl Validator for ConsoleHelper {}
impl Helper for ConsoleHelper {}
