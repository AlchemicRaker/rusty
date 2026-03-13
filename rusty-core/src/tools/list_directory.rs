use anyhow::Result;
use ignore::WalkBuilder;
use std::path::Path;

pub async fn list_directory(
    workspace_root: &str,
    path: String,
    max_depth: Option<usize>,
    include_hidden: Option<bool>,
) -> Result<String> {
    let root = Path::new(workspace_root);
    let target = root.join(&path);

    // Safety: prevent traversal outside workspace
    if !target.starts_with(root) {
        return Ok(format!(
            "ERROR: Path traversal blocked. Use only paths relative to workspace root (e.g. 'src/')."
        ));
    }

    if !target.exists() {
        return Ok(format!("ERROR: Path not found: {}", path));
    }
    if !target.is_dir() {
        return Ok(format!("ERROR: Not a directory: {}", path));
    }

    let depth = max_depth.unwrap_or(3).min(10);
    let show_hidden = include_hidden.unwrap_or(false);

    let walker = WalkBuilder::new(&target)
        .max_depth(Some(depth))
        .hidden(!show_hidden) // true = skip hidden
        .standard_filters(true) // gitignore etc.
        .build();

    let mut output = format!("# Directory Listing: {}\n", path);

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let full_path = entry.path();
        if full_path == target {
            continue;
        } // skip root

        let rel_path = full_path.strip_prefix(&target).unwrap_or(full_path);
        let name = rel_path.to_string_lossy();

        // Calculate indent level
        let level = rel_path.components().count().saturating_sub(1);
        let prefix = "  ".repeat(level);

        let typ = if entry.file_type().map_or(false, |t| t.is_dir()) {
            format!("{}/", name)
        } else {
            format!("{} (file)", name)
        };

        output.push_str(&format!("{}- {}\n", prefix, typ));
    }

    if output.lines().count() > 200 {
        output.push_str("\n... (truncated - increase max_depth or narrow path)");
    }

    Ok(output)
}
