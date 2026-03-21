use std::sync::Arc;

use forge_api::Environment;
use nu_ansi_term::{Color, Style};
use reedline::{
    ColumnarMenu, DefaultHinter, EditCommand, Emacs, FileBackedHistory, KeyCode, KeyModifiers,
    MenuBuilder, Prompt, Reedline, ReedlineEvent, ReedlineMenu, Signal, default_emacs_keybindings,
};

use super::completer::InputCompleter;
use crate::model::ForgeCommandManager;

// TODO: Store the last `HISTORY_CAPACITY` commands in the history file
const HISTORY_CAPACITY: usize = 1024 * 1024;
const COMPLETION_MENU: &str = "completion_menu";

pub struct ForgeEditor {
    editor: Reedline,
}

pub enum ReadResult {
    Success(String),
    Empty,
    Continue,
    Exit,
}

impl ForgeEditor {
    fn init() -> reedline::Keybindings {
        let mut keybindings = default_emacs_keybindings();
        // on TAB press shows the completion menu, and if we've exact match it will
        // insert it
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu(COMPLETION_MENU.to_string()),
                ReedlineEvent::Edit(vec![EditCommand::Complete]),
            ]),
        );

        // on CTRL + k press clears the screen
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('k'),
            ReedlineEvent::ClearScreen,
        );

        // on CTRL + r press searches the history
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('r'),
            ReedlineEvent::SearchHistory,
        );

        // on ALT + Enter press inserts a newline
        keybindings.add_binding(
            KeyModifiers::ALT,
            KeyCode::Enter,
            ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
        );

        keybindings
    }

    /// Detects if running in Git Bash or MinTTY on Windows
    ///
    /// These terminals have known issues with bracketed paste mode causing
    /// arrow keys and other special keys to stop working after the first input.
    fn is_git_bash_or_mintty() -> bool {
        #[cfg(windows)]
        {
            // Check for MSYSTEM which is set by Git Bash (MINGW64, MINGW32, MSYS, etc.)
            if std::env::var("MSYSTEM").is_ok() {
                return true;
            }

            // Check for TERM=xterm or TERM=cygwin which MinTTY uses
            if let Ok(term) = std::env::var("TERM") {
                if term == "xterm" || term == "cygwin" || term.starts_with("xterm-") {
                    // On Windows, xterm usually means MinTTY/Git Bash
                    // Real Windows terminals use different TERM values or don't set it
                    return true;
                }
            }

            // Check for MSYS which is also set by Git Bash
            if std::env::var("MSYS").is_ok() {
                return true;
            }
        }

        false
    }

    pub fn new(env: Environment, manager: Arc<ForgeCommandManager>) -> Self {
        // Store file history in system config directory
        let history_file = env.history_path();

        let history = Box::new(
            FileBackedHistory::with_file(HISTORY_CAPACITY, history_file).unwrap_or_default(),
        );
        let completion_menu = Box::new(
            ColumnarMenu::default()
                .with_name(COMPLETION_MENU)
                .with_marker("")
                .with_text_style(Style::new().bold().fg(Color::Cyan))
                .with_selected_text_style(Style::new().on(Color::White).fg(Color::Black)),
        );

        let edit_mode = Box::new(Emacs::new(Self::init()));

        // Git Bash and MinTTY on Windows have issues with bracketed paste mode
        // where arrow keys and delete stop working after the first prompt
        let use_bracketed_paste = !Self::is_git_bash_or_mintty();

        let editor = Reedline::create()
            .with_completer(Box::new(InputCompleter::new(env.cwd, manager)))
            .with_history(history)
            .with_hinter(Box::new(
                DefaultHinter::default().with_style(Style::new().fg(Color::DarkGray)),
            ))
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_edit_mode(edit_mode)
            .with_quick_completions(true)
            .with_ansi_colors(true)
            .use_bracketed_paste(use_bracketed_paste);
        Self { editor }
    }

    pub fn prompt(&mut self, prompt: &dyn Prompt) -> anyhow::Result<ReadResult> {
        let signal = self.editor.read_line(prompt);
        signal
            .map(Into::into)
            .map_err(|e| anyhow::anyhow!(ReadLineError(e)))
    }

    /// Sets the buffer content to be pre-filled on the next prompt
    pub fn set_buffer(&mut self, content: String) {
        self.editor
            .run_edit_commands(&[EditCommand::InsertString(content)]);
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ReadLineError(std::io::Error);

impl From<Signal> for ReadResult {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::Success(buffer) => {
                let trimmed = buffer.trim();
                if trimmed.is_empty() {
                    ReadResult::Empty
                } else {
                    ReadResult::Success(trimmed.to_string())
                }
            }
            Signal::CtrlC => ReadResult::Continue,
            Signal::CtrlD => ReadResult::Exit,
        }
    }
}
