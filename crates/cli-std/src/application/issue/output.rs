#[cfg(feature = "online")]
use crate::domain::issue::created::{created_issue_label_state, CreatedIssue};
use crate::domain::issue::url::{issue_url, prefilled_url};

#[cfg(feature = "online")]
pub(super) fn print_created_issue(created: &CreatedIssue, requested_labels: &[String]) {
    println!("filed: {}", created.url);
    println!(
        "{}",
        created_issue_label_state(requested_labels, &created.labels)
    );
    println!("next: done");
}

pub(super) fn print_preview(repo: &str, title: &str, body: &str, labels: &[String]) {
    println!("repo:  {repo}");
    println!("title: {title}");
    if !labels.is_empty() {
        println!("labels: {}", labels.join(", "));
    }
    println!("---");
    println!("{body}");
    println!("next: done");
}

/// Print the pre-filled-issue URL plus the title/body so the user can file by
/// hand. The preceding diagnostic note (why we fell back) is the caller's
/// responsibility — the "no credential" and "offline build" conditions are
/// distinct and must not be conflated.
pub(super) fn print_fallback(repo: &str, title: &str, body: &str, labels: &[String]) {
    println!("{}", prefilled_url(repo, title, body, labels));
    println!("next: done");
    eprintln!("\n--- title ---\n{title}\n--- body ---\n{body}");
}

pub(super) fn print_comment_preview(repo: &str, number: u64, body: &str) {
    println!("repo:  {repo}");
    println!("issue: #{number}");
    println!("state: open");
    println!("---");
    println!("{body}");
    println!("next: done");
}

pub(super) fn print_comment_fallback(repo: &str, number: u64, body: &str) {
    println!("{}", issue_url(repo, number));
    println!("next: done");
    eprintln!("\n--- comment ---\n{body}");
}

/// Online build, but no GitHub credential was found anywhere.
#[cfg(feature = "online")]
pub(super) fn note_no_credential() {
    eprintln!(
        "note: no GitHub credential found (checked $GH_TOKEN, $GITHUB_TOKEN, and `gh auth token`). \
         Run `gh auth login` or set GITHUB_TOKEN to file directly. \
         Meanwhile, open this pre-filled issue:"
    );
}

/// Online build, but no GitHub credential was found for a state-changing comment.
#[cfg(feature = "online")]
pub(super) fn note_no_credential_comment() {
    eprintln!(
        "note: no GitHub credential found (checked $GH_TOKEN, $GITHUB_TOKEN, and `gh auth token`). \
         Run `gh auth login` or set GITHUB_TOKEN to comment and reopen directly. \
         Meanwhile, open this issue, reopen it if closed, and add the comment below:"
    );
}

/// This binary was built without the `online` feature, so it cannot do network
/// I/O at all — independent of whether a credential exists.
#[cfg(not(feature = "online"))]
pub(super) fn note_offline_build() {
    eprintln!(
        "note: this build has no `online` feature; it cannot file directly. \
         Open this pre-filled issue:"
    );
}

/// This binary was built without the `online` feature, so it cannot comment or
/// reopen via the GitHub API.
#[cfg(not(feature = "online"))]
pub(super) fn note_offline_comment_build() {
    eprintln!(
        "note: this build has no `online` feature; it cannot comment or reopen directly. \
         Open this issue, reopen it if closed, and add the comment below:"
    );
}
