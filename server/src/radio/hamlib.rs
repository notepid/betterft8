use anyhow::{anyhow, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

pub struct RigCtld {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
}

impl RigCtld {
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let stream = TcpStream::connect(format!("{}:{}", host, port)).await?;
        let (read_half, write_half) = stream.into_split();
        Ok(RigCtld {
            reader: BufReader::new(read_half),
            writer: write_half,
        })
    }

    async fn send_command(&mut self, cmd: &str) -> Result<Vec<String>> {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.send_command_inner(cmd),
        )
        .await
        .map_err(|_| anyhow!("rigctld command timed out: {}", cmd))?
    }

    async fn send_command_inner(&mut self, cmd: &str) -> Result<Vec<String>> {
        let line = format!("{}\n", cmd);
        self.writer.write_all(line.as_bytes()).await?;
        self.writer.flush().await?;

        let mut lines = Vec::new();
        loop {
            let mut buf = String::new();
            let n = self.reader.read_line(&mut buf).await?;
            if n == 0 {
                return Err(anyhow!("connection closed by rigctld"));
            }
            let trimmed = buf.trim_end_matches(['\n', '\r']).to_string();
            if let Some(code_str) = trimmed.strip_prefix("RPRT ") {
                let code: i32 = code_str.trim().parse().unwrap_or(-1);
                if code == 0 {
                    return Ok(lines);
                } else {
                    return Err(anyhow!("rigctld error code: {}", code));
                }
            }
            lines.push(trimmed);
        }
    }

    pub async fn get_frequency(&mut self) -> Result<u64> {
        let lines = self.send_command("+f").await?;
        for line in &lines {
            if let Some(val) = line.strip_prefix("Frequency: ") {
                return Ok(val.trim().parse()?);
            }
        }
        Err(anyhow!("Frequency not found in response: {:?}", lines))
    }

    pub async fn set_frequency(&mut self, freq: u64) -> Result<()> {
        self.send_command(&format!("+F {}", freq)).await?;
        Ok(())
    }

    pub async fn get_mode(&mut self) -> Result<(String, i32)> {
        let lines = self.send_command("+m").await?;
        let mut mode = String::new();
        let mut passband = 0i32;
        for line in &lines {
            if let Some(val) = line.strip_prefix("Mode: ") {
                mode = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("Passband: ") {
                passband = val.trim().parse().unwrap_or(0);
            }
        }
        if mode.is_empty() {
            return Err(anyhow!("Mode not found in response: {:?}", lines));
        }
        Ok((mode, passband))
    }

    pub async fn set_mode(&mut self, mode: &str, passband: i32) -> Result<()> {
        self.send_command(&format!("+M {} {}", mode, passband)).await?;
        Ok(())
    }

    pub async fn get_ptt(&mut self) -> Result<bool> {
        let lines = self.send_command("+t").await?;
        for line in &lines {
            if let Some(val) = line.strip_prefix("PTT: ") {
                return Ok(val.trim() == "1");
            }
        }
        Err(anyhow!("PTT not found in response: {:?}", lines))
    }

    pub async fn set_ptt(&mut self, on: bool) -> Result<()> {
        self.send_command(&format!("+T {}", if on { 1 } else { 0 })).await?;
        Ok(())
    }
}
