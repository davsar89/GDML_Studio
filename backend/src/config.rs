pub const DEFAULT_PORT: u16 = 3001;
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_MESH_SEGMENTS: u32 = 32;

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
