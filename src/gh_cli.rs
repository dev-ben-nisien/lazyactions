use color_eyre::eyre::{WrapErr, eyre};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubWorkflowRun {
    pub id: u64,
    pub actor_login: String,
    pub head_branch: String,
    pub repo: String,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct RepoInfo {
    pub name: String,
    pub owner: Owner,
}
#[derive(Debug, Default, Deserialize, Clone)]
pub struct Owner {
    pub login: String,
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

/// Fetches repository information using the `gh repo view` command.
pub fn fetch_repo_info() -> color_eyre::Result<RepoInfo> {
    let output = Command::new("gh")
        .arg("repo")
        .arg("view")
        .arg("--json")
        .arg("owner,name")
        .output()
        .wrap_err("Failed to execute `gh repo view` command")?;

    if output.status.success() {
        let json_str = String::from_utf8(output.stdout)
            .wrap_err("`gh repo view` output is not valid UTF-8")?;
        let repo_info: RepoInfo = serde_json::from_str(&json_str)
            .wrap_err(format!("Failed to parse `gh repo view` JSON: {}", json_str))?;
        Ok(repo_info)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        Err(eyre!(
            "Failed to fetch repo info. `gh repo view` exited with status {}:\n{}",
            output.status,
            error_msg
        ))
    }
}
// Helper function to run a command and return its stdout
fn run_command(command_name: &str, args: &[&str], error_msg: &str) -> color_eyre::Result<String> {
    let output = Command::new(command_name)
        .args(args)
        .output()
        .wrap_err(format!("Failed to execute `{}` command", command_name))?;

    if !output.status.success() {
        return Err(eyre!(
            "{}. Command `{}` failed with exit code {}:\nStdout: {}\nStderr: {}",
            error_msg,
            command_name,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
/// A client for interacting with the GitHub CLI.
#[derive(Debug, Clone)]
pub struct GhCli {
    repo_info: RepoInfo,
    branch: bool,
    user: bool,
    latest: bool,
    current_user: String,
    current_branch: String,
}

impl GhCli {
    /// Creates a new `GhCli` instance.
    /// It requires `RepoInfo` to construct API endpoints specific to the repository.
    pub fn new(branch: bool, user: bool, latest: bool) -> Self {
        let repo_info = match fetch_repo_info() {
            Ok(info) => info,
            Err(e) => {
                eprintln!("Error fetching repository info: {:?}", e);
                RepoInfo::default() // Provide a default or handle the error appropriately
            }
        };
        // Fetch current user using `gh auth status`
        let current_user = match Self::fetch_current_gh_user() {
            Ok(user) => user,
            Err(e) => {
                eprintln!("Warning: Could not determine current GitHub user: {:?}", e);
                String::new() // Default to empty string if not found
            }
        };

        // Fetch current branch using `git rev-parse --abbrev-ref HEAD`
        let current_branch = match Self::fetch_current_git_branch() {
            Ok(branch) => branch,
            Err(e) => {
                eprintln!("Warning: Could not determine current Git branch: {:?}", e);
                String::new() // Default to empty string if not found
            }
        };
        if branch {}
        Self {
            repo_info,
            branch,
            user,
            latest,
            current_branch,
            current_user,
        }
    }

    /// Fetches the current authenticated GitHub user's login.
    fn fetch_current_gh_user() -> color_eyre::Result<String> {
        // We parse the output of `gh auth status` to find the user.
        // A typical output might look like:
        // gh.github.com
        //   ✓ Logged in to github.com as octocat (~/.config/gh/hosts.yml)
        let output = run_command(
            "gh",
            &["auth", "status"],
            "Failed to fetch GitHub user status",
        )?;

        // Look for the line containing "Logged in to ... as <username>"
        output
            .lines()
            .find(|line| line.contains("Logged in to") && line.contains(" account "))
            .and_then(|line| {
                line.split(" account ")
                    .nth(1) // Get the part after " as "
                    .and_then(|s| s.split_whitespace().next()) // Get the first word (the username)
                    .map(|s| s.trim_matches(|c| c == '(' || c == ')').to_string())
            })
            .ok_or_else(|| {
                eyre!("Could not parse current GitHub user from `gh auth status` output")
            })
    }

    /// Fetches the current Git branch name.
    fn fetch_current_git_branch() -> color_eyre::Result<String> {
        run_command(
            "git",
            &["rev-parse", "--abbrev-ref", "HEAD"],
            "Failed to fetch current Git branch",
        )
    }
    /// Executes a `gh` CLI command and returns its stdout as a string.
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
    pub fn fetch_github_workflow_data(&self) -> color_eyre::Result<WorkflowData> {
        let mut workflow_runs: Vec<GithubWorkflowRun> = Vec::new();
        let mut all_jobs: Vec<GithubJob> = Vec::new();

        let mut gh_args = vec!["api", "-H", "Accept: application/vnd.github+json"];
        let api_path = format!(
            "/repos/{}/{}/actions/runs",
            self.repo_info.owner.login, self.repo_info.name
        );
        gh_args.push(&api_path);
        let mut jq_filters = Vec::new();
        jq_filters.push(format!(
            ".workflow_runs[0:{}]",
            self.latest.then_some(1).unwrap_or(3)
        ));
        jq_filters.push(".[]".to_string());
        if self.user {
            jq_filters.push(format!("select(.actor.login == \"{}\")", self.current_user));
        }
        if self.branch {
            jq_filters.push(format!(
                "select(.head_branch == \"{}\")",
                self.current_branch
            ));
        }
        jq_filters.push("{id: .id, actor_login: .actor.login, head_branch: .head_branch, repo: .repository.full_name}".to_string());
        let jq_query = jq_filters.join(" | ");
        gh_args.push("--jq");
        gh_args.push(&jq_query);
        let runs_json_str = self.run_gh_command(&gh_args)?;

        let mut gh_runs: Vec<GithubWorkflowRun> = Vec::new();
        for line in runs_json_str.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let run: GithubWorkflowRun = serde_json::from_str(line)
                .wrap_err(format!("Failed to parse workflow run JSON line: {}", line))?;
            gh_runs.push(run);
        }

        for run in gh_runs {
            let current_run_id = run.id;
            let current_actor_login = run.actor_login.clone();
            let current_head_branch = run.head_branch.clone();
            let repo_name = run.repo.clone();
            workflow_runs.push(run);

            // 2. Fetch jobs for each run using `gh api` and jq
            let jobs_json_str = self.run_gh_command(&[
                "api",
                "--paginate",
                "-H",
                "Accept: application/vnd.github+json",
                &format!(
                    "/repos/{}/{}/actions/runs/{}/jobs",
                    self.repo_info.owner.login, self.repo_info.name, current_run_id
                ),
                "--jq",
                &format!(
                    ".\"jobs\"[] | select(.status == \"in_progress\" or (.conclusion == \"success\" or .conclusion == \"failure\")) | {{id: .id, name: .name, run_id: {}, run_url: .run_url, actor_login: \"{}\", head_branch: \"{}\", status: .status, conclusion: .conclusion, started_at: .started_at, completed_at: .completed_at, html_url: .html_url, repo: \"{}\"}}",
                    current_run_id, current_actor_login, current_head_branch, repo_name
                ),
            ])?;

            for line in jobs_json_str.lines() {
                if line.trim().is_empty() {
                    continue;
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

    /// Fetches the console logs for a specific GitHub Job.
    /// Returns the raw log content as a string.
    pub fn fetch_job_logs(&self, job_id: u64) -> color_eyre::Result<String> {
        self.run_gh_command(&[
            "api",
            "-H",
            "Accept: application/vnd.github.v3+raw", // Request raw content
            &format!(
                "/repos/{}/{}/actions/jobs/{}/logs",
                self.repo_info.owner.login, self.repo_info.name, job_id
            ),
        ])
    }
}
