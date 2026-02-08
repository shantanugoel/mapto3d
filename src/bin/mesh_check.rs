use anyhow::{Context, Result, bail};

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::path::PathBuf;

use stl_io::IndexedMesh;

const FLOATING_COMPONENT_Z_THRESHOLD: f32 = 1e-4;

#[derive(Debug, Default)]
struct MeshStats {
    vertices: usize,
    faces: usize,
    boundary_edges: usize,
    non_manifold_edges: usize,
    degenerate_faces: usize,
    components: usize,
    floating_components: usize,
    min_z: f32,
    max_z: f32,
    stl_io_validate_ok: bool,
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .context("Usage: cargo run --bin mesh_check -- <path/to/model.stl>")?;

    let mut file =
        File::open(&path).with_context(|| format!("Failed to open STL: {}", path.display()))?;
    let mesh = stl_io::read_stl(&mut file)
        .with_context(|| format!("Failed to parse STL: {}", path.display()))?;

    let stats = analyze_mesh(&mesh);
    print_report(&path, &stats);

    if stats.boundary_edges > 0 || stats.non_manifold_edges > 0 {
        bail!(
            "Mesh is not manifold: {} boundary edges, {} non-manifold edges",
            stats.boundary_edges,
            stats.non_manifold_edges
        );
    }

    if stats.floating_components > 0 {
        bail!(
            "Mesh contains {} floating components (min z > {:.4})",
            stats.floating_components,
            FLOATING_COMPONENT_Z_THRESHOLD
        );
    }

    Ok(())
}

fn analyze_mesh(mesh: &IndexedMesh) -> MeshStats {
    let mut stats = MeshStats {
        vertices: mesh.vertices.len(),
        faces: mesh.faces.len(),
        min_z: f32::INFINITY,
        max_z: f32::NEG_INFINITY,
        stl_io_validate_ok: mesh.validate().is_ok(),
        ..Default::default()
    };

    if mesh.faces.is_empty() {
        stats.min_z = 0.0;
        stats.max_z = 0.0;
        return stats;
    }

    let mut edge_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    let mut face_min_z = vec![f32::INFINITY; mesh.faces.len()];

    for (face_index, face) in mesh.faces.iter().enumerate() {
        let [a, b, c] = face.vertices;
        if a == b || b == c || c == a {
            stats.degenerate_faces += 1;
        }

        for &vertex_index in &face.vertices {
            let z = mesh.vertices[vertex_index][2];
            stats.min_z = stats.min_z.min(z);
            stats.max_z = stats.max_z.max(z);
            face_min_z[face_index] = face_min_z[face_index].min(z);
        }

        for (u, v) in [(a, b), (b, c), (c, a)] {
            edge_faces
                .entry(sorted_edge(u, v))
                .or_default()
                .push(face_index);
        }
    }

    stats.boundary_edges = edge_faces
        .values()
        .filter(|face_ids| face_ids.len() == 1)
        .count();
    stats.non_manifold_edges = edge_faces
        .values()
        .filter(|face_ids| face_ids.len() > 2)
        .count();

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); mesh.faces.len()];
    for face_ids in edge_faces.values() {
        if face_ids.len() < 2 {
            continue;
        }
        for i in 0..face_ids.len() {
            for j in (i + 1)..face_ids.len() {
                let a = face_ids[i];
                let b = face_ids[j];
                adjacency[a].push(b);
                adjacency[b].push(a);
            }
        }
    }

    let mut visited = vec![false; mesh.faces.len()];
    for start in 0..mesh.faces.len() {
        if visited[start] {
            continue;
        }

        stats.components += 1;
        let mut component_min_z = f32::INFINITY;
        let mut queue = VecDeque::from([start]);
        visited[start] = true;

        while let Some(face_index) = queue.pop_front() {
            component_min_z = component_min_z.min(face_min_z[face_index]);

            for &neighbor in &adjacency[face_index] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }

        if component_min_z > FLOATING_COMPONENT_Z_THRESHOLD {
            stats.floating_components += 1;
        }
    }

    stats
}

fn print_report(path: &PathBuf, stats: &MeshStats) {
    println!("Mesh analysis for {}", path.display());
    println!("  vertices: {}", stats.vertices);
    println!("  faces: {}", stats.faces);
    println!("  z-range: {:.4} .. {:.4}", stats.min_z, stats.max_z);
    println!("  boundary edges: {}", stats.boundary_edges);
    println!("  non-manifold edges: {}", stats.non_manifold_edges);
    println!("  degenerate faces: {}", stats.degenerate_faces);
    println!("  connected components: {}", stats.components);
    println!("  floating components: {}", stats.floating_components);
    println!(
        "  stl_io::IndexedMesh::validate(): {}",
        if stats.stl_io_validate_ok {
            "ok"
        } else {
            "failed"
        }
    );
}

fn sorted_edge(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}
