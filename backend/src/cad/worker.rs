//! The STEP reader: `gdml-cad-worker` (cad-worker/), a separate process on
//! OpenCASCADE, so that the backend stays one static binary and a kernel
//! crash cannot take it down. It reads the file on stdin and writes the
//! JSON below on stdout; these structs own that format. Its progress lines
//! on stderr feed [`Control`], and a cancelled import kills it.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::ir::Analytic;
use super::Control;

#[derive(Deserialize)]
pub struct Output {
    /// Entries of the top-level labels.
    pub roots: Vec<String>,
    /// Every label reachable from them, once.
    pub labels: Vec<Label>,
    /// Placed solids whose interiors overlap.
    pub interferences: Vec<Interference>,
    /// Placed solids closer than their meshes' chordal deviations, without
    /// interfering: where a mesh beside an exact solid can cut into it.
    pub contacts: Vec<Interference>,
    /// Pairs of placed solids the interference check did not decide.
    pub unchecked: usize,
    /// About the file as a whole.
    pub issues: Vec<String>,
}

/// An XCAF label: an assembly of components, or a part with solids.
#[derive(Deserialize)]
pub struct Label {
    pub entry: String,
    pub name: String,
    pub assembly: bool,
    pub components: Vec<Component>,
    /// None where `--only` skipped the solid.
    pub solids: Vec<Option<Solid>>,
    /// What of the part is not imported, and why.
    pub issues: Vec<String>,
}

/// An instance of the label `target` in an assembly.
#[derive(Deserialize)]
pub struct Component {
    pub target: String,
    /// 3 x 4, row-major: rotation and translation in mm.
    pub matrix: [f64; 12],
}

#[derive(Deserialize)]
pub struct Solid {
    /// BRepCheck_Analyzer's verdict, after any healing.
    pub valid: bool,
    pub tolerance_mm: f64,
    /// Exact, from the B-rep.
    pub volume_mm3: f64,
    pub area_mm2: f64,
    /// Minimum x, y, z, then maximum.
    pub bounds_mm: [f64; 6],
    pub mesh: Mesh,
    /// Its exact GDML solid, or why there is none.
    pub exact: Option<Analytic>,
    pub exact_reason: String,
    pub issues: Vec<String>,
}

/// Face triangulations stitched along the B-rep edges, in the part's frame.
#[derive(Deserialize)]
pub struct Mesh {
    /// x, y, z per node, in mm.
    pub positions: Vec<f64>,
    /// Three nodes per facet.
    pub triangles: Vec<u32>,
    pub chord_mm: f64,
    pub issues: Vec<String>,
}

/// Two placed solids, each named by its path: the root index, the component
/// index at each assembly level, then the solid index.
#[derive(Deserialize)]
pub struct Interference {
    pub a: Vec<usize>,
    pub b: Vec<usize>,
    pub volume_mm3: f64,
}

/// How to mesh: the mesh volume error to aim for, chordal deviations fixed
/// per solid id ("<label entry>.<solid index>"), and whether to mesh only
/// those solids, skipping the interference check.
pub struct Meshing<'a> {
    pub volume_target: f64,
    pub chords: &'a BTreeMap<String, f64>,
    pub only: bool,
}

/// `$GDML_CAD_WORKER`, else `cad/gdml-cad-worker` next to this executable,
/// where the desktop app ships it.
fn locate() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GDML_CAD_WORKER") {
        return Some(path.into());
    }
    let exe = std::env::current_exe().ok()?;
    let name = format!("gdml-cad-worker{}", std::env::consts::EXE_SUFFIX);
    Some(exe.parent()?.join("cad").join(name)).filter(|path| path.is_file())
}

/// Run the worker on a STEP file, reporting its progress to `control` (under
/// `phase` when given, else the worker's own phase names). The error
/// completes "<filename>: ...".
pub fn run(
    bytes: &[u8],
    meshing: &Meshing,
    control: &Control,
    phase: Option<&str>,
) -> Result<Output, String> {
    let path = locate().ok_or("STEP import is not available in this build")?;
    let mut command = Command::new(&path);
    command
        .arg("--volume-target")
        .arg(meshing.volume_target.to_string());
    for (id, chord) in meshing.chords {
        command.arg("--chord").arg(format!("{id}={chord}"));
    }
    if meshing.only {
        command.arg("--only");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // Without this, a console window flashes up for every import.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("the STEP reader {} could not start ({e})", path.display()))?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    // The pipes are served by threads, so that none fills up, while this one
    // waits for the worker to exit and kills it if the import is cancelled.
    // A worker that stops reading early says why when it exits.
    let (status, json, message) = std::thread::scope(|scope| {
        scope.spawn(move || stdin.write_all(bytes));
        let json = scope.spawn(move || {
            let mut json = String::new();
            stdout.read_to_string(&mut json).map(|_| json)
        });
        let message = scope.spawn(move || {
            let mut message = None;
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Some(error) = line.strip_prefix("error ") {
                    message = Some(error.to_string());
                } else if let ["progress", name, done, total] =
                    line.split(' ').collect::<Vec<_>>()[..]
                {
                    if let (Ok(done), Ok(total)) = (done.parse(), total.parse()) {
                        control.set(phase.unwrap_or(name), done, total);
                    }
                }
            }
            message
        });
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if control.cancelled() {
                        let _ = child.kill();
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => break Err(e),
            }
        };
        (status, json.join(), message.join())
    });
    if control.cancelled() {
        return Err("the import was cancelled".to_string());
    }
    let status = status.map_err(|e| format!("the STEP reader failed ({e})"))?;
    let message = message.ok().flatten();
    let message = message.as_deref().unwrap_or("no message");
    match status.code() {
        Some(0) => match json {
            Ok(Ok(json)) => serde_json::from_str(&json)
                .map_err(|e| format!("the STEP reader's output is not as expected ({e})")),
            _ => Err("the STEP reader's output could not be read".to_string()),
        },
        Some(3) => Err(message.to_string()),
        _ => Err(format!("the STEP reader failed ({status}: {message})")),
    }
}
