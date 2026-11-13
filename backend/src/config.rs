pub const DEFAULT_PORT: u16 = 4001;
pub const DEFAULT_MESH_SEGMENTS: u32 = 32;
/// Largest request body: an uploaded GDML file, or a CAD file to import.
/// Large tessellated geometry (an exported CAD import can exceed 100 MB) must
/// reopen; this is a single-user tool on localhost.
pub const MAX_UPLOAD_BYTES: usize = 1 << 30;

pub fn port() -> u16 {
    std::env::var("GDML_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub fn mesh_segments() -> u32 {
    std::env::var("GDML_MESH_SEGMENTS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MESH_SEGMENTS)
}

/// Set by the desktop app (`desktop/main.js`), which holds the backend's stdin
/// open: the backend exits when it closes, so that none outlives the app, not
/// even a killed one, keeping its installed files in use.
pub fn exit_with_stdin() -> bool {
    std::env::var_os("GDML_EXIT_WITH_STDIN").is_some()
}
