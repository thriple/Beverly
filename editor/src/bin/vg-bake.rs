//! Tessera's bake check: bakes the test mesh (`tessera::test_mesh`, 1,310,720 triangles) with `tessera::bake`, saves
//! it in a plain temp folder under its cache key (`beverly::bake::key`), reads it and its proxy back, and prints the
//! time, the bytes, the bytes per triangle and the proxy's triangles. `vg-bake [LEVELS]` (default 8).
use std::time::Instant;
use tessera::bake::{self, BakeSettings, KIND};

fn main() -> std::process::ExitCode {
    let levels: u32 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(8);
    let settings = BakeSettings::default();
    let mesh = tessera::test_mesh::test_mesh(levels, 1);
    let triangles = mesh.indices().map_or(0, |i| i.len() / 3);

    let start = Instant::now();
    let bytes = match bake::bake(&mesh, &settings) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("bake failed: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let baked = start.elapsed();

    // the test mesh has no file; its generator's arguments stand in for the source bytes
    let parts = bake::key_parts(format!("tessera test_mesh {levels} 1").as_bytes(), &settings);
    let key = beverly::bake::key(KIND, &parts.iter().map(Vec::as_slice).collect::<Vec<_>>());
    let folder = std::env::temp_dir().join("beverly-vg-bake").join(KIND);
    let path = folder.join(&key);
    if let Err(e) = std::fs::create_dir_all(&folder).and_then(|_| std::fs::write(&path, &bytes)) {
        eprintln!("saving {} failed: {e}", path.display());
        return std::process::ExitCode::FAILURE;
    }

    let start = Instant::now();
    let loaded = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| {
        let meshlet = bake::load(&b).map_err(|e| e.to_string())?;
        let proxy = bake::proxy_mesh(&b).map_err(|e| e.to_string())?;
        Ok((meshlet, proxy))
    });
    let read = start.elapsed();
    let proxy_triangles = match loaded {
        Ok((_, proxy)) => proxy.indices().map_or(0, |i| i.len() / 3),
        Err(e) => {
            eprintln!("reading {} back failed: {e}", path.display());
            return std::process::ExitCode::FAILURE;
        }
    };

    println!("test mesh: {triangles} triangles");
    println!("baked in {baked:.2?}, read back in {read:.2?}");
    println!("{} bytes, {:.1} bytes per triangle", bytes.len(), bytes.len() as f64 / triangles as f64);
    println!("proxy (coarsest level): {proxy_triangles} triangles");
    println!("saved {}", path.display());
    std::process::ExitCode::SUCCESS
}
