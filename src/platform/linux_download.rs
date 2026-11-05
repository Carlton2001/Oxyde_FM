//! HTTPS GET through curl, or wget when curl is missing: the tools install.sh itself
//! needs, which follow the user's proxy variables and the system's certificates.
use std::io::{self, Read};
use std::process::{Command, Stdio};

const USER_AGENT: &str = concat!("Oxyde/", env!("CARGO_PKG_VERSION"));

/// Download `url` into memory, refusing bodies larger than `limit` bytes.
pub(super) fn download(url: &str, limit: usize) -> io::Result<Vec<u8>> {
    if !url.starts_with("https://") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Only HTTPS downloads are allowed"));
    }
    match run(curl(url, limit), limit) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => run(wget(url), limit).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(io::ErrorKind::NotFound, "curl or wget is needed to download updates")
            } else {
                error
            }
        }),
        result => result,
    }
}

fn curl(url: &str, limit: usize) -> Command {
    let mut command = Command::new("curl");
    // -q comes first so that ~/.curlrc is ignored; redirects stay on HTTPS.
    command.args(["-q", "--silent", "--show-error", "--fail", "--location", "--proto", "=https",
        "--proto-redir", "=https", "--tlsv1.2", "--connect-timeout", "15", "--max-time", "600",
        "--user-agent", USER_AGENT, "--max-filesize"])
        .arg(limit.to_string())
        .arg(url);
    command
}

fn wget(url: &str) -> Command {
    let mut command = Command::new("wget");
    command.args(["--quiet", "--https-only", "--max-redirect=5", "--timeout=15", "--output-document=-"])
        .arg(format!("--user-agent={USER_AGENT}"))
        .arg(url);
    command
}

fn run(mut command: Command, limit: usize) -> io::Result<Vec<u8>> {
    let tool = command.get_program().to_string_lossy().into_owned();
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let mut body = Vec::new();
    let read = child.stdout.take().expect("piped stdout").take(limit as u64 + 1).read_to_end(&mut body);
    if body.len() > limit {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::new(io::ErrorKind::FileTooLarge, "The download exceeds its size limit"));
    }
    let output = child.wait_with_output()?;
    read?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let message = message.lines().rev().map(str::trim).find(|line| !line.is_empty())
            .map_or_else(|| format!("{tool} failed with {}", output.status), |line| format!("{tool}: {line}"));
        return Err(io::Error::other(message));
    }
    Ok(body)
}

#[cfg(test)]
#[path = "linux_download/tests/download.rs"]
mod tests;
