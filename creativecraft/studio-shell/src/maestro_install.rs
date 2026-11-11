//! Installing Maestro on demand — the optional, one-time step.
//!
//! Nothing is bundled with CreativeCraft and nothing is downloaded until the
//! user presses Install. This runs Maestro's own setup as one script, streams its
//! output into the tab and can be stopped: the tools it needs (git, Python 3.11,
//! Node), the repository, the Python environment, the CUDA build of torch, and the
//! React interface. Model weights are *not* downloaded here — Maestro fetches
//! those itself the first time a model is used.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};

/// The setup script, with `{{FOLDER}}` where the install folder goes. A raw
/// string keeps PowerShell's braces readable (no `format!` escaping).
const SCRIPT: &str = r#"$ErrorActionPreference = 'Continue'
$folder = '{{FOLDER}}'
Write-Output "CreativeCraft: setting up Maestro in $folder"

function Have($name) { return [bool](Get-Command $name -ErrorAction SilentlyContinue) }
function Install-WithWinget($id, $label) {
  Write-Output "CreativeCraft: installing $label (winget)..."
  $ok = $false
  if (Have winget) {
    winget install --id $id -e --scope user --accept-package-agreements --accept-source-agreements
    $ok = ($LASTEXITCODE -eq 0)
  }
  if (-not $ok) { Write-Output "CreativeCraft: could not install $label - install it by hand, then press Install / repair again." }
}

# --- tools -------------------------------------------------------------
if (-not (Have git)) { Install-WithWinget "Git.Git" "git" }

$py311 = Join-Path $env:LOCALAPPDATA 'Programs\Python\Python311\python.exe'
if (-not (Test-Path $py311)) {
  $viaLauncher = $false
  if (Get-Command py -ErrorAction SilentlyContinue) {
    & py -3.11 --version *> $null
    $viaLauncher = ($LASTEXITCODE -eq 0)
  }
  if (-not $viaLauncher -and -not (Test-Path $py311)) {
    Install-WithWinget "Python.Python.3.11" "Python 3.11"
  }
}
if (-not (Test-Path $py311)) {
  if (Get-Command py -ErrorAction SilentlyContinue) {
    $candidate = & py -3.11 -c "import sys; print(sys.executable)" 2>$null
    if ($candidate -and (Test-Path $candidate)) { $py311 = $candidate.Trim() }
  }
}
if (-not (Test-Path $py311)) {
  Write-Output 'CreativeCraft: Python 3.11 is still missing. Install it (winget install Python.Python.3.11), then press Install / repair again.'
  exit 1
}

if (-not (Have npm)) {
  Install-WithWinget "OpenJS.NodeJS.LTS" "Node.js (npm)"
  if (-not (Have npm)) {
    foreach ($npm in @("$env:ProgramFiles\nodejs\npm.cmd", "$env:ProgramFiles(x86)\nodejs\npm.cmd")) {
      if (Test-Path $npm) { $env:PATH = (Split-Path $npm) + ';' + $env:PATH; break }
    }
  }
}

# --- repository --------------------------------------------------------
if (-not (Test-Path (Join-Path $folder 'app'))) {
  if (-not (Have git)) { Write-Output 'CreativeCraft: git is missing, so Maestro cannot be cloned.'; exit 1 }
  Write-Output 'CreativeCraft: cloning Maestro...'
  git clone https://github.com/Blizaine/Maestro "$folder"
  if ($LASTEXITCODE -ne 0) { Write-Output 'CreativeCraft: the clone failed (network?). Press Install / repair to try again.'; exit 1 }
}
$app = Join-Path $folder 'app'

# --- Python environment (3.11) ----------------------------------------
$envPy = Join-Path $app 'env\Scripts\python.exe'
if (Test-Path $envPy) {
  $ver = & $envPy -c "import sys; print(str(sys.version_info[0]) + '.' + str(sys.version_info[1]))"
  if ($ver -ne '3.11') {
    Write-Output "CreativeCraft: the existing app\env uses Python $ver; rebuilding it with 3.11..."
    Remove-Item (Join-Path $app 'env') -Recurse -Force
  }
}
if (-not (Test-Path $envPy)) {
  Write-Output 'CreativeCraft: creating the Python 3.11 environment...'
  & $py311 -m venv (Join-Path $app 'env')
  if ($LASTEXITCODE -ne 0) { Write-Output 'CreativeCraft: the Python environment could not be created.'; exit 1 }
}

