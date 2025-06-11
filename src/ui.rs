use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};
use std::collections::BTreeMap; // Using BTreeMap for sorted group keys

use crate::app::App; // Assuming App struct is defined here

impl Widget for &App {
    /// Renders the user interface widgets.
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Define the main layout to split the screen vertically
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7), // Fixed height for header/instructions
                Constraint::Min(0), // Remaining space for job details columns or columns + details
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
                Use `Left`/`Right` to navigate columns, `Up`/`Down` for rows.\n\
                Press `Enter` to toggle job details. Auto-refresh every 5 seconds.",
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

        // --- Render the job columns and potentially the details panel ---
        if self.app_state.show_details {
            // Split the bottom chunk for columns and details panel
            let app_body_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(70), // 70% for job columns
                    Constraint::Percentage(30), // 30% for details panel
                ])
                .split(main_chunks[1]);

            self.render_job_columns(app_body_chunks[0], buf);
            self.render_job_details_panel(app_body_chunks[1], buf);
        } else {
            // Just render columns if details are not shown
            self.render_job_columns(main_chunks[1], buf);
        }
    }
}

impl App {
    // Helper to get the tool from a job name.
    // This is the common parsing logic used for grouping and display.
    fn parse_job_name_for_tool(&self, job_name: &str) -> String {
        let parts: Vec<&str> = job_name.split(" / ").collect();
        parts.get(0).unwrap_or(&"Other").to_string()
    }

    // This new function manages the three-column layout
    fn render_job_columns(&self, area: Rect, buf: &mut Buffer) {
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
            &self.app_state.in_progress_jobs,
            Color::Yellow, // Color for in progress
            0,             // Column index for 'In Progress'
        );

        self.render_job_list_column(
            columns[1],
            buf,
            "Concluded Success",
            &self.app_state.success_jobs,
            Color::Green, // Color for success
            1,            // Column index for 'Concluded Success'
        );

        self.render_job_list_column(
            columns[2],
            buf,
            "Concluded Failure",
            &self.app_state.failure_jobs,
            Color::Red, // Color for failure
            2,          // Column index for 'Concluded Failure'
        );
    }

    // Reusable function to render a single column of job summaries with grouping and scrolling
    fn render_job_list_column(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: &str,
        job_indices: &[usize], // Now takes a slice of original indices
        border_color: Color,
        column_idx: usize, // New parameter for column index
    ) {
        let is_selected_column = self.app_state.column_index == column_idx;

        let block =
            Block::default()
                .title(format!("{} ({})", title, job_indices.len()))
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
        let mut grouped_jobs: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for &original_idx in job_indices {
            let job = &self.job_details[original_idx];
            let tool = self.parse_job_name_for_tool(&job.name);
            grouped_jobs.entry(tool).or_default().push(original_idx);
        }

        let available_height = inner_area.height as usize;
        let mut all_column_lines: Vec<Line> = Vec::new(); // Collect all lines first

        let mut current_column_job_idx = 0; // Tracks the sequential index of jobs within the column (ignoring groups)

        // Iterate through grouped jobs to build all lines, including group headers
        for (tool_name, indices_in_group) in grouped_jobs.iter() {
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

                // --- Robust Parsing of the job name for display ---
                let (action_part, tool_workflow_service_parts_for_display) =
                    if let Some((before_dash, after_dash)) = job.name.split_once(" - ") {
                        (after_dash, before_dash.split(" / ").collect::<Vec<&str>>())
                    } else {
                        (
                            job.name.as_str(),
                            job.name.split(" / ").collect::<Vec<&str>>(),
                        )
                    };

                // Skip the first part (the tool) as it's handled by the group header
                let workflow_part = tool_workflow_service_parts_for_display
                    .get(1)
                    .unwrap_or(&"");
                let service_or_sub_service_part = tool_workflow_service_parts_for_display
                    .get(2)
                    .unwrap_or(&"");

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

                // Line 3: Service/Sub-service (conditionally displayed and indented)
                if !service_or_sub_service_part.is_empty() {
                    all_column_lines.push(Line::from(vec![Span::styled(
                        format!("    {}", service_or_sub_service_part),
                        base_style.fg(Color::White).add_modifier(Modifier::ITALIC),
                    )]));
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

                // Add separator if it's not the last job overall, and there's space
                // No separator if it's the very last job.
                // We're adding separator AFTER a job, so the last job will NOT get a separator.
                // Also, we don't want a separator at the very end of a group if the next group starts immediately.
                // For simplicity, we add it after each job, and the group header adds its own below itself.
                all_column_lines.push(Line::from(Span::styled(
                    "---",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        // --- Apply Scrolling Offset ---
        // Get the relevant scroll_offset for this column. You might store this
        // per-column in app_state if each column scrolls independently.
        // For now, let's assume scroll_offset applies to the currently selected column.
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

        // Optional: Render a scrollbar if the content overflows.
        // This requires an additional widget (e.g., from tui-big-text or custom)
        // or a manual buffer manipulation, which is outside the scope of this specific change.
        // For now, the content simply scrolls.
    }

    fn render_job_details_panel(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("Job Details")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let selected_job = self.job_details.get(self.current_job_index);

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
                "No job selected. Use navigation keys to select a job.",
                Style::default().fg(Color::DarkGray),
            );
            let paragraph = Paragraph::new(no_job_selected_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });
            paragraph.render(inner_area, buf);
        }
    }
}
