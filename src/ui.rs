use crate::app::App;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};
use std::collections::BTreeMap; // Using BTreeMap for sorted group keys // Assuming App struct is defined here

impl Widget for &App {
    /// Renders the user interface widgets.
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Define the main layout to split the screen vertically: Header + Body
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7), // Fixed height for header/instructions
                Constraint::Min(0),    // Remaining space for job columns OR logs + details
            ])
            .split(area);

        // --- Render the header/instructions paragraph ---
        let header_block = Block::bordered()
            .title("lazyactions")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta));

        let header_text = format!(
            "Showing jobs for: {} | Fetch Status: {}\n\
             Press `Esc`, `Ctrl-C` or `q` to stop running. \n\
             Use `Left`/`Right` to navigate columns, `Up`/`Down` for rows, `PageUp`/`PageDown` for scrolling\n\
             Press `Enter` to toggle more job info, `Backspace` to open GitHub URL. Auto-refresh every 5 seconds.",
            self.job_details
                .front()
                .map_or("N/A", |job| job.repo.as_str()),
            self.app_state.loading_status
        );

        let header_paragraph = Paragraph::new(header_text)
            .block(header_block)
            .fg(Color::Cyan)
            .bg(Color::Black)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });

        header_paragraph.render(main_chunks[0], buf);

        // --- Render the main application body based on show_details ---
        if self.app_state.show_details {
            // If show_details is true, render the detailed logs and full details panels
            self.render_detailed_overlay(main_chunks[1], buf);
        } else {
            // Otherwise, render the three job columns
            self.render_job_columns(main_chunks[1], buf);
        }
    }
}