# --- Python packages ---------------------------------------------------
Write-Output 'CreativeCraft: installing Python packages (several GB, once)...'
& $envPy -m pip install --upgrade pip
$req = Join-Path $app 'requirements.txt'
if (Test-Path $req) {
  & $envPy -m pip install -r $req
  if ($LASTEXITCODE -ne 0) { Write-Output 'CreativeCraft: pip stopped with an error (see above). Press Install / repair to resume - finished packages are kept.'; exit 1 }
}

# --- GPU build of torch ------------------------------------------------
Write-Output 'CreativeCraft: installing the CUDA build of torch (GPU acceleration)...'
& $envPy -m pip install torch==2.10.0 torchvision==0.25.0 torchaudio==2.10.0 --index-url https://download.pytorch.org/whl/cu130 --force-reinstall --no-deps
if ($LASTEXITCODE -ne 0) { Write-Output 'CreativeCraft: the CUDA torch install failed (driver/network). Maestro would run on the CPU only.' }

# --- React interface ---------------------------------------------------
$ui = Join-Path $folder 'ui'
if (Test-Path (Join-Path $ui 'package.json')) {
  Write-Output 'CreativeCraft: building the Maestro interface...'
  Push-Location $ui
  if (-not (Test-Path (Join-Path $ui 'node_modules'))) { npm install }
  npm run build
  Pop-Location
}

# --- check -------------------------------------------------------------
Write-Output 'CreativeCraft: checking that the packages import...'
& $envPy -c "import torch; print('torch', torch.__version__, 'cuda', torch.cuda.is_available())"
if ($LASTEXITCODE -ne 0) {
  Write-Output 'CreativeCraft: the packages installed, but torch still does not import - read the lines above.'
  exit 1
}
Write-Output 'CreativeCraft: Maestro is installed - press Start Maestro.'
"#;

/// One running install, with its output.
pub struct MaestroInstall {
    child: Child,
    lines: Arc<Mutex<Vec<String>>>,
}

impl MaestroInstall {
    /// Set Maestro up in `folder`: tools if missing, clone, Python 3.11, packages,
    /// CUDA torch, interface. Safe to run again — finished steps are skipped.
    pub fn start(folder: &Path) -> Result<Self, String> {
        let script_dir = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("CreativeCraft")
            .join("maestro");
        std::fs::create_dir_all(&script_dir)
            .map_err(|error| format!("Could not create {}: {error}", script_dir.display()))?;
        let script = script_dir.join("install.ps1");
        std::fs::write(&script, script_for(folder))
            .map_err(|error| format!("Could not write {}: {error}", script.display()))?;

        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .env("PYTHONUNBUFFERED", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start the installer: {error}"))?;
        let lines = Arc::new(Mutex::new(Vec::new()));
        if let Some(stdout) = child.stdout.take() {
            pump(stdout, Arc::clone(&lines));
        }
        if let Some(stderr) = child.stderr.take() {
            pump(stderr, Arc::clone(&lines));
        }
        Ok(Self { child, lines })
    }

    /// The last line the installer printed.
    pub fn tail(&self) -> String {
        let lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        lines
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_default()
    }

    /// Has the installer finished? `Some(true)` was success.
    pub fn finished(&mut self) -> Option<bool> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.success()),
            Ok(None) => None,
            Err(_) => Some(false),
        }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for MaestroInstall {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pump<R: std::io::Read + Send + 'static>(stream: R, lines: Arc<Mutex<Vec<String>>>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let mut buffer = lines.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.push(line);
            if buffer.len() > 400 {
                buffer.drain(..200);
            }
        }
    });
}

/// The script with the folder filled in.
fn script_for(folder: &Path) -> String {
    SCRIPT.replace("{{FOLDER}}", &folder.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_names_the_folder_and_the_steps() {
        let script = script_for(std::path::Path::new("C:\\Users\\me\\Maestro"));
        assert!(script.contains("C:\\Users\\me\\Maestro"));
        assert!(script.contains("git clone https://github.com/Blizaine/Maestro"));
        assert!(
            script.contains("Python.Python.3.11"),
            "Python 3.11 is installed when missing"
        );
        assert!(
            script.contains("OpenJS.NodeJS.LTS"),
            "Node is installed when missing"
        );
        assert!(
            script.contains("download.pytorch.org/whl/cu130"),
            "GPU torch from the CUDA index"
        );
        assert!(script.contains("npm run build"));
        assert!(
            !script.contains("huggingface.co") && !script.contains("civitai"),
            "model weights stay Maestro's own download, not the installer's"
        );
    }

    #[test]
    fn an_existing_environment_of_the_wrong_python_is_rebuilt() {
        let script = script_for(std::path::Path::new("C:\\m"));
        assert!(script.contains("-ne '3.11'"));
        assert!(script.contains("Remove-Item"));
    }
}
