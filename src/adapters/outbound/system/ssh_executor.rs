use async_trait::async_trait;
use tokio::process::Command;

use crate::domain::{
    errors::DomainError,
    server::{Server, ServerMetrics, ServerSpec, ServiceAction, ServiceStatus},
};
use crate::ports::outbound::server_executor::{CommandResult, ServerExecutorPort};

pub struct SshServerExecutor;

impl SshServerExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SshServerExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServerExecutorPort for SshServerExecutor {
    async fn ping(&self, server: &Server) -> Result<bool, DomainError> {
        let output = Command::new("nc")
            .arg("-z")
            .arg("-w")
            .arg("2")
            .arg(server.host().ip())
            .arg(server.host().port().to_string())
            .output()
            .await;

        match output {
            Ok(res) => Ok(res.status.success()),
            Err(_) => Ok(false),
        }
    }

    async fn execute_command(&self, server: &Server, command: &str) -> Result<CommandResult, DomainError> {
        let output = Command::new("ssh")
            .arg("-p")
            .arg(server.host().port().to_string())
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg(format!("root@{}", server.host().ip()))
            .arg(command)
            .output()
            .await
            .map_err(|e| DomainError::ExecutionFailed {
                reason: "Failed to spawn ssh process".to_string(),
                details: Some(e.to_string()),
            })?;

        Ok(CommandResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn inspect_system(&self, server: &Server) -> Result<ServerSpec, DomainError> {
        let res = self.execute_command(server, "uname -s && uname -m && nproc").await?;
        if res.exit_code != 0 {
            return Err(DomainError::ExecutionFailed {
                reason: "Failed to inspect remote system".to_string(),
                details: Some(res.stderr),
            });
        }

        let lines: Vec<&str> = res.stdout.lines().collect();
        let os = lines.first().unwrap_or(&"Linux").to_string();
        let arch = lines.get(1).unwrap_or(&"x86_64").to_string();
        let cpu_cores = lines
            .get(2)
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(2);

        Ok(ServerSpec {
            os,
            arch,
            cpu_cores,
            ram_mb: 8192,
        })
    }

    async fn manage_system_service(
        &self,
        server: &Server,
        service: &str,
        action: ServiceAction,
    ) -> Result<ServiceStatus, DomainError> {
        let cmd = format!("systemctl {} {}", action.as_systemd_command(), service);
        let res = self.execute_command(server, &cmd).await?;
        if res.exit_code == 0 {
            Ok(ServiceStatus::Active)
        } else {
            Ok(ServiceStatus::Failed)
        }
    }

    async fn collect_metrics(&self, server: &Server) -> Result<ServerMetrics, DomainError> {
        let cmd = "top -bn1 | grep 'Cpu(s)' | awk '{print $2}' && free -m | awk 'NR==2{print $3,$2}' && df -m / | awk 'NR==2{print $3,$2}' && cat /proc/uptime | awk '{print $1}'";
        let res = self.execute_command(server, cmd).await?;
        if res.exit_code != 0 {
            return Err(DomainError::ExecutionFailed {
                reason: "Failed to collect metrics".to_string(),
                details: Some(res.stderr),
            });
        }

        let lines: Vec<&str> = res.stdout.lines().collect();
        let cpu_pct = lines.first().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(15.0);
        let (ram_used, ram_total) = lines
            .get(1)
            .map(|s| {
                let parts: Vec<&str> = s.split_whitespace().collect();
                let u = parts.first().and_then(|x| x.parse::<u64>().ok()).unwrap_or(2048);
                let t = parts.get(1).and_then(|x| x.parse::<u64>().ok()).unwrap_or(8192);
                (u, t)
            })
            .unwrap_or((2048, 8192));

        let (disk_used, disk_total) = lines
            .get(2)
            .map(|s| {
                let parts: Vec<&str> = s.split_whitespace().collect();
                let u = parts.first().and_then(|x| x.parse::<f32>().ok()).unwrap_or(10000.0) / 1024.0;
                let t = parts.get(1).and_then(|x| x.parse::<f32>().ok()).unwrap_or(50000.0) / 1024.0;
                (u, t)
            })
            .unwrap_or((10.0, 50.0));

        let uptime = lines.get(3).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(3600.0) as u64;

        ServerMetrics::new(cpu_pct, ram_used, ram_total, disk_used, disk_total, uptime, [0.1, 0.2, 0.3])
    }

    async fn reboot(&self, server: &Server) -> Result<(), DomainError> {
        let _ = self.execute_command(server, "systemctl reboot").await;
        Ok(())
    }
}
