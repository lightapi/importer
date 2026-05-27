use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn read_to_string(path: &str) -> Result<String> {
    if path == "-" {
        let mut input = String::new();
        let mut stdin = tokio::io::stdin();
        stdin.read_to_string(&mut input).await?;
        Ok(input)
    } else {
        Ok(tokio::fs::read_to_string(path).await?)
    }
}

pub async fn write_string(path: Option<&str>, content: &str) -> Result<()> {
    match path {
        Some(path) if path != "-" => {
            tokio::fs::write(path, content).await?;
        }
        _ => {
            let mut stdout = tokio::io::stdout();
            stdout.write_all(content.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

pub async fn read_rule_arg(value: &str) -> Result<String> {
    if let Some(path) = value.strip_prefix('@') {
        read_to_string(path).await
    } else {
        Ok(value.to_string())
    }
}
