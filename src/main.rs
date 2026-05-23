use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[38;5;87m";
const PURPLE: &str = "\x1b[38;5;141m";
const GREEN: &str = "\x1b[38;5;120m";
const GOLD: &str = "\x1b[38;5;220m";
const GRAY: &str = "\x1b[38;5;240m";
const WHITE: &str = "\x1b[38;5;255m";

const EXT_MAP: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "toml",
    "json", "md", "html", "css", "sh", "sol", "go", "yaml", "yml",
    "c", "h", "cpp", "hpp", "cc",
];

const ALWAYS_SKIP: &[&str] = &[
    "package-lock.json", "yarn.lock", "Cargo.lock", "pnpm-lock.yaml",
];

fn tracked_files(repo: &Path) -> Vec<PathBuf> {
    let Ok(out) = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(repo)
        .output()
    else {
        return vec![];
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| repo.join(l))
        .collect()
}

fn count_lines(path: &Path) -> usize {
    let Ok(f) = fs::File::open(path) else { return 0 };
    BufReader::new(f).lines().count()
}

fn scan_repo(repo: &Path) -> usize {
    tracked_files(repo)
        .into_iter()
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if ALWAYS_SKIP.contains(&name) {
                return false;
            }
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            EXT_MAP.contains(&ext.as_str())
        })
        .map(|p| count_lines(&p))
        .sum()
}

fn find_repos(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else { return vec![] };
    let mut repos: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            p.is_dir()
                && !name.starts_with('.')
                && p.join(".git").exists()
        })
        .collect();
    repos.sort();
    repos
}

fn bar(n: usize, max: usize, width: usize) -> String {
    let filled = (width * n).checked_div(max).unwrap_or(0).min(width);
    format!(
        "{CYAN}{}{GRAY}{}{RESET}",
        "█".repeat(filled),
        "░".repeat(width - filled),
    )
}

fn fmt_loc(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn root_dir() -> PathBuf {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        return PathBuf::from(arg);
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let root = root_dir();

    let repos = if root.join(".git").exists() {
        vec![root.clone()]
    } else {
        let found = find_repos(&root);
        if found.is_empty() { vec![root.clone()] } else { found }
    };

    let label = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".");

    let mut results: Vec<(String, usize)> = repos
        .iter()
        .filter_map(|r| {
            let loc = scan_repo(r);
            if loc > 0 {
                let name = if r == &root {
                    label.to_string()
                } else {
                    r.file_name()?.to_str()?.to_string()
                };
                Some((name, loc))
            } else {
                None
            }
        })
        .collect();

    results.sort_by_key(|b| std::cmp::Reverse(b.1));

    let grand: usize = results.iter().map(|(_, n)| n).sum();
    let max_loc = results.first().map(|(_, n)| *n).unwrap_or(1);
    let name_w = results.iter().map(|(n, _)| n.len()).max().unwrap_or(10);
    let sep_w = name_w + 38;

    println!();
    println!("  {BOLD}{WHITE}lines of code{RESET}  {DIM}{GRAY}{label}{RESET}");
    println!("  {GRAY}{}{RESET}", "─".repeat(sep_w));
    println!();

    for (name, loc) in &results {
        let pct = *loc as f64 / grand as f64 * 100.0;
        println!(
            "  {PURPLE}{name:<name_w$}{RESET}  {}  {GREEN}{BOLD}{:>7}{RESET}  {DIM}{GRAY}{:>5.1}%{RESET}",
            bar(*loc, max_loc, 20),
            fmt_loc(*loc),
            pct,
        );
    }

    println!();
    println!("  {GRAY}{}{RESET}", "─".repeat(sep_w));
    println!(
        "  {DIM}{GRAY}{:<name_w$}{RESET}  {}  {GOLD}{BOLD}{:>7}{RESET}",
        "total",
        " ".repeat(22),
        fmt_loc(grand),
    );
    println!();
}
