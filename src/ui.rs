use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};

use crate::app::App;
use crate::event::GithubJob;

impl Widget for &App {
    /// Renders the user interface widgets.
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Define the main layout to split the screen vertically
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7), // Fixed height for header/instructions
                Constraint::Min(0),    // Remaining space for job details columns
            ])
            .split(area);

        // --- Render the header/instructions paragraph ---
        let header_block = Block::bordered()
            .title("lazyactions")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta));

        let header_text = format!(
            "Currently Running Jobs.\n\
                Press `Esc`, `Ctrl-C` or `q` to stop running.\n\
                Press `up`/`k` to increment, `down`/`j` to decrement counter",
        );

        let header_paragraph = Paragraph::new(header_text)
            .block(header_block)
            .fg(Color::Cyan)
            .bg(Color::Black)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        header_paragraph.render(chunks[0], buf);

        // --- Render the three job columns in the second chunk ---
        self.render_job_columns(chunks[1], buf);
    }
}

impl App {
    // This new function manages the three-column layout
    fn render_job_columns(&self, area: Rect, buf: &mut Buffer) {
        // Filter jobs into categories
        let (mut in_progress_jobs, mut success_jobs, mut failure_jobs) =
            (Vec::new(), Vec::new(), Vec::new());

        for job in self.job_details.iter().rev() {
            // Iterate over most recent jobs
            match job.status.as_str() {
                "completed" => {
                    if let Some(conclusion) = &job.conclusion {
                        match conclusion.as_str() {
                            "success" => success_jobs.push(job),
                            "failure" => failure_jobs.push(job),
                            _ => { /* Ignore cancelled, skipped, etc. as per request */ }
                        }
                    }
                }
                "in_progress" | "queued" | "waiting" => in_progress_jobs.push(job),
                _ => { /* Ignore other statuses if any */ }
            }
        }

        // Define the horizontal layout for the three columns
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33), // In Progress
                Constraint::Percentage(34), // Concluded Success
                Constraint::Percentage(33), // Concluded Failure
            ])
            .split(area);

        // Render each column using the helper function
        self.render_job_list_column(
            columns[0],
            buf,
            "In Progress",
            &in_progress_jobs,
            Color::Yellow, // Color for in progress
        );

        self.render_job_list_column(
            columns[1],
            buf,
            "Concluded Success",
            &success_jobs,
            Color::Green, // Color for success
        );

        self.render_job_list_column(
            columns[2],
            buf,
            "Concluded Failure",
            &failure_jobs,
            Color::Red, // Color for failure
        );
    }

    // Reusable function to render a single column of job summaries
    fn render_job_list_column(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        jobs: &[&GithubJob], // Correct type: slice of references to GithubJob
        border_color: Color,
    ) {
        let block = Block::default()
            .title(format!("{} ({})", title, jobs.len()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color));

        let inner_area = block.inner(area);
        block.render(area, buf);

        if jobs.is_empty() {
            let no_data_text = Text::styled(
                "No jobs in this category.",
                Style::default().fg(Color::DarkGray),
            );
            let paragraph = Paragraph::new(no_data_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });
            paragraph.render(inner_area, buf);
            return;
        }

        let lines_per_job_summary = 3;
        let separator_lines = 1;
        let total_lines_per_job_entry = lines_per_job_summary + separator_lines;

        let available_height = inner_area.height as usize;
        let max_jobs_by_height = if available_height >= lines_per_job_summary {
            let full_entries =
                (available_height - lines_per_job_summary) / total_lines_per_job_entry;
            full_entries + 1
        } else {
            0
        };

        let mut all_summary_lines: Vec<Line> = Vec::new();

        let num_jobs_in_category = jobs.len();
        let actual_jobs_to_display = max_jobs_by_height.min(num_jobs_in_category);

        for i in 0..actual_jobs_to_display {
            let job = jobs[i];

            let status_style = match job.status.as_str() {
                "completed" => Style::default().fg(Color::Green),
                "in_progress" => Style::default().fg(Color::Yellow),
                "queued" | "waiting" => Style::default().fg(Color::DarkGray),
                _ => Style::default().fg(Color::White),
            };

            let conclusion_span = if let Some(conclusion) = &job.conclusion {
                let conclusion_style = match conclusion.as_str() {
                    "success" => Style::default().fg(Color::LightGreen),
                    "failure" => Style::default().fg(Color::Red),
                    "cancelled" => Style::default().fg(Color::DarkGray),
                    "skipped" => Style::default().fg(Color::Blue),
                    _ => Style::default().fg(Color::White),
                };
                Span::styled(format!(" ({})", conclusion), conclusion_style)
            } else {
                Span::raw("")
            };

            all_summary_lines.push(Line::from(vec![
                Span::styled(
                    format!("{}. ", i + 1),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}", job.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [", status_style),
                Span::styled(job.status.clone(), status_style),
                conclusion_span,
                Span::styled("]", status_style),
            ]));
            all_summary_lines.push(Line::from(vec![Span::styled(
                format!("  {} by {}", job.head_branch, job.actor_login),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            )]));
            let max_url_width = inner_area.width.saturating_sub(2 + 3);
            let url_content = if job.html_url.len() > max_url_width as usize {
                format!(
                    "{}...",
                    &job.html_url[..(max_url_width as usize - 3).min(job.html_url.len())]
                )
            } else {
                job.html_url.clone()
            };
            all_summary_lines.push(Line::from(vec![
                Span::raw(format!("  {}", url_content))
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            ]));

            if i < actual_jobs_to_display.saturating_sub(1) {
                all_summary_lines.push(Line::from(Span::styled(
                    "---",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        let paragraph = Paragraph::new(all_summary_lines).wrap(Wrap { trim: false });

        paragraph.render(inner_area, buf);
    }
}
