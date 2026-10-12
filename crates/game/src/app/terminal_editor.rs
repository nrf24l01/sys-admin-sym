use cloud_provider_sim::{ConsoleEditorFile, ConsoleEditorKind, DeviceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Nano,
    Normal,
    Insert,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPrompt {
    None,
    WritePath,
    VimCommand,
    ConfirmExit,
}
#[derive(Debug)]
pub enum EditorIntent {
    Save { path: String, contents: String },
    Close,
}

/// UI-owned draft. Saves remember the exact buffer sent to the worker.
pub struct TerminalEditor {
    pub device: DeviceId,
    pub kind: ConsoleEditorKind,
    pub path: String,
    pub buffer: String,
    pub mode: EditorMode,
    pub prompt: EditorPrompt,
    pub prompt_text: String,
    pub status: String,
    pub focus: bool,
    saved: String,
    pending: Option<(String, bool)>,
    exit_after_write: bool,
}

impl TerminalEditor {
    pub fn new(file: ConsoleEditorFile) -> Self {
        Self {
            device: file.device,
            kind: file.kind,
            path: file.path,
            saved: file.contents.clone(),
            buffer: file.contents,
            mode: if file.kind == ConsoleEditorKind::Nano {
                EditorMode::Nano
            } else {
                EditorMode::Normal
            },
            prompt: EditorPrompt::None,
            prompt_text: String::new(),
            status: String::new(),
            focus: true,
            pending: None,
            exit_after_write: false,
        }
    }
    pub fn dirty(&self) -> bool {
        self.buffer != self.saved
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn escape(&mut self) {
        self.prompt = EditorPrompt::None;
        self.prompt_text.clear();
        self.exit_after_write = false;
        if self.kind == ConsoleEditorKind::Vim {
            self.mode = EditorMode::Normal;
        }
        self.focus = true;
    }
    pub fn vim_normal_key(&mut self, key: char) {
        match key {
            'i' => self.mode = EditorMode::Insert,
            ':' => {
                self.prompt = EditorPrompt::VimCommand;
                self.prompt_text.clear();
            }
            _ => {}
        }
        self.focus = true;
    }
    pub fn nano_write(&mut self) {
        if self.busy() {
            return;
        }
        self.prompt = EditorPrompt::WritePath;
        self.prompt_text = self.path.clone();
        self.focus = true;
    }
    pub fn request_quit(&mut self) -> Option<EditorIntent> {
        if self.busy() {
            return None;
        }
        if !self.dirty() {
            return Some(EditorIntent::Close);
        }
        self.prompt = EditorPrompt::ConfirmExit;
        self.focus = true;
        None
    }
    pub fn confirm_save(&mut self) -> Option<EditorIntent> {
        if self.kind == ConsoleEditorKind::Nano {
            self.exit_after_write = true;
            self.nano_write();
            None
        } else {
            self.begin_save(self.path.clone(), true)
        }
    }
    pub fn submit_prompt(&mut self) -> Option<EditorIntent> {
        match self.prompt {
            EditorPrompt::WritePath => {
                self.begin_save(self.prompt_text.clone(), self.exit_after_write)
            }
            EditorPrompt::VimCommand => self.vim_command(&self.prompt_text.clone()),
            _ => None,
        }
    }
    pub fn vim_command(&mut self, command: &str) -> Option<EditorIntent> {
        let command = command.trim().trim_start_matches(':');
        self.prompt = EditorPrompt::None;
        self.focus = true;
        match command {
            "w" => self.begin_save(self.path.clone(), false),
            "wq" | "x" => self.begin_save(self.path.clone(), true),
            "q" => self.request_quit(),
            "q!" if !self.busy() => Some(EditorIntent::Close),
            _ if command.starts_with("w ") => self.begin_save(command[2..].trim().into(), false),
            _ => {
                self.status = "Supported commands: :w, :w FILE, :q, :q!, :wq".into();
                None
            }
        }
    }
    fn begin_save(&mut self, path: String, close: bool) -> Option<EditorIntent> {
        if self.busy() {
            return None;
        }
        if path.is_empty() {
            self.status = "File name cannot be empty".into();
            return None;
        }
        self.pending = Some((self.buffer.clone(), close));
        self.prompt = EditorPrompt::None;
        self.exit_after_write = false;
        self.focus = true;
        self.status = "Saving…".into();
        Some(EditorIntent::Save {
            path,
            contents: self.buffer.clone(),
        })
    }
    /// Close only if the saved snapshot still matches the current draft.
    pub fn finish_save(&mut self, result: Result<String, String>) -> bool {
        let Some((contents, close)) = self.pending.take() else {
            return false;
        };
        match result {
            Ok(path) => {
                self.path = path;
                self.saved = contents;
                self.status = "File saved".into();
                close && !self.dirty()
            }
            Err(error) => {
                self.status = error;
                false
            }
        }
    }
}
