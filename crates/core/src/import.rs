//! Leitores de malha do mercado: STL, PLY e glTF/GLB (além de OBJ e DXF, em `mesh`).
use crate::mesh::{Mesh, Tri};
use glam::{Mat3, Mat4, Vec3};
use std::path::Path;

const GRAY: Vec3 = Vec3::splat(0.8);

fn rd_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl Mesh {
    /// STL binário ou ASCII.
    pub fn from_stl(bytes: &[u8]) -> Result<Mesh, String> {
        let mut tris: Vec<([Vec3; 3], u16)> = vec![];
        let binary_len = |b: &[u8]| (b.len() >= 84).then(|| 84 + 50 * u32::from_le_bytes([b[80], b[81], b[82], b[83]]) as usize);
        if binary_len(bytes) == Some(bytes.len()) {
            let n = (bytes.len() - 84) / 50;
            for i in 0..n {
                let o = 84 + i * 50 + 12;
                let p = |k: usize| Vec3::new(rd_f32(bytes, o + k * 12), rd_f32(bytes, o + k * 12 + 4), rd_f32(bytes, o + k * 12 + 8));
                tris.push(([p(0), p(1), p(2)], 0));
            }
        } else {
            let text = String::from_utf8_lossy(bytes);
            let mut cur: Vec<Vec3> = vec![];
            for line in text.lines() {
                let mut it = line.split_whitespace();
                if it.next() == Some("vertex") {
                    let v: Vec<f32> = it.filter_map(|x| x.parse().ok()).collect();
                    if v.len() == 3 {
                        cur.push(Vec3::new(v[0], v[1], v[2]));
                    }
                    if cur.len() == 3 {
                        tris.push(([cur[0], cur[1], cur[2]], 0));
                        cur.clear();
                    }
                }
            }
        }
        if tris.is_empty() {
            return Err("STL sem triângulos".into());
        }
        Ok(Mesh::smooth_from_faces(tris, vec![GRAY]))
    }

    /// PLY (ASCII, binário little/big endian): vértices x/y/z e faces poligonais.
    pub fn from_ply(bytes: &[u8]) -> Result<Mesh, String> {
        let end = bytes.windows(10).position(|w| w == b"end_header").ok_or("PLY sem cabeçalho")?;
        let header = String::from_utf8_lossy(&bytes[..end]).into_owned();
        let mut body = end + 10;
        while body < bytes.len() && bytes[body] != b'\n' {
            body += 1;
        }
        body += 1;
        #[derive(Clone)]
        struct Prop {
            name: String,
            ty: String,
            list: Option<String>,
        }
        let mut format = String::new();
        let mut elems: Vec<(String, usize, Vec<Prop>)> = vec![];
        for line in header.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            match t.as_slice() {
                ["format", f, ..] => format = f.to_string(),
                ["element", n, c] => elems.push((n.to_string(), c.parse().map_err(|_| "PLY: contagem inválida")?, vec![])),
                ["property", "list", ct, it, n] => {
                    if let Some(e) = elems.last_mut() {
                        e.2.push(Prop { name: n.to_string(), ty: ct.to_string(), list: Some(it.to_string()) });
                    }
                }
                ["property", ty, n] => {
                    if let Some(e) = elems.last_mut() {
                        e.2.push(Prop { name: n.to_string(), ty: ty.to_string(), list: None });
                    }
                }
                _ => {}
            }
        }
        let size_of = |ty: &str| match ty {
            "char" | "uchar" | "int8" | "uint8" => 1,
            "short" | "ushort" | "int16" | "uint16" => 2,
            "int" | "uint" | "float" | "int32" | "uint32" | "float32" => 4,
            "double" | "float64" => 8,
            _ => 4,
        };
        let ascii = format == "ascii";
        let be = format == "binary_big_endian";
        let data = &bytes[body.min(bytes.len())..];
        let text = if ascii { Some(String::from_utf8_lossy(data).into_owned()) } else { None };
        let mut toks = text.as_deref().unwrap_or("").split_whitespace();
        let mut pos = 0usize;
        let mut read = |ty: &str| -> Result<f64, String> {
            if ascii {
                return toks.next().and_then(|t| t.parse::<f64>().ok()).ok_or_else(|| "PLY: dados ASCII truncados".to_string());
            }
            let n = size_of(ty);
            if pos + n > data.len() {
                return Err("PLY: dados binários truncados".into());
            }
            let mut b = [0u8; 8];
            b[..n].copy_from_slice(&data[pos..pos + n]);
            if be {
                b[..n].reverse();
            }
            pos += n;
            Ok(match ty {
                "char" | "int8" => b[0] as i8 as f64,
                "uchar" | "uint8" => b[0] as f64,
                "short" | "int16" => i16::from_le_bytes([b[0], b[1]]) as f64,
                "ushort" | "uint16" => u16::from_le_bytes([b[0], b[1]]) as f64,
                "int" | "int32" => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                "uint" | "uint32" => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                "float" | "float32" => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                _ => f64::from_le_bytes(b),
            })
        };
        let mut verts: Vec<Vec3> = vec![];
        let mut tris: Vec<([Vec3; 3], u16)> = vec![];
        for (name, count, props) in &elems {
            for _ in 0..*count {
                let mut xyz = [0.0f32; 3];
                let mut face: Vec<usize> = vec![];
                for p in props {
                    if let Some(it) = &p.list {
                        let n = read(&p.ty)? as usize;
                        let mut idx = Vec::with_capacity(n);
                        for _ in 0..n {
                            idx.push(read(it)? as usize);
                        }
                        if p.name == "vertex_indices" || p.name == "vertex_index" {
                            face = idx;
                        }
                    } else {
                        let v = read(&p.ty)?;
                        match p.name.as_str() {
                            "x" => xyz[0] = v as f32,
                            "y" => xyz[1] = v as f32,
                            "z" => xyz[2] = v as f32,
                            _ => {}
                        }
                    }
                }
                if name == "vertex" {
                    verts.push(Vec3::from(xyz));
                } else if name == "face" && face.len() >= 3 {
                    for k in 1..face.len() - 1 {
                        let (a, b, c) = (face[0], face[k], face[k + 1]);
                        if a < verts.len() && b < verts.len() && c < verts.len() {
                            tris.push(([verts[a], verts[b], verts[c]], 0));
                        }
                    }
                }
            }
        }
        if tris.is_empty() {
            return Err("PLY sem faces".into());
        }
        Ok(Mesh::smooth_from_faces(tris, vec![GRAY]))
    }

    /// glTF 2.0 (.gltf com .bin ou .glb): todas as malhas da cena, com as transformações dos nós.
    pub fn from_gltf(path: &Path) -> Result<Mesh, String> {
        let (doc, buffers, _) = gltf::import(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut mats: Vec<Vec3> = vec![];
        let mut mat_ids: Vec<Option<usize>> = vec![];
        let mut with_n: Vec<Tri> = vec![];
        let mut without: Vec<([Vec3; 3], u16)> = vec![];
        let scene = doc.default_scene().or_else(|| doc.scenes().next()).ok_or("glTF sem cena")?;
        let mut stack: Vec<(gltf::Node, Mat4)> = scene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
        while let Some((node, parent)) = stack.pop() {
            let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
            for c in node.children() {
                stack.push((c, world));
            }
            let Some(mesh) = node.mesh() else { continue };
            let nm = Mat3::from_mat4(world).inverse().transpose();
            let flip = world.determinant() < 0.0;
            for prim in mesh.primitives() {
                if prim.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }
                let reader = prim.reader(|b| Some(&buffers[b.index()]));
                let Some(pos) = reader.read_positions() else { continue };
                let pos: Vec<Vec3> = pos.map(|p| world.transform_point3(Vec3::from(p))).collect();
                let nrm: Option<Vec<Vec3>> = reader.read_normals().map(|n| n.map(|v| (nm * Vec3::from(v)).normalize_or_zero()).collect());
                let idx: Vec<u32> = match reader.read_indices() {
                    Some(i) => i.into_u32().collect(),
                    None => (0..pos.len() as u32).collect(),
                };
                let id = prim.material().index();
                let slot = match mat_ids.iter().position(|m| *m == id) {
                    Some(s) => s,
                    None => {
                        let c = prim.material().pbr_metallic_roughness().base_color_factor();
                        mats.push(Vec3::new(c[0], c[1], c[2]));
                        mat_ids.push(id);
                        mats.len() - 1
                    }
                } as u16;
                for t in idx.chunks_exact(3) {
                    let (a, mut b, mut c) = (t[0] as usize, t[1] as usize, t[2] as usize);
                    if a >= pos.len() || b >= pos.len() || c >= pos.len() {
                        continue;
                    }
                    if flip {
                        std::mem::swap(&mut b, &mut c);
                    }
                    match &nrm {
                        Some(n) => with_n.push(Tri { v: [pos[a], pos[b], pos[c]], n: [n[a], n[b], n[c]], mat: slot }),
                        None => without.push(([pos[a], pos[b], pos[c]], slot)),
                    }
                }
            }
        }
        if mats.is_empty() {
            mats.push(GRAY);
        }
        let mut out = if without.is_empty() { Mesh { tris: vec![], mats: mats.clone() } } else { Mesh::smooth_from_faces(without, mats.clone()) };
        out.mats = mats;
        out.tris.extend(with_n);
        if out.tris.is_empty() {
            return Err("glTF sem triângulos".into());
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stl_ascii_and_binary_agree() {
        let ascii = "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid";
        let a = Mesh::from_stl(ascii.as_bytes()).unwrap();
        let mut b = vec![0u8; 80];
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&[0u8; 12]);
        for v in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in v {
                b.extend_from_slice(&c.to_le_bytes());
            }
        }
        b.extend_from_slice(&[0, 0]);
        let m = Mesh::from_stl(&b).unwrap();
        assert_eq!((a.tris.len(), m.tris.len()), (1, 1));
        assert_eq!(a.tris[0].v, m.tris[0].v);
    }

    #[test]
    fn ply_quad_becomes_two_triangles_ascii_and_binary() {
        let ascii = "ply\nformat ascii 1.0\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        assert_eq!(Mesh::from_ply(ascii.as_bytes()).unwrap().tris.len(), 2);
        let mut b = b"ply\nformat binary_little_endian 1.0\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n".to_vec();
        for v in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in v {
                b.extend_from_slice(&c.to_le_bytes());
            }
        }
        b.push(4);
        for i in 0..4i32 {
            b.extend_from_slice(&i.to_le_bytes());
        }
        assert_eq!(Mesh::from_ply(&b).unwrap().tris.len(), 2);
    }

    #[test]
    fn broken_files_report_errors_instead_of_panicking() {
        assert!(Mesh::from_stl(b"nada").is_err());
        assert!(Mesh::from_ply(b"ply\nformat ascii 1.0\nend_header\n").is_err());
        assert!(Mesh::from_ply(b"lixo").is_err());
    }
}