impl App {
    // Renders the three-column job summary layout
    fn render_job_columns(&self, area: Rect, buf: &mut Buffer) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33), // In Progress
                Constraint::Percentage(34), // Concluded Success
                Constraint::Percentage(33), // Concluded Failure
            ])
            .split(area);

        self.render_job_list_column(
            columns[0],
            buf,
            "In Progress",
            &self.app_state.in_progress_jobs,
            Color::Yellow,
            0,
        );

        self.render_job_list_column(
            columns[1],
            buf,
            "Concluded Success",
            &self.app_state.success_jobs,
            Color::Green,
            1,
        );

        self.render_job_list_column(
            columns[2],
            buf,
            "Concluded Failure",
            &self.app_state.failure_jobs,
            Color::Red,
            2,
        );
    }

    // Reusable function to render a single column of job summaries
    fn render_job_list_column(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        job_indices: &BTreeMap<String, Vec<usize>>,
        border_color: Color,
        column_idx: usize,
    ) {
        let is_selected_column = self.app_state.column_index == column_idx;
        let block =
            Block::default()
                .title(format!("{} ({})", title, job_indices.iter().map(|(_, v)| v.len()).sum::<usize>()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color).add_modifier(
                    if is_selected_column {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    },
                ));

        let inner_area = block.inner(area);
        block.render(area, buf);

        if job_indices.is_empty() {
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

        // Group jobs by their "tool"

        let available_height = inner_area.height as usize;
        let mut all_column_lines: Vec<Line> = Vec::new(); // Collect all lines first

        let mut current_column_job_idx = 0; // Tracks the sequential index of jobs within the column (ignoring groups)

        // Iterate through grouped jobs to build all lines, including group headers
        for (tool_name, indices_in_group) in job_indices.iter() {
            // Add group header lines
            all_column_lines.push(Line::from(vec![
                Span::raw("── "),
                Span::styled(
                    tool_name.clone(),
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::raw(" ──"),
            ]));
            all_column_lines.push(Line::from(Span::styled(
                "─",
                Style::default().fg(Color::DarkGray),
            )));

            // Add job lines within this group
            for &original_job_idx in indices_in_group {
                let job = &self.job_details[original_job_idx];
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

                let base_style =
                    if is_selected_column && self.app_state.row_index == current_column_job_idx {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default().fg(Color::White)
                    };
                let action_part = job.name.split(" / ").last().unwrap_or(&job.name);
                let workflow_part = job.name.as_str();

                // Line 1: Index, Action (or primary name), Status, Conclusion
                all_column_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}. ", current_column_job_idx + 1), // Index relative to column view
                        base_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        action_part.to_string(), // Display the parsed action/primary name
                        base_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" [", status_style),
                    Span::styled(job.status.clone(), status_style),
                    conclusion_span,
                    Span::styled("]", status_style),
                ]));

                // Line 2: Workflow (conditionally displayed)
                if !workflow_part.is_empty() {
                    all_column_lines.push(Line::from(vec![
                        Span::raw("  "), // Indent for readability
                        Span::styled(
                            format!("{}", workflow_part),
                            base_style.fg(Color::LightYellow),
                        ),
                    ]));
                } else {
                    all_column_lines.push(Line::from(Span::raw("")));
                }

                // Line 4: Branch and Actor
                all_column_lines.push(Line::from(vec![Span::styled(
                    format!("  {} by {}", job.head_branch, job.actor_login),
                    base_style
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )]));

                current_column_job_idx += 1; // Increment for the next job
                all_column_lines.push(Line::from(Span::styled(
                    "\n",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        let scroll_offset = if is_selected_column {
            self.app_state.scroll_offset
        } else {
            0 // Other columns don't scroll unless selected
        };

        let start_index = scroll_offset.min(all_column_lines.len());
        let end_index = (start_index + available_height).min(all_column_lines.len());

        let visible_lines = &all_column_lines[start_index..end_index];

        let paragraph = Paragraph::new(visible_lines.to_vec()).wrap(Wrap { trim: false });
        paragraph.render(inner_area, buf);
    }

    /// Renders the detailed view with Job Logs and full Job Details in a horizontal split.
    fn render_detailed_overlay(&self, area: Rect, buf: &mut Buffer) {
        let detailed_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(70),
                Constraint::Percentage(30), // 30% for Job Details (bottom)
            ])
            .split(area);

        // Render Jobs
        self.render_job_columns(detailed_chunks[0], buf);

        // Render Job Details in the bottom panel
        self.render_full_job_details_panel(detailed_chunks[1], buf);
    }
    /// Renders the full job details panel (used in detailed view).
    fn render_full_job_details_panel(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("Job Details")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let selected_job_original_index = self.get_selected_job_original_index();
        let selected_job = selected_job_original_index.and_then(|idx| self.job_details.get(idx));
        if let Some(job) = selected_job {
            let mut details_text = Vec::new();

            details_text.push(Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.name.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Repo: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.repo.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Run ID: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.run_id.to_string()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::LightBlue)),
                Span::styled(
                    job.status.clone(),
                    match job.status.as_str() {
                        "completed" => Style::default().fg(Color::Green),
                        "in_progress" => Style::default().fg(Color::Yellow),
                        "queued" | "waiting" => Style::default().fg(Color::DarkGray),
                        _ => Style::default().fg(Color::White),
                    },
                ),
            ]));
            if let Some(conclusion) = &job.conclusion {
                details_text.push(Line::from(vec![
                    Span::styled("Conclusion: ", Style::default().fg(Color::LightBlue)),
                    Span::styled(
                        conclusion.clone(),
                        match conclusion.as_str() {
                            "success" => Style::default().fg(Color::LightGreen),
                            "failure" => Style::default().fg(Color::Red),
                            "cancelled" => Style::default().fg(Color::DarkGray),
                            "skipped" => Style::default().fg(Color::Blue),
                            _ => Style::default().fg(Color::White),
                        },
                    ),
                ]));
            }
            details_text.push(Line::from(vec![
                Span::styled("Branch: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.head_branch.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("Actor: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.actor_login.clone()),
            ]));
            details_text.push(Line::from(vec![
                Span::styled("URL: ", Style::default().fg(Color::LightBlue)),
                Span::raw(job.html_url.clone()).add_modifier(Modifier::UNDERLINED),
            ]));

            let paragraph = Paragraph::new(details_text).wrap(Wrap { trim: false });
            paragraph.render(inner_area, buf);
        } else {
            let no_job_selected_text = Text::styled(
                "No job selected. Select a job in the main view before toggling detailed view.",
                Style::default().fg(Color::DarkGray),
            );
            let paragraph = Paragraph::new(no_job_selected_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });
            paragraph.render(inner_area, buf);
        }
    }

    /// Returns the original index into `self.job_details` for the currently
    /// selected job in the UI, or None if no job is selected or the index is out of bounds.
    pub fn get_selected_job_original_index(&self) -> Option<usize> {
        let (job_indices_for_current_column, _) = self.get_current_column_data();

        if job_indices_for_current_column.is_empty() {
            return None;
        }

        let mut visual_job_counter = 0;
        for (_tool_name, indices_in_group) in job_indices_for_current_column.iter() {
            for &original_job_idx in indices_in_group {
                if visual_job_counter == self.app_state.row_index {
                    return Some(original_job_idx);
                }
                visual_job_counter += 1;
            }
        }
        None // No job found at the current row_index
    }

    /// Helper to get the job indices and color for the currently selected column.
    /// This avoids duplicating logic in get_selected_job_original_index and render_job_list_column.
    fn get_current_column_data(&self) -> (&BTreeMap<String, Vec<usize>>, Color) {
        match self.app_state.column_index {
            0 => (&self.app_state.in_progress_jobs, Color::Yellow),
            1 => (&self.app_state.success_jobs, Color::Green),
            2 => (&self.app_state.failure_jobs, Color::Red),
            _ => (&self.app_state.in_progress_jobs, Color::White), // Should not happen
        }
    }
}
