use glam::{Vec2, Vec3};

#[derive(Default, Clone, Debug)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub indices: Vec<u16>,
}

impl Mesh {
    // Specification: https://en.wikipedia.org/wiki/Wavefront_.obj_file
    pub fn from_obj(s: &[u8]) -> Result<Self, String> {
        let mut vertices = vec![];
        let mut uvs = vec![];
        let mut faces: Vec<[Option<[u16; 3]>; 3]> = vec![];
        let text = std::str::from_utf8(s).map_err(|e| format!("{}", e))?;
        for (line_number, line) in text.lines().enumerate() {
            let mut tokens = line.split_whitespace();
            let Some(kind_token) = tokens.next() else {
                continue;
            };
            match kind_token {
                "v" => {
                    let v1 = tokens
                        .next()
                        .ok_or(format!("Obj: missing vertex.x at line {}", line_number))?;
                    let v2 = tokens
                        .next()
                        .ok_or(format!("Obj: missing vertex.y at line {}", line_number))?;
                    let v3 = tokens
                        .next()
                        .ok_or(format!("Obj: missing vertex.z at line {}", line_number))?;
                    let v1: f32 = v1
                        .parse()
                        .map_err(|e| format!("Obj: {} at {line_number}", e))?;
                    let v2: f32 = v2
                        .parse()
                        .map_err(|e| format!("Obj: {} at {line_number}", e))?;
                    let v3: f32 = v3
                        .parse()
                        .map_err(|e| format!("Obj: {} at {line_number}", e))?;
                    vertices.push(Vec3::new(v1, v2, v3));
                }
                "f" => {
                    let f1 = tokens
                        .next()
                        .ok_or(format!("Obj: missing face index 0 at line {}", line_number))?;
                    let [f1_vertex, f1_texture, f1_normal] =
                        obj_parse_face_indices(f1, line_number)?;
                    let f2 = tokens
                        .next()
                        .ok_or(format!("Obj: missing face index 1 at line {}", line_number))?;
                    let [f2_vertex, f2_texture, f2_normal] =
                        obj_parse_face_indices(f2, line_number)?;
                    let f3 = tokens
                        .next()
                        .ok_or(format!("Obj: missing face index 2 at line {}", line_number))?;
                    let [f3_vertex, f3_texture, f3_normal] =
                        obj_parse_face_indices(f3, line_number)?;
                    assert!(
                        tokens.next().is_none(),
                        "Obj: only triangles supported, line {}",
                        line_number
                    );
                    let mut face = [None, None, None];
                    if let (Some(f1), Some(f2), Some(f3)) = (f1_vertex, f2_vertex, f3_vertex) {
                        face[0] = Some([f1 - 1, f2 - 1, f3 - 1]);
                    } else {
                        panic!(
                            "Obj: face at line {line_number} doesn't have all vertex indices set"
                        );
                    }
                    if let (Some(f1), Some(f2), Some(f3)) = (f1_texture, f2_texture, f3_texture) {
                        face[1] = Some([f1 - 1, f2 - 1, f3 - 1]);
                    }
                    if let (Some(f1), Some(f2), Some(f3)) = (f1_normal, f2_normal, f3_normal) {
                        face[2] = Some([f1 - 1, f2 - 1, f3 - 1]);
                    }
                    faces.push(face);
                }
                "vt" => {
                    let u = tokens
                        .next()
                        .ok_or(format!("Obj: missing vertex.u at line {}", line_number))?;
                    let v = tokens
                        .next()
                        .ok_or(format!("Obj: missing vertex.v at line {}", line_number))?;
                    let u: f32 = u
                        .parse()
                        .map_err(|e| format!("Obj: {} at {line_number}", e))?;
                    let v: f32 = v
                        .parse()
                        .map_err(|e| format!("Obj: {} at {line_number}", e))?;
                    uvs.push(Vec2::new(u, 1. - v));
                }
                _ => {}
            }
        }

        // Generate the triangles from the faces
        let mut packed_vertices = vec![];
        let mut packed_uvs = vec![];
        let mut packed_indices = vec![];
        let mut index = 0;
        for [vert_index, uv_index, _] in faces {
            let Some(vert_index) = vert_index else {
                continue;
            };
            packed_vertices.push(vertices[vert_index[0] as usize]);
            packed_vertices.push(vertices[vert_index[1] as usize]);
            packed_vertices.push(vertices[vert_index[2] as usize]);
            packed_indices.push(index + 0);
            packed_indices.push(index + 1);
            packed_indices.push(index + 2);
            index += 3;
            let Some(uv_index) = uv_index else {
                continue;
            };
            packed_uvs.push(uvs[uv_index[0] as usize]);
            packed_uvs.push(uvs[uv_index[1] as usize]);
            packed_uvs.push(uvs[uv_index[2] as usize]);
        }

        Ok(Mesh {
            vertices: packed_vertices,
            uvs: packed_uvs,
            indices: packed_indices,
        })
    }
}

fn obj_parse_face_indices(
    face_indices: &str,
    line_number: usize,
) -> Result<[Option<u16>; 3], String> {
    let mut tokens = face_indices.split("/");
    let i0 = obj_parse_index(&mut tokens, line_number).ok();
    let i1 = obj_parse_index(&mut tokens, line_number).ok();
    let i2 = obj_parse_index(&mut tokens, line_number).ok();
    Ok([i0, i1, i2])
}

fn obj_parse_index<'a>(
    iter: &mut impl Iterator<Item = &'a str>,
    line_number: usize,
) -> Result<u16, String> {
    let f = iter
        .next()
        .ok_or(format!("Obj: missing face index at line {}", line_number))?;
    f.parse().map_err(|e| format!("{} at {line_number}", e))
}
