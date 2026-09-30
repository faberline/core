//! The issue and upgrade use cases against fake ports: which port each branch
//! reaches, and which it must not.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value};

use crate::application::issue::comment::{comment, CommentOptions};
use crate::application::issue::create::{create, CreateOptions};
use crate::application::upgrade::{run, Options};
use crate::domain::issue::created::CreatedIssue;
use crate::domain::issue::tracker::{CourierApi, GitHubApi, NodeProbe, TrackerAccess};
use crate::domain::prompt::{Confirm, PromptError};
use crate::domain::release::{InstallError, ReleaseSource, SelfInstall};
use crate::domain::remote::RemoteError;
use crate::ToolInfo;

const TOOL: ToolInfo = ToolInfo::new(
    "lumen",
    "faberline/lumen",
    "aarch64-apple-darwin",
    "0.4.11",
    "abc1234",
    "1700000000",
);

struct Access {
    courier: Option<&'static str>,
    token: Option<&'static str>,
}

impl TrackerAccess for Access {
    fn courier_url(&self) -> Option<String> {
        self.courier.map(str::to_string)
    }
    fn github_token(&self) -> Option<String> {
        self.token.map(str::to_string)
    }
}

/// Answers every confirmation with `answer`; `None` fails the test if asked.
struct Prompt(Option<bool>);

impl Confirm for Prompt {
    fn confirm(&self, prompt: &str) -> Result<bool, PromptError> {
        Ok(self.0.unwrap_or_else(|| panic!("asked `{prompt}`")))
    }
}

/// Records each remote call it receives.
#[derive(Clone, Default)]
struct Api(Rc<RefCell<Vec<String>>>);

