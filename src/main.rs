use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

// Custom error types for better error handling
#[derive(Debug)]
enum InstallError {
    DistributionDetectionError(String),
    ArchitectureDetectionError(String),
    DownloadError(String),
    SudoError(String),
    InstallationError(String),
    IOError(std::io::Error),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InstallError::DistributionDetectionError(err) => {
                write!(f, "Distribution detection error: {}", err)
            }
            InstallError::ArchitectureDetectionError(err) => {
                write!(f, "Architecture detection error: {}", err)
            }
            InstallError::DownloadError(err) => write!(f, "Download error: {}", err),
            InstallError::SudoError(err) => write!(f, "Sudo error: {}", err),
            InstallError::InstallationError(err) => write!(f, "Installation error: {}", err),
            InstallError::IOError(err) => write!(f, "IO error: {}", err),
        }
    }
}

impl Error for InstallError {}

impl From<std::io::Error> for InstallError {
    fn from(err: std::io::Error) -> Self {
        InstallError::IOError(err)
    }
}

fn main() {
    match check_wazuh_installed() {
        Ok(installed) => {
            if installed {
                println!("Wazuh agent is already installed.");
            } else {
                println!("Wazuh agent is not installed. Installing...");
                if let Err(e) = install_wazuh_agent() {
                    eprintln!("Failed to install Wazuh agent: {}", e);
                } else {
                    println!("Wazuh agent installed successfully.");
                }
            }
        }
        Err(e) => eprintln!("Error checking Wazuh agent installation: {}", e),
    }
}

fn check_wazuh_installed() -> Result<bool, InstallError> {
    let wazuhctl_result = Command::new("which").arg("wazuhctl").output();

    if wazuhctl_result.is_err() {
        return Ok(false);
    }

    let wazuhctl_installed = wazuhctl_result.unwrap().status.success();

    Ok(wazuhctl_installed)
}

fn install_wazuh_agent() -> Result<(), InstallError> {
    // Determine Linux distribution and version
    let (distribution, version) = get_distribution_and_version()?;
    let architecture = get_architecture()?;

    let package_url = format!(
        "https://packages.wazuh.com/4.x/{}/{}/{}/{}",
        distribution, version, architecture, get_package_name(&distribution, &architecture)
    );

    let package_extension = get_package_extension(&distribution);
    let package_path = Path::new("/tmp/").join(format!("wazuh-agent.{}", package_extension));

    // Check for curl
    if Command::new("curl").output().is_err() {
        return Err(InstallError::DownloadError("Curl is not installed.".to_string()));
    }

    let download_result = Command::new("curl")
        .args(&["-L", &package_url, "-o", package_path.to_str().unwrap()])
        .status();

    if download_result.is_err() || !download_result.unwrap().success() {
        return Err(InstallError::DownloadError(
            "Failed to download the Wazuh agent package.".to_string(),
        ));
    }

    let sudo_check = Command::new("sudo").arg("-v").output();
    if sudo_check.is_err() || !sudo_check.unwrap().status.success() {
        return Err(InstallError::SudoError(
            "Sudo privileges are required for installation.".to_string(),
        ));
    }

    let install_command = if package_extension == "deb" {
        "dpkg -i"
    } else {
        "rpm -Uvh"
    };

    let install_status = Command::new("sudo")
        .args(&[install_command, package_path.to_str().unwrap()])
        .status();
    if install_status.is_err() || !install_status.unwrap().success() {
        return Err(InstallError::InstallationError(
            "Failed to install Wazuh agent package.".to_string(),
        ));
    }

    // Attempt to clean up the downloaded package regardless of installation success
    let _ = fs::remove_file(&package_path);

    Ok(())
}

fn get_distribution_and_version() -> Result<(&'static str, &'static str), InstallError> {
    let etc_release_content = fs::read_to_string("/etc/os-release")
        .map_err(|_| InstallError::DistributionDetectionError("Failed to read /etc/os-release".to_string()))?;

    let etc_release_content = Box::leak(etc_release_content.into_boxed_str());

    let mut distribution = "";
    let mut version = "";

    for line in etc_release_content.lines() {
        if line.starts_with("ID=") {
            distribution = line.split('=').nth(1).unwrap_or("").trim_matches('"');
        } else if line.starts_with("VERSION_ID=") {
            version = line.split('=').nth(1).unwrap_or("").trim_matches('"');
        }
    }

    match (distribution, version) {
        ("alpine", _) => Ok(("alpine", version)),
        ("amazon", _) => Ok(("amazon", "latest")),
        ("centos", _) => Ok(("centos", version)),
        ("debian", _) => Ok(("debian", version)),
        ("fedora", _) => Ok(("fedora", version)),
        ("opensuse", _) => Ok(("opensuse", version)),
        ("oracle", _) => Ok(("oracle", version)),
        ("redhat", _) => Ok(("redhat", version)),
        ("suse", _) => Ok(("suse", version)),
        ("ubuntu", _) => Ok(("ubuntu", version)),
        ("raspbian", _) => Ok(("raspbian", version)),
        (_, _) => Err(InstallError::DistributionDetectionError(
            "Unsupported distribution".to_string(),
        )),
    }
}

fn get_architecture() -> Result<&'static str, InstallError> {
    if cfg!(target_arch = "x86") {
        Ok("i386")
    } else if cfg!(target_arch = "x86_64") {
        Ok("x86_64")
    } else if cfg!(target_arch = "aarch64") {
        Ok("aarch64")
    } else if cfg!(target_arch = "arm") {
        Ok("armhf")
    } else if cfg!(target_arch = "powerpc64") {
        Ok("powerpc")
    } else {
        Err(InstallError::ArchitectureDetectionError(
            "Unsupported architecture".to_string(),
        ))
    }
}

fn get_package_name(distribution: &str, architecture: &str) -> String {
    match (distribution, architecture) {
        ("alpine", _) => "wazuh-agent-4.7.3-r1.apk".to_string(),
        ("amazon", _) => "wazuh-agent-4.7.3-1.ppc64le.rpm".to_string(),
        ("centos", "5") | ("oracle", "5") => "wazuh-agent-4.7.3-1.el5.x86_64.rpm".to_string(),
        ("centos", _) => "wazuh-agent-4.7.3-1.x86_64.rpm".to_string(),
        ("debian", _) => "wazuh-agent_4.7.3-1_amd64.deb".to_string(),
        ("fedora", _) => "wazuh-agent-4.7.3-1.x86_64.rpm".to_string(),
        ("opensuse", _) => "wazuh-agent-4.7.3-1.x86_64.rpm".to_string(),
        ("oracle", _) => "wazuh-agent-4.7.3-1.x86_64.rpm".to_string(),
        ("redhat", "5") => "wazuh-agent-4.7.3-1.el5.x86_64.rpm".to_string(),
        ("redhat", _) => "wazuh-agent-4.7.3-1.x86_64.rpm".to_string(),
        ("suse", "11") => "wazuh-agent-4.7.3-1.el5.x86_64.rpm".to_string(),
        ("suse", _) => "wazuh-agent-4.7.3-1.x86_64.rpm".to_string(),
        ("ubuntu", _) => "wazuh-agent_4.7.3-1_amd64.deb".to_string(),
        ("raspbian", _) => "wazuh-agent_4.7.3-1_armhf.deb".to_string(),
        _ => unreachable!(),
    }
}

fn get_package_extension(distribution: &str) -> String {
    match distribution {
        "alpine" => "apk".to_string(),
        _ => "rpm".to_string(),
    }
}