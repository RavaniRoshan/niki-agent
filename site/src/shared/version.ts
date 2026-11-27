// The version the site falls back to when the generated changelog has no
// release entry. `rust/Cargo.toml` stays authoritative for the binary; this
// constant keeps the download page and the prerendered metadata in step when
// the site is built without the rest of the repository on disk.
export const VERSION = '2.2.0';