impl Api {
    fn log(&self, call: String) {
        self.0.borrow_mut().push(call);
    }
    fn calls(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}

fn created() -> CreatedIssue {
    CreatedIssue {
        url: "https://github.com/faberline/lumen/issues/1".into(),
        labels: vec![],
    }
}

impl GitHubApi for Api {
    async fn get_json(&self, url: &str, _what: &'static str) -> Result<Value, RemoteError> {
        self.log(format!("get {url}"));
        Ok(json!({}))
    }
    async fn submit_issue(
        &self,
        repo: &str,
        token: &str,
        _: &Value,
    ) -> Result<CreatedIssue, RemoteError> {
        self.log(format!("submit {repo} {token}"));
        Ok(created())
    }
    async fn reopen_issue(
        &self,
        repo: &str,
        number: u64,
        token: &str,
    ) -> Result<String, RemoteError> {
        self.log(format!("reopen {repo}#{number} {token}"));
        Ok(String::new())
    }
    async fn post_issue_comment(
        &self,
        repo: &str,
        number: u64,
        token: &str,
        _body: &str,
    ) -> Result<String, RemoteError> {
        self.log(format!("comment {repo}#{number} {token}"));
        Ok(String::new())
    }
}

impl CourierApi for Api {
    async fn courier_get_json(&self, url: &str, _what: &'static str) -> Result<Value, RemoteError> {
        self.log(format!("courier get {url}"));
        Ok(json!({}))
    }
    async fn submit_issue_via_courier(
        &self,
        url: &str,
        _: &Value,
    ) -> Result<CreatedIssue, RemoteError> {
        self.log(format!("courier submit {url}"));
        Ok(created())
    }
    async fn post_issue_comment_via_courier(
        &self,
        url: &str,
        _: &str,
    ) -> Result<String, RemoteError> {
        self.log(format!("courier comment {url}"));
        Ok(String::new())
    }
}

impl NodeProbe for Api {
    async fn node_status(&self, url: &str) -> String {
        self.log(format!("node {url}"));
        String::new()
    }
}

impl ReleaseSource for Api {
    async fn releases(&self, repo: &str) -> Result<Value, RemoteError> {
        self.log(format!("releases {repo}"));
        Ok(json!([{ "tag_name": "lumen@0.4.12" }, { "tag_name": "vat@9.9.9" }]))
    }
    async fn release(&self, repo: &str, tag: &str) -> Result<Value, RemoteError> {
        self.log(format!("release {repo} {tag}"));
        Ok(json!({}))
    }
    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>, RemoteError> {
        self.log(format!("download {url}"));
        Ok(vec![])
    }
    async fn download_text(&self, url: &str) -> Result<String, RemoteError> {
        self.log(format!("download {url}"));
        Ok(String::new())
    }
}

struct NoInstall;

impl SelfInstall for NoInstall {
    fn install_over_self(&self, _: &[u8], _: &str) -> Result<(), InstallError> {
        panic!("installed a binary")
    }
}

fn never() -> Result<Api, RemoteError> {
    panic!("built an HTTP client")
}

#[tokio::test]
async fn comment_dry_run_builds_no_client_and_asks_nothing() {
    let opts = CommentOptions::try_new(7).unwrap().with_dry_run(true);
    let access = Access {
        courier: None,
        token: Some("tok"),
    };
    comment(&TOOL, opts, &access, &Prompt(None), never)
        .await
        .unwrap();
}

#[tokio::test]
async fn comment_declined_stops_before_the_client_is_built() {
    let opts = CommentOptions::try_new(7).unwrap();
    let access = Access {
        courier: None,
        token: Some("tok"),
    };
    comment(&TOOL, opts, &access, &Prompt(Some(false)), never)
        .await
        .unwrap();
}

#[tokio::test]
async fn comment_reopens_then_comments_with_the_github_token() {
    let api = Api::default();
    let opts = CommentOptions::try_new(7).unwrap().with_yes(true);
    let access = Access {
        courier: None,
        token: Some("tok"),
    };
    let open = || Ok(api.clone());
    comment(&TOOL, opts, &access, &Prompt(None), open)
        .await
        .unwrap();
    assert_eq!(
        api.calls(),
        [
            "reopen faberline/lumen#7 tok",
            "comment faberline/lumen#7 tok"
        ]
    );
}

#[tokio::test]
async fn create_goes_through_courier_when_it_is_configured() {
    let api = Api::default();
    let opts = CreateOptions::new("boom").with_yes(true);
    let access = Access {
        courier: Some("https://courier.example/"),
        token: Some("tok"),
    };
    let open = || Ok(api.clone());
    create(&TOOL, opts, &access, &Prompt(None), open)
        .await
        .unwrap();
    assert_eq!(
        api.calls(),
        ["courier submit https://courier.example/v1/issues/faberline/lumen"]
    );
}

#[tokio::test]
async fn create_without_a_credential_submits_nothing_and_asks_nothing() {
    let api = Api::default();
    let opts = CreateOptions::new("boom");
    let access = Access {
        courier: None,
        token: None,
    };
    let open = || Ok(api.clone());
    create(&TOOL, opts, &access, &Prompt(None), open)
        .await
        .unwrap();
    assert!(api.calls().is_empty());
}

#[tokio::test]
async fn upgrade_check_lists_releases_and_installs_nothing() {
    let api = Api::default();
    let opts = Options {
        check: true,
        ..Options::default()
    };
    let open = || Ok(api.clone());
    run(&TOOL, opts, &Prompt(None), &NoInstall, open)
        .await
        .unwrap();
    assert_eq!(api.calls(), ["releases faberline/lumen"]);
}

/// The public entry points stay usable from multi-threaded runtimes.
#[test]
fn public_futures_are_send() {
    fn assert_send<T: Send>(_: T) {}
    assert_send(crate::issue::create(&TOOL, CreateOptions::default()));
    assert_send(crate::issue::comment(
        &TOOL,
        CommentOptions::try_new(1).unwrap(),
    ));
    assert_send(crate::issue::search(&TOOL, Default::default()));
    assert_send(crate::issue::view(&TOOL, 1));
    assert_send(crate::upgrade::run(&TOOL, Options::default()));
}
