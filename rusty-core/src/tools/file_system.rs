use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use tokio::task;
// use tracing::debug;

const MAX_READ_LINES: usize = 2000;

// Reads a file from the workspace, optionally limited to a line range.
// workspace_root: /workspace inside the container
// file_path: relative to the root, first actual argument
// start_line: 1-based inclusive, default 1
// end_line: 1-based inclusive, default EOF (but, max-lines of 200?)
// returns content with line number prefixes

pub async fn read_file(
    workspace_root: &str,
    file_path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<String> {
    let workspace_root = workspace_root.to_string();
    task::spawn_blocking(move || {
        let full_path = Path::new(&workspace_root).join(&file_path);

        // Don't allow escaping the workspace root
        if !full_path.starts_with(workspace_root) {
            return Ok(format!(
                "ERROR: Path traversal attempt blocked.\nPath: {}\nSuggestion: Use only relative paths from the repository root (e.g. 'src/main.rs'). Try list_directory first.",
                file_path
            ));
        }

        let file = match File::open(&full_path) {
            Ok(f) => f,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    return Ok(format!(
                        "ERROR: File not found: {}\nSuggestion: Use list_directory or find_files to discovery the correct path first.",
                        file_path
                    ));
                } else {
                    return Ok(format!(
                        "ERROR: Cannot open file: {}\nSuggestion: Try a different file.",
                        file_path,
                    ));
                }
            }
        };

        let reader = BufReader::new(file);
        let lines: Vec<String> = match reader.lines().collect::<Result<_, _>>() {
            Ok(l) => l,
            Err(e) => {
                return Ok(format!(
                    "ERROR: Failed to read lines from {}\nSuggestion: The file may be binary or too large - try a different approach.",
                    file_path,
                ));
            }
        };

        let total_lines = lines.len();

        let start = start_line.unwrap_or(1).max(1).min(total_lines);

        let end_line_hydrated = end_line.unwrap_or(total_lines);
        let end_line_bounded = end_line_hydrated.min(start + MAX_READ_LINES);

        let end = end_line_bounded.min(total_lines);

        if start > end {
            return Ok(format!(
                "ERROR: Empty range: lines {}-{} of {} in file {}\n(no lines returned)",
                start, end, total_lines, file_path
            ));
        }

        let selected = &lines[start.saturating_sub(1)..end];
        let numbered: Vec<String> = selected
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>6} │ {}", start + i, line))
            .collect();

        // were the bounds t
        let is_truncated = end_line_hydrated < end_line_bounded;
        let header = format!(
            "=== File: {} (lines {}-{} of {}) ===",
            file_path, start, end, total_lines
        );
        let body = numbered.join("\n");
        let footer = if end == total_lines {
            "=== End of file ===".to_string()
        } else if is_truncated {
            format!(
                "=== TRUNCATED: reached agent limit of {} lines ===\nTo continue, call read_file with start_line={}",
                MAX_READ_LINES,
                end + 1
            )
        } else {
            "=== End of requested range ===".to_string()
        };

        Ok(format!("{}\n{}\n{}", header, body, footer))
    })
    .await?
    // .or_else("ERROR: Unable to read file.".to_string())
}
