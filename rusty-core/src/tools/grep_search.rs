use anyhow::Result;
use ignore::{WalkBuilder, types::TypesBuilder};
use regex::Regex;
use std::path::Path;

pub async fn grep_search(
    workspace_root: &str,
    pattern: String,
    path: Option<String>,
    max_results: Option<usize>,
    file_extension: Option<String>,
) -> Result<String> {
    let max = max_results.unwrap_or(30).min(100);
    let search_path = path.unwrap_or_else(|| "/".to_string());
    let root = Path::new(workspace_root);
    let target = root.join(search_path.trim_start_matches('/'));

    // Safety
    if !target.starts_with(root) {
        return Ok("ERROR: Path traversal blocked.".to_string());
    }
    if !target.exists() {
        return Ok(format!("ERROR: Path not found: {}", search_path));
    }

    let re = Regex::new(&pattern)
        .map_err(|e| anyhow::anyhow!("Invalid regex pattern '{}': {}", pattern, e))?;

    let mut walker = WalkBuilder::new(&target);
    walker.standard_filters(true); // respects .gitignore, .ignore, etc.

    if let Some(ext) = file_extension {
        let mut tb = TypesBuilder::new();
        tb.add("custom", &format!("*.{}", ext)).unwrap();
        tb.select("custom");
        walker.types(tb.build().unwrap());
    }

    let mut output = format!("# Grep Search: `{}` (max {} results)\n\n", pattern, max);
    let mut count = 0;

    for entry in walker.build() {
        if count >= max {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if p.is_dir() {
            continue;
        }

        let content = match tokio::fs::read_to_string(p).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                let rel_path = p.strip_prefix(root).unwrap_or(p).to_string_lossy();
                output.push_str(&format!("`{}`:{}: {}\n", rel_path, i + 1, line.trim()));
                count += 1;
                if count >= max {
                    break;
                }
            }
        }
    }

    if count == 0 {
        output.push_str("No matches found.");
    } else if count == max {
        output.push_str("\n... (truncated — increase max_results or narrow path)");
    }

    Ok(output)
}
