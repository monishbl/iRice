//! The screen group of the host's desktop services (wave 4): Spotlight's
//! shell rows, files, the clipboard's write side, the screen tools (grim,
//! tesseract, the recorder), Live Text, live captions and dictation.
//! Each file names the QML it replaces; host/desktop/PORT-NOTES-screen.md
//! says what differs and how to check it.

pub mod captions;
pub mod clipboard;
pub mod dictation;
pub mod files;
pub mod livetext;
pub mod ocr;
mod recorder;
pub mod screentools;
pub mod search;
pub mod service;
pub mod util;

use tessera_core::host::Service;

/// Every service of this group (call prefixes: search, files, clipboard,
/// screen, recorder, livetext, captions; IPC targets: screen, recorder,
/// shotEditor, captions, dictation)
pub fn services() -> Vec<Box<dyn Service>> {
    vec![
        Box::new(search::SearchSvc::new()),
        Box::new(files::FilesSvc),
        Box::new(clipboard::ClipboardSvc::new()),
        Box::new(service::ScreenSvc::new()),
        Box::new(captions::CaptionsSvc::new()),
        Box::new(dictation::DictationSvc::new()),
    ]
}
