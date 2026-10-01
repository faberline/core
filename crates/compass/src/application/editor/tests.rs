use std::path::PathBuf;

use crate::application::editor::session::EditorSession;
use crate::application::editor::views::CompletionKind;
use crate::domain::diagnostic::model::{Position, Range};
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::infrastructure::syntax::multi_parser::MultiParser;

const URI: &str = "file:///tmp/editor_test.py";
const SOURCE: &str = "import os\n\ndef greet(name):\n    return name\n\ngreet('x')\n";

fn session() -> EditorSession {
    EditorSession::new(
        Box::new(MultiParser::new().expect("grammars load")),
        RefactoringEngine::new(),
    )
}

async fn open(session: &EditorSession, content: &str) {
    let opened = session
        .open(
            URI,
            PathBuf::from("/tmp/editor_test.py"),
            content.to_string(),
            1,
        )
        .await;
    assert!(opened);
}

#[tokio::test]
async fn open_rejects_unknown_languages() {
    let session = session();
    let opened = session
        .open(
            "file:///tmp/notes.unknown",
            PathBuf::from("/tmp/notes.unknown"),
            String::new(),
            1,
        )
        .await;
    assert!(!opened);
    assert!(!session.is_open("file:///tmp/notes.unknown").await);
    assert!(session.analyse("file:///tmp/notes.unknown").await.is_none());
}

#[tokio::test]
async fn analyse_answers_symbol_queries() {
    let session = session();
    open(&session, SOURCE).await;
    assert!(session.analyse(URI).await.is_some());
    assert!(session.diagnostics(URI).await.is_some());

    let hover = session.hover(URI, 2, 5).await.expect("hover on greet");
    assert!(hover.markdown.contains("greet"));

    let definition = session.definition(URI, 5, 1).await.expect("definition");
    assert_eq!(definition.start.line, 2);

    assert!(!session.references(URI, 2, 5, true).await.is_empty());
}

#[tokio::test]
async fn completions_cover_modules_builtins_and_keywords() {
    let session = session();
    open(&session, "import os\nos.\npri\nde\n").await;

    let attributes = session.completions(URI, 1, 3, false).await.unwrap();
    assert!(attributes.iter().any(|c| c.label == "getcwd"));

    let builtins = session.completions(URI, 2, 3, false).await.unwrap();
    assert!(builtins.iter().any(|c| c.label == "print"));

    let keywords = session.completions(URI, 3, 2, false).await.unwrap();
    assert!(keywords
        .iter()
        .any(|c| c.label == "def" && c.kind == CompletionKind::Keyword));

    assert!(session.completions(URI, 9, 0, false).await.is_none());
}

#[tokio::test]
async fn refactorings_offer_extract_for_a_selection() {
    let session = session();
    open(&session, SOURCE).await;
    let selection = Range::new(Position::new(3, 11), Position::new(3, 15));
    let titles: Vec<String> = session
        .refactorings(URI, selection)
        .await
        .into_iter()
        .map(|p| p.title)
        .collect();
    assert!(
        titles.iter().any(|t| t == "Extract to variable"),
        "{titles:?}"
    );
}

#[tokio::test]
async fn close_forgets_the_document() {
    let session = session();
    open(&session, SOURCE).await;
    session.analyse(URI).await;
    session.close(URI).await;
    assert!(!session.is_open(URI).await);
    assert!(session.diagnostics(URI).await.is_none());
    assert!(session.hover(URI, 2, 5).await.is_none());
}
