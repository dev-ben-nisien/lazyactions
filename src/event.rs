use color_eyre::eyre::{WrapErr, eyre};
use ratatui::crossterm::event::{self, Event as CrosstermEvent};
use std::{
    process::Command, // Import Command for external process execution
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// Add these imports
use serde::{Deserialize, Serialize};

use crate::app::RepoInfo;
// To handle intermediate JSON parsing

/// The frequency at which tick events are emitted.
const TICK_FPS: f64 = 0.2;

// Define the structs for GitHub data - these remain largely the same,
// as they represent the final desired data structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubWorkflowRun {
    pub id: u64,
    pub actor_login: String,
    pub head_branch: String,
    pub repo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubJob {
    pub id: u64,
    pub name: String,
    pub run_id: u64,
    pub repo: String,
    pub run_url: String,
    pub actor_login: String,
    pub head_branch: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub html_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowData {
    pub runs: Vec<GithubWorkflowRun>,
    pub jobs: Vec<GithubJob>,
}

/// Representation of all possible events.
#[derive(Clone, Debug)]
pub enum Event {
    /// An event that is emitted on a regular schedule.
    ///
    /// Use this event to run any code which has to run outside of being a direct response to a user
    /// event. e.g. polling exernal systems, updating animations, or rendering the UI based on a
    /// fixed frame rate.
    Action(Option<WorkflowData>), // Still carries optional workflow data
    /// Crossterm events.
    ///
    /// These events are emitted by the terminal.
    Crossterm(CrosstermEvent),
    /// Application events.
    ///
    /// Use this event to emit custom events that are specific to your application.
    App(AppEvent),
}

/// Application events.
///
/// You can extend this enum with your own custom events.
#[derive(Clone, Debug)]
pub enum AppEvent {
    /// Increment the counter.
    NavigateLeft,
    /// Decrement the counter.
    NavigateRight,
    /// Quit the application.
    NavigateUp,
    NavigateDown,
    Quit,
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::Sender<Event>,
    /// Event receiver channel.
    receiver: mpsc::Receiver<Event>, // Store repo info for later use
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let actor = EventThread::new(sender.clone());
        // No need for Tokio runtime here, as we're using blocking Command::output
        thread::spawn(|| actor.run());
        Self { sender, receiver }
    }

    /// Receives an event from the sender.
    ///
    /// This function blocks until an event is received.
    ///
    /// # Errors
    ///
    /// This function returns an error if the sender channel is disconnected. This can happen if an
    /// error occurs in the event thread. In practice, this should not happen unless there is a
    /// problem with the underlying terminal.
    pub fn next(&self) -> color_eyre::Result<Event> {
        Ok(self.receiver.recv()?)
    }

    /// Queue an app event to be sent to the event receiver.
    ///
    /// This is useful for sending events to the event handler which will be processed by the next
    /// iteration of the application's event loop.
    pub fn send(&mut self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(app_event));
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
struct EventThread {
    /// Event sender channel.
    sender: mpsc::Sender<Event>,
    repo_info: RepoInfo,
}
fn fetch_repo_info() -> color_eyre::Result<RepoInfo> {
    let output = Command::new("gh")
        .arg("repo")
        .arg("view")
        .arg("--json")
        .arg("owner,name")
        .output()?; // This is now a blocking call

    if output.status.success() {
        let json_str = String::from_utf8(output.stdout)?;
        let repo_info: RepoInfo = serde_json::from_str(&json_str)?;
        Ok(repo_info)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        Err(color_eyre::eyre::eyre!(
            "Failed to fetch repo info: {}",
            error_msg
        ))
    }
}
impl EventThread {
    /// Constructs a new instance of [`EventThread`].
    fn new(sender: mpsc::Sender<Event>) -> Self {
        let repo_info = match fetch_repo_info() {
            Ok(info) => info,
            Err(e) => {
                eprintln!("Error fetching repository info: {:?}", e);
                RepoInfo::default() // Use default if fetching fails
            }
        };
        Self { sender, repo_info }
    }

    /// Executes a `gh` CLI command and returns its stdout as a string.
    ///
    /// # Arguments
    /// * `args` - A slice of string slices, representing the arguments to pass to `gh`.
    fn run_gh_command(&self, args: &[&str]) -> color_eyre::Result<String> {
        let output = Command::new("gh")
            .args(args)
            .output()
            .wrap_err(format!("Failed to execute `gh {}` command", args.join(" ")))?;

        if !output.status.success() {
            return Err(eyre!(
                "Command `gh {}` failed with exit code {}:\nStdout: {}\nStderr: {}",
                args.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Fetches workflow runs and jobs using the GitHub CLI.
    fn fetch_github_workflow_data(&self) -> color_eyre::Result<WorkflowData> {
        let mut workflow_runs: Vec<GithubWorkflowRun> = Vec::new();
        let mut all_jobs: Vec<GithubJob> = Vec::new();

        // 1. Fetch the LATEST 3 workflow runs
        // This jq command (`.workflow_runs[0:3] | .[]`) outputs a stream of JSON objects, each on a new line.
        let runs_json_str = self.run_gh_command(&[
        "api",
        "-H",
        "Accept: application/vnd.github+json",
        &format!("/repos/{}/{}/actions/runs", self.repo_info.owner.login, self.repo_info.name),
        "--jq",
        ".workflow_runs[0:3] | .[] | {id: .id, actor_login: .actor.login, head_branch: .head_branch, repo: .repository.full_name}",
    ])?;

        let mut gh_runs: Vec<GithubWorkflowRun> = Vec::new();
        for line in runs_json_str.lines() {
            if line.trim().is_empty() {
                continue; // Skip empty lines
            }
            let run: GithubWorkflowRun = serde_json::from_str(line)
                .wrap_err(format!("Failed to parse workflow run JSON line: {}", line))?;
            gh_runs.push(run);
        }

        for run in gh_runs {
            // Need to clone for the jobs query later
            let current_run_id = run.id;
            let current_actor_login = run.actor_login.clone();
            let current_head_branch = run.head_branch.clone();
            let repo_name = run.repo.clone();
            workflow_runs.push(run);

            // 2. Fetch jobs for each run using `gh api` and jq
            //    Filtering jobs to include only those with 'success', 'failure', or 'in_progress' status/conclusion.
            //    'skipped' and 'queued' are explicitly excluded by this selection.
            let jobs_json_str = self.run_gh_command(&[
            "api",
            "--paginate",
            "-H",
            "Accept: application/vnd.github+json",
            &format!(
                "/repos/{}/{}/actions/runs/{}/jobs",
                self.repo_info.owner.login,self.repo_info.name, current_run_id
            ),
            "--jq",
            &format!(
                // Filter jobs:
                // - if status is 'in_progress'
                // - OR if conclusion is 'success' or 'failure'
                ".\"jobs\"[] | select(.status == \"in_progress\" or (.conclusion == \"success\" or .conclusion == \"failure\")) | {{id: .id, name: .name, run_id: {}, run_url: .run_url, actor_login: \"{}\", head_branch: \"{}\", status: .status, conclusion: .conclusion, started_at: .started_at, completed_at: .completed_at, html_url: .html_url, repo: \"{}\"}}",
                current_run_id, current_actor_login, current_head_branch, repo_name // Inject these into jq
            ),
        ])?;

            // Parse each line as a separate JSON object because `jq .[]` outputs a stream of objects
            for line in jobs_json_str.lines() {
                if line.trim().is_empty() {
                    continue; // Skip empty lines
                }
                let job: GithubJob = serde_json::from_str(line).wrap_err(format!(
                    "Failed to parse job JSON line for run {}: {}",
                    current_run_id, line
                ))?;
                all_jobs.push(job);
            }
        }

        Ok(WorkflowData {
            runs: workflow_runs,
            jobs: all_jobs,
        })
    }
    /// Runs the event thread.
    ///
    /// This function emits tick events at a fixed rate and polls for crossterm events in between.
    fn run(self) -> color_eyre::Result<()> {
        let tick_interval = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut last_tick = Instant::now();
        let mut first = true;
        loop {
            // emit tick events at a fixed rate
            let timeout = tick_interval.saturating_sub(last_tick.elapsed());
            if timeout == Duration::ZERO || first {
                last_tick = Instant::now();
                first = false;

                // Fetch GitHub data here using gh CLI and send it with the Action event
                match self.fetch_github_workflow_data() {
                    Ok(data) => self.send(Event::Action(Some(data))),
                    Err(e) => {
                        eprintln!("Error fetching GitHub data via gh CLI: {:?}", e);
                        self.send(Event::Action(None)); // Send an action event without data
                    }
                }
            }
            // poll for crossterm events, ensuring that we don't block the tick interval
            if event::poll(timeout).wrap_err("failed to poll for crossterm events")? {
                let event = event::read().wrap_err("failed to read crossterm event")?;
                self.send(Event::Crossterm(event));
            }
        }
    }

    /// Sends an event to the receiver.
    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.sender.send(event);
    }
}
