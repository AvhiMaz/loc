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

struct Syntax {
    line: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    nested: bool,
    // (delimiter, may span lines)
    quotes: &'static [(&'static str, bool)],
    // 'x' is a char literal, not a string (rust, c, go)
    char_lits: bool,
}

const C_LIKE: Syntax = Syntax {
    line: &["//"],
    block: Some(("/*", "*/")),
    nested: false,
    quotes: &[("\"", false)],
    char_lits: true,
};

const HASH: Syntax = Syntax {
    line: &["#"],
    block: None,
    nested: false,
    quotes: &[("\"", false), ("'", false)],
    char_lits: false,
};

fn syntax_for(ext: &str) -> Option<Syntax> {
    let s = match ext {
        "rs" => Syntax { nested: true, quotes: &[("\"", true)], ..C_LIKE },
        "c" | "h" | "cpp" | "hpp" | "cc" => C_LIKE,
        "go" => Syntax { quotes: &[("\"", false), ("`", true)], ..C_LIKE },
        "js" | "jsx" | "ts" | "tsx" => Syntax {
            quotes: &[("\"", false), ("'", false), ("`", true)],
            char_lits: false,
            ..C_LIKE
        },
        "sol" => Syntax { quotes: &[("\"", false), ("'", false)], char_lits: false, ..C_LIKE },
        "css" => Syntax { line: &[], quotes: &[("\"", false), ("'", false)], char_lits: false, ..C_LIKE },
        "py" | "toml" => Syntax {
            quotes: &[("\"\"\"", true), ("'''", true), ("\"", false), ("'", false)],
            ..HASH
        },
        "sh" | "yaml" | "yml" => HASH,
        "html" | "md" => Syntax {
            line: &[],
            block: Some(("<!--", "-->")),
            nested: false,
            quotes: &[],
            char_lits: false,
        },
        _ => return None,
    };
    Some(s)
}

enum State {
    Code,
    Block(usize),
    Str(&'static str, bool),
}

// returns true if the line has anything outside a comment
fn has_code(line: &[u8], syn: &Syntax, state: &mut State) -> bool {
    let mut code = false;
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        match state {
            State::Block(depth) => {
                let (open, close) = syn.block.unwrap();
                if syn.nested && rest.starts_with(open.as_bytes()) {
                    *depth += 1;
                    i += open.len();
                } else if rest.starts_with(close.as_bytes()) {
                    *depth -= 1;
                    if *depth == 0 {
                        *state = State::Code;
                    }
                    i += close.len();
                } else {
                    i += 1;
                }
            }
            State::Str(q, _) => {
                code = true;
                if line[i] == b'\\' {
                    i += 2;
                } else if rest.starts_with(q.as_bytes()) {
                    i += q.len();
                    *state = State::Code;
                } else {
                    i += 1;
                }
            }
            State::Code => {
                if line[i].is_ascii_whitespace() {
                    i += 1;
                } else if syn.line.iter().any(|l| rest.starts_with(l.as_bytes())) {
                    break;
                } else if let Some((open, _)) = syn.block.filter(|(o, _)| rest.starts_with(o.as_bytes())) {
                    *state = State::Block(1);
                    i += open.len();
                } else if let Some(&(q, multi)) = syn.quotes.iter().find(|(q, _)| rest.starts_with(q.as_bytes())) {
                    code = true;
                    *state = State::Str(q, multi);
                    i += q.len();
                } else if syn.char_lits && line[i] == b'\'' {
                    code = true;
                    if rest.get(1) == Some(&b'\\') {
                        // escaped char like '\'' or '\n': skip to the closing quote
                        let close = rest.iter().skip(3).position(|&c| c == b'\'');
                        i += close.map_or(rest.len(), |p| p + 4);
                    } else if rest.get(2) == Some(&b'\'') {
                        i += 3;
                    } else {
                        // lifetime or multibyte char
                        i += 1;
                    }
                } else {
                    code = true;
                    i += 1;
                }
            }
        }
    }
    if let State::Str(_, false) = state {
        *state = State::Code;
    }
    code
}

fn count_code_lines(path: &Path, syn: &Syntax) -> usize {
    let Ok(bytes) = fs::read(path) else { return 0 };
    let mut state = State::Code;
    let mut lines = bytes.split(|&b| b == b'\n').peekable();
    let mut n = 0;
    while let Some(line) = lines.next() {
        // trailing newline doesn't start a new line
        if lines.peek().is_none() && line.is_empty() {
            break;
        }
        let blank = line.iter().all(|b| b.is_ascii_whitespace());
        let in_comment = matches!(state, State::Block(_));
        if (blank && !in_comment) || has_code(line, syn, &mut state) {
            n += 1;
        }
    }
    n
}

fn scan_repo(repo: &Path, no_comments: bool) -> usize {
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
        .map(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            match syntax_for(&ext) {
                Some(syn) if no_comments => count_code_lines(&p, &syn),
                _ => count_lines(&p),
            }
        })
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

const USAGE: &str = "usage: loc [path] [--no-comments]";

struct Args {
    root: PathBuf,
    no_comments: bool,
}

fn parse_args() -> Args {
    let mut root = None;
    let mut no_comments = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--no-comments" => no_comments = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            a if a.starts_with('-') => {
                eprintln!("unknown flag: {a}\n{USAGE}");
                std::process::exit(1);
            }
            _ => root = Some(PathBuf::from(arg)),
        }
    }
    let root = root.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    Args { root, no_comments }
}

fn main() {
    let Args { root, no_comments } = parse_args();

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
            let loc = scan_repo(r, no_comments);
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
    let title = if no_comments { "lines of code (no comments)" } else { "lines of code" };
    println!("  {BOLD}{WHITE}{title}{RESET}  {DIM}{GRAY}{label}{RESET}");
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
