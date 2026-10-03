mod completion;
mod history;

use crate::client::GameClient;
use cloud_provider_sim::DeviceId;
use completion::ConsoleHelper;
use history::HistoryFile;
use rustyline::{CompletionType, Config, EditMode, Editor};

pub type LineEditor = Editor<ConsoleHelper, rustyline::history::DefaultHistory>;

pub struct ConsoleEditor {
    editor: LineEditor,
    history: HistoryFile,
}

impl ConsoleEditor {
    pub fn new(client: GameClient, device: DeviceId) -> Result<Self, String> {
        let config = Config::builder()
            .max_history_size(2000)
            .map_err(|error| error.to_string())?
            .history_ignore_dups(true)
            .map_err(|error| error.to_string())?
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();
        let mut editor = Editor::with_config(config).map_err(|error| error.to_string())?;
        let mut history = HistoryFile::new(client.endpoint(), device);
        history.load(&mut editor);
        editor.set_helper(Some(ConsoleHelper::new(client, device)));
        Ok(Self { editor, history })
    }

    pub fn readline(&mut self, prompt: &str) -> rustyline::Result<String> {
        self.editor.readline(&format!("{prompt} "))
    }

    pub fn remember(&mut self, input: &str) -> Result<(), String> {
        self.editor
            .add_history_entry(input)
            .map_err(|error| error.to_string())?;
        self.history.save(&mut self.editor);
        Ok(())
    }
}

impl Drop for ConsoleEditor {
    fn drop(&mut self) {
        self.history.save(&mut self.editor);
    }
}
